<script setup lang="ts">
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, ref, shallowRef } from 'vue'

import type { ProjectDetail, ProjectVersion } from '../../../data'
import { projectIcon } from '../../../data'
import { chineseFileNamesEnabled } from '../../../lib/content-search'
import { usePreview } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'
import PreviewModal from '../../../overlays/PreviewModal.vue'
import { contentTypeOf, injectContentInstall, toProjectCard } from '../install'
import { messages } from '../messages'
import type { ContentSelectionItem, ContentSelectionType, ReviewedInstall } from '../state'
import { makeContentSelectionKey, useDiscover } from '../state'

/**
 * Port of "Review content changes" (apps/app-frontend/src/components/ui/
 * ResourceOperationPreviewModal.vue). The plan is resolved from the snapshot: the chosen or
 * automatic release of every selected project, required dependencies the snapshot knows
 * (already installed ones are "Already satisfied"), optional ones offered separately, and
 * the compatibility assessment against the instance. "Queue changes" hands every file to the
 * download queue.
 */
type Compatibility = 'compatible' | 'incompatible' | 'unknown'

interface PlanNode {
    id: string
    projectId: string
    title: string
    iconUrl: string | null
    summary: string
    state: 'add' | 'satisfied' | 'optionalCandidate'
    version: ProjectVersion | null
    fileName: string
    targetPath: string
    contentType: ContentSelectionType
    compatibility: { gameVersion: Compatibility; loader: Compatibility }
    root: boolean
    requiredBy: string[]
    optionalFor: string[]
    detail: ProjectDetail
}

const { formatMessage } = useVIntl()
const preview = usePreview()
const discover = useDiscover()
const install = injectContentInstall()

const modal = ref<InstanceType<typeof PreviewModal>>()
const nodes = shallowRef<PlanNode[]>([])
const exceptions = ref<Set<string>>(new Set())
const busy = ref(false)
const queueing = ref(false)
const closing = ref(false)
const planned = ref(false)
const instance = ref<GameInstance | null>(null)
const items = ref<ContentSelectionItem[]>([])
const worldOptions = ref<{ value: string; label: string }[]>([])
const targetWorld = ref<string | null>(null)
/** Required dependencies the snapshot does not cover; the desktop app would resolve them. */
const unknownDependencies = ref(new Set<string>())
let generation = 0
let resolveShow: ((value: ReviewedInstall | null) => void) | null = null

const needsWorld = computed(() => items.value.some((item) => item.contentType === 'datapack'))
const active = computed(() => nodes.value.filter((node) => node.state !== 'optionalCandidate'))
const candidates = computed(() => nodes.value.filter((node) => node.state === 'optionalCandidate'))
const actions = computed(() => active.value.filter((node) => node.state === 'add'))
const blocking = computed(() =>
    active.value.some(
        (node) =>
            node.state === 'add' &&
            (['gameVersion', 'loader'] as const).some(
                (dimension) =>
                    node.compatibility[dimension] === 'incompatible' &&
                    !exceptions.value.has(`${node.id}:${dimension}`),
            ),
    ),
)
const canApply = computed(
    () =>
        !busy.value &&
        !queueing.value &&
        planned.value &&
        actions.value.length > 0 &&
        !blocking.value,
)

function folderFor(contentType: ContentSelectionType) {
    switch (contentType) {
        case 'resourcepack':
            return 'resourcepacks'
        case 'shader':
            return 'shaderpacks'
        case 'datapack':
            return `${targetWorld.value ?? 'saves'}/datapacks`
        default:
            return 'mods'
    }
}

function compatibilityOf(
    version: ProjectVersion | null,
    contentType: ContentSelectionType,
    target: GameInstance,
) {
    if (!version)
        return { gameVersion: 'unknown' as Compatibility, loader: 'unknown' as Compatibility }
    const gameVersion: Compatibility =
        contentType === 'resourcepack' || version.game_versions.includes(target.game_version)
            ? 'compatible'
            : 'incompatible'
    let loader: Compatibility = 'compatible'
    if (contentType === 'mod')
        loader =
            version.loaders.includes(target.loader) || version.loaders.includes('datapack')
                ? 'compatible'
                : 'incompatible'
    else if (contentType === 'datapack')
        loader = version.loaders.includes('datapack') ? 'compatible' : 'incompatible'
    return { gameVersion, loader }
}

function releaseFor(
    detail: ProjectDetail,
    contentType: ContentSelectionType,
    target: GameInstance,
    versionId?: string,
) {
    if (versionId) return detail.versions.find((version) => version.id === versionId) ?? null
    return (
        install.automaticVersion(detail, target, contentType) ??
        detail.versions
            .slice()
            .sort((a, b) => Date.parse(b.date_published) - Date.parse(a.date_published))[0] ??
        null
    )
}

function makeNode(
    detail: ProjectDetail,
    version: ProjectVersion | null,
    contentType: ContentSelectionType,
    target: GameInstance,
    state: PlanNode['state'],
    root: boolean,
): PlanNode {
    const file = version?.files.find((entry) => entry.primary) ?? version?.files[0]
    const fileName = file?.filename ?? ''
    return {
        id: makeContentSelectionKey('modrinth', detail.id),
        projectId: detail.id,
        title: detail.title,
        iconUrl: projectIcon(detail.id),
        summary: detail.description,
        state,
        version,
        fileName,
        targetPath: `${folderFor(contentType)}/${fileName}`,
        contentType,
        compatibility: compatibilityOf(version, contentType, target),
        root,
        requiredBy: [],
        optionalFor: [],
        detail,
    }
}

async function prepare(current: number) {
    const target = instance.value
    if (!target) return
    busy.value = true
    planned.value = false
    nodes.value = []
    unknownDependencies.value = new Set()
    const plan: PlanNode[] = []
    const unknown = new Set<string>()
    const installed = discover.installedProjectIds(target.id)
    for (const item of items.value) {
        const detail = await discover.ensureDetail(item.projectId)
        if (current !== generation) return
        if (!detail) continue
        const version = releaseFor(detail, item.contentType, target, item.versionId)
        const installedVersion = discover.installedVersionId(target.id, detail.id)
        const state =
            installed.has(detail.id) && installedVersion === version?.id ? 'satisfied' : 'add'
        plan.push(makeNode(detail, version, item.contentType, target, state, true))
        // Progress rows appear while dependencies are being resolved.
        nodes.value = [...plan]
    }
    for (const node of [...plan]) {
        if (!node.version || node.state !== 'add') continue
        for (const dependency of node.version.dependencies ?? []) {
            if (!dependency.project_id || dependency.project_id === node.projectId) continue
            if (
                dependency.dependency_type !== 'required' &&
                dependency.dependency_type !== 'optional'
            )
                continue
            const existing = plan.find((entry) => entry.projectId === dependency.project_id)
            if (existing) {
                if (dependency.dependency_type === 'required') {
                    if (!existing.requiredBy.includes(node.title))
                        existing.requiredBy.push(node.title)
                    if (existing.state === 'optionalCandidate') existing.state = 'add'
                } else if (!existing.optionalFor.includes(node.title))
                    existing.optionalFor.push(node.title)
                continue
            }
            // Only dependencies the snapshot knows can be planned; the others are named below.
            if (
                !discover.findCard(dependency.project_id) &&
                !discover.details.value[dependency.project_id]
            ) {
                if (
                    dependency.dependency_type === 'required' &&
                    !installed.has(dependency.project_id)
                )
                    unknown.add(dependency.project_id)
                continue
            }
            const depDetail = await discover.ensureDetail(dependency.project_id)
            if (current !== generation) return
            if (!depDetail) continue
            const depType = contentTypeOf(depDetail.project_type)
            const depVersion = dependency.version_id
                ? (depDetail.versions.find((version) => version.id === dependency.version_id) ??
                  releaseFor(depDetail, depType, target))
                : releaseFor(depDetail, depType, target)
            const state: PlanNode['state'] =
                dependency.dependency_type === 'required'
                    ? installed.has(depDetail.id)
                        ? 'satisfied'
                        : 'add'
                    : installed.has(depDetail.id)
                      ? 'satisfied'
                      : 'optionalCandidate'
            const depNode = makeNode(depDetail, depVersion, depType, target, state, false)
            if (dependency.dependency_type === 'required') depNode.requiredBy.push(node.title)
            else depNode.optionalFor.push(node.title)
            plan.push(depNode)
            nodes.value = [...plan]
        }
    }
    await new Promise<void>((resolve) => preview.clock.after(420, resolve))
    if (current !== generation) return
    nodes.value = [...plan]
    unknownDependencies.value = unknown
    planned.value = true
    busy.value = false
}

function instanceWorlds(instanceId: string): { value: string; label: string }[] {
    type WorldEntry = { name: string; kind: string; path?: string }
    const domain = preview.domains.get('instance-screen')?.value as
        { entryFor?: (id: string) => { worlds?: WorldEntry[] } } | undefined
    const worlds = domain?.entryFor?.(instanceId)?.worlds ?? []
    return worlds
        .filter((world) => world.kind === 'singleplayer')
        .map((world) => ({
            value: `saves/${(world.path ?? world.name).replace(/^saves\//, '')}`,
            label: world.name,
        }))
}

function hide() {
    generation++
    const resolve = resolveShow
    resolveShow = null
    resolve?.(null)
}

function showInstallSelection(input: {
    instance: GameInstance
    items: ContentSelectionItem[]
}): Promise<ReviewedInstall | null> {
    if (queueing.value) return Promise.resolve(null)
    hide()
    const current = ++generation
    instance.value = input.instance
    items.value = input.items
    nodes.value = []
    exceptions.value = new Set()
    planned.value = false
    targetWorld.value = null
    worldOptions.value = []
    modal.value?.show()
    const result = new Promise<ReviewedInstall | null>((resolve) => {
        resolveShow = resolve
    })
    if (needsWorld.value) {
        busy.value = true
        preview.clock.after(300, () => {
            if (current !== generation) return
            worldOptions.value = instanceWorlds(input.instance.id)
            busy.value = false
        })
        return result
    }
    void prepare(current)
    return result
}

function selectWorld(path: string) {
    if (busy.value) return
    targetWorld.value = path
    void prepare(generation)
}

function accept(node: PlanNode, dimension: 'gameVersion' | 'loader') {
    const next = new Set(exceptions.value)
    next.add(`${node.id}:${dimension}`)
    exceptions.value = next
}

function removeRoot(node: PlanNode) {
    if (busy.value) return
    items.value = items.value.filter((item) => item.key !== node.id)
    nodes.value = nodes.value.filter(
        (entry) =>
            entry.id !== node.id &&
            !(entry.requiredBy.length === 1 && entry.requiredBy[0] === node.title && !entry.root),
    )
}

function addCandidate(node: PlanNode) {
    if (busy.value) return
    nodes.value = nodes.value.map((entry) =>
        entry.id === node.id ? { ...entry, state: 'add' } : entry,
    )
}

async function apply() {
    const target = instance.value
    if (!target || !canApply.value) return
    const current = generation
    const resolve = resolveShow
    busy.value = true
    queueing.value = true
    try {
        const jobs: ReviewedInstall['jobs'] = []
        for (const node of actions.value) {
            const job = await preview.downloads.enqueue({
                project: toProjectCard(node.detail),
                instanceId: target.id,
                versionId: node.version?.id,
                chineseFileNames: chineseFileNamesEnabled(preview.domains),
            })
            if (job) jobs.push({ key: node.id, jobId: job.id })
        }
        if (generation !== current) return
        resolveShow = null
        closing.value = true
        // `disable-close` must see `closing` before the modal is asked to hide.
        await nextTick()
        await modal.value?.hide()
        resolve?.({
            jobs,
            activeRoots: active.value
                .filter((node) => node.root && node.state === 'add')
                .map((node) => node.id),
        })
    } finally {
        busy.value = false
        queueing.value = false
        closing.value = false
    }
}

function compatibilityLabel(state: Compatibility) {
    return formatMessage(messages.operationCompatibility, { state })
}

defineExpose({ showInstallSelection })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.operationHeader)"
        :on-hide="hide"
        :disable-close="busy && !closing"
        scrollable
        width="min(42rem, 1152px)"
    >
        <div class="flex flex-col gap-3">
            <div v-if="needsWorld" class="flex flex-col gap-2">
                <label>{{ formatMessage(messages.operationWorld) }}</label>
                <Combobox
                    :model-value="targetWorld"
                    :options="worldOptions"
                    :disabled="busy"
                    @update:model-value="selectWorld"
                />
                <p v-if="!busy && !worldOptions.length" class="m-0">
                    {{ formatMessage(messages.operationNoWorlds) }}
                </p>
            </div>
            <p v-if="planned && !busy" class="m-0">
                {{
                    formatMessage(messages.operationChanges, {
                        count: actions.length,
                        instance: instance?.name ?? '',
                    })
                }}
            </p>
            <p
                v-if="planned && !busy && unknownDependencies.size"
                class="m-0 text-sm text-[var(--color-text-tertiary)]"
            >
                {{
                    formatMessage(messages.operationUnknownDependencies, {
                        count: unknownDependencies.size,
                    })
                }}
            </p>
            <div
                v-for="node in active"
                :key="node.id"
                class="flex flex-col gap-2 rounded-lg border border-surface-4 bg-surface-2 p-3"
            >
                <div class="flex items-center gap-2">
                    <Avatar :src="node.iconUrl ?? undefined" :alt="node.title" size="32px" />
                    <span class="grow font-semibold">{{ node.title }}</span>
                    <Button
                        v-if="node.root"
                        size="sm"
                        :disabled="busy"
                        :aria-label="formatMessage(messages.operationRemove, { name: node.title })"
                        @click="removeRoot(node)"
                        >×</Button
                    >
                </div>
                <p
                    v-if="node.state !== 'satisfied' && node.summary"
                    class="m-0 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ node.summary }}
                </p>
                <p v-if="node.state !== 'satisfied' && node.version" class="m-0 break-all text-sm">
                    {{
                        formatMessage(messages.operationArtifact, {
                            source: 'Modrinth',
                            release: node.version.id,
                            file: node.fileName,
                        })
                    }}
                </p>
                <p v-if="node.state === 'satisfied'" class="m-0 text-sm">
                    {{ formatMessage(messages.operationSatisfied) }}
                </p>
                <p v-if="node.requiredBy.length" class="m-0 text-sm">
                    {{
                        formatMessage(messages.operationRequiredBy, {
                            names: node.requiredBy.join(', '),
                        })
                    }}
                </p>
                <p v-else-if="node.optionalFor.length" class="m-0 text-sm">
                    {{
                        formatMessage(messages.operationOptionalFor, {
                            names: node.optionalFor.join(', '),
                        })
                    }}
                </p>
                <p v-if="node.state === 'add' && planned" class="m-0 break-all text-sm">
                    {{ node.targetPath }}
                </p>
                <template
                    v-if="
                        planned &&
                        node.state === 'add' &&
                        node.version &&
                        (node.compatibility.gameVersion !== 'compatible' ||
                            node.compatibility.loader !== 'compatible')
                    "
                >
                    <p class="m-0 text-sm">
                        {{
                            formatMessage(messages.operationAssessment, {
                                game: compatibilityLabel(node.compatibility.gameVersion),
                                loader: compatibilityLabel(node.compatibility.loader),
                            })
                        }}
                    </p>
                    <template
                        v-for="dimension in ['gameVersion', 'loader'] as const"
                        :key="dimension"
                    >
                        <Button
                            v-if="
                                node.compatibility[dimension] !== 'compatible' &&
                                !exceptions.has(`${node.id}:${dimension}`)
                            "
                            :disabled="busy"
                            @click="accept(node, dimension)"
                            >{{
                                formatMessage(messages.operationAcceptDimension, { dimension })
                            }}</Button
                        >
                    </template>
                </template>
            </div>
            <template v-if="candidates.length && planned">
                <h3 class="m-0">{{ formatMessage(messages.operationOptional) }}</h3>
                <div
                    v-for="node in candidates"
                    :key="node.id"
                    class="flex items-center gap-2 rounded-lg border border-surface-4 p-3"
                >
                    <Avatar :src="node.iconUrl ?? undefined" :alt="node.title" size="32px" />
                    <div class="flex min-w-0 grow flex-col gap-1">
                        <span>{{ node.title }}</span>
                        <span
                            v-if="node.optionalFor.length"
                            class="text-sm text-[var(--color-text-tertiary)]"
                        >
                            {{
                                formatMessage(messages.operationOptionalFor, {
                                    names: node.optionalFor.join(', '),
                                })
                            }}
                        </span>
                    </div>
                    <Button
                        size="sm"
                        :disabled="busy"
                        :aria-label="formatMessage(messages.operationAdd, { name: node.title })"
                        @click="addCandidate(node)"
                        >＋</Button
                    >
                </div>
            </template>
            <div
                v-if="busy && !queueing"
                class="flex flex-col gap-3"
                role="status"
                data-operation-skeleton
            >
                <span class="sr-only">{{ formatMessage(messages.operationLoading) }}</span>
                <div
                    class="flex flex-col gap-2 rounded-lg border border-surface-4 bg-surface-2 p-3"
                    aria-hidden="true"
                >
                    <div class="flex items-center gap-2">
                        <div class="size-8 shrink-0 animate-pulse rounded-full bg-surface-3" />
                        <div class="h-4 w-2/5 animate-pulse rounded bg-surface-3" />
                    </div>
                    <div class="h-3 w-3/5 animate-pulse rounded bg-surface-3" />
                </div>
            </div>
            <p v-if="planned && !busy && !actions.length && !blocking" class="m-0">
                {{ formatMessage(messages.operationEmpty) }}
            </p>
        </div>
        <template #actions>
            <div class="flex items-center justify-end gap-2">
                <Button :disabled="busy" @click="modal?.hide()">{{
                    formatMessage(messages.operationCancel)
                }}</Button>
                <Button color="brand" :disabled="!canApply" @click="apply">{{
                    formatMessage(messages.operationApply)
                }}</Button>
            </div>
        </template>
    </PreviewModal>
</template>
