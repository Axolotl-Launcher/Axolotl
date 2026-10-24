<script setup lang="ts">
import { Avatar, Button, Combobox, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, triggerRef } from 'vue'

import type { InstallJobSnapshot } from '@/helpers/install'
import { refresh_content } from '@/helpers/instance'
import { resourceOperationError, resourceSourceName } from '@/helpers/resource-operation-logic'
import {
    type ResourceDependencyNode,
    resourceGet,
    type ResourceOperationIntent,
    type ResourceOperationPlan,
    type ResourceOperationProgress,
    type ResourcePlanDecision,
    resourcePrepareEntryOperation,
    resourcePrepareOperation,
    resourceProjectDocuments,
    resourceQueuePlan,
    resourceRevisePlan,
    type ResourceSelection,
    type SourceReleaseRef,
} from '@/helpers/resources'
import { get_instance_worlds } from '@/helpers/worlds'
import type { ContentAnalysisReview } from '@/providers/content-analysis'

const { formatMessage } = useVIntl()
const messages = defineMessages({
    invalidPath: {
        id: 'app.resources.operation.invalid-path',
        defaultMessage: 'The file path is invalid or cannot be used for this installation.',
    },
    identityUnconfirmed: {
        id: 'app.resources.operation.identity-unconfirmed',
        defaultMessage: 'The selected resource could not be identified reliably.',
    },
    staleRevision: {
        id: 'app.resources.operation.stale-revision',
        defaultMessage: 'The instance changed. Refresh this preview before continuing.',
    },
    world: { id: 'app.resources.operation.world', defaultMessage: 'Target world for data packs' },
    noWorlds: {
        id: 'app.resources.operation.no-worlds',
        defaultMessage:
            'Create a singleplayer world in this instance before installing data packs.',
    },
    header: { id: 'app.resources.operation.header', defaultMessage: 'Review content changes' },
    loading: {
        id: 'app.resources.operation.loading',
        defaultMessage: 'Resolving releases and dependencies…',
    },
    artifact: {
        id: 'app.resources.operation.artifact',
        defaultMessage: '{source} · release {release} · {file}',
    },
    required: { id: 'app.resources.operation.required', defaultMessage: 'Required dependencies' },
    optional: { id: 'app.resources.operation.optional', defaultMessage: 'Available dependencies' },
    satisfied: { id: 'app.resources.operation.satisfied', defaultMessage: 'Already satisfied' },
    disabledDependency: {
        id: 'app.resources.operation.disabled-dependency',
        defaultMessage:
            'This installed dependency is disabled. Enable it or review its version before continuing.',
    },
    installedDependencyUnavailable: {
        id: 'app.resources.operation.installed-dependency-unavailable',
        defaultMessage:
            'An installed copy is disabled, missing, or does not satisfy the required release. Review that copy before continuing.',
    },
    changes: {
        id: 'app.resources.operation.changes',
        defaultMessage:
            '{count, plural, one {# file will change} other {# files will change}} in {instance}.',
    },
    add: { id: 'app.resources.operation.add', defaultMessage: 'Add {name}' },
    remove: { id: 'app.resources.operation.remove', defaultMessage: 'Remove {name}' },
    requiredBy: {
        id: 'app.resources.operation.required-by',
        defaultMessage: 'Required by {names}',
    },
    optionalFor: {
        id: 'app.resources.operation.optional-for',
        defaultMessage: 'Optional dependency of {names}',
    },
    acceptDimension: {
        id: 'app.resources.operation.accept-dimension',
        defaultMessage:
            '{dimension, select, gameVersion {Accept Minecraft compatibility exception} other {Accept loader compatibility exception}}',
    },
    assessment: {
        id: 'app.resources.operation.assessment',
        defaultMessage: 'Game version: {game}; loader: {loader}',
    },
    state: {
        id: 'app.resources.operation.compatibility',
        defaultMessage:
            '{state, select, compatible {Compatible} incompatible {Incompatible} other {Unknown}}',
    },
    retry: { id: 'app.resources.operation.retry', defaultMessage: 'Retry' },
    refresh: { id: 'app.resources.operation.refresh', defaultMessage: 'Refresh this preview' },
    cancel: { id: 'app.resources.operation.cancel', defaultMessage: 'Cancel' },
    apply: { id: 'app.resources.operation.apply', defaultMessage: 'Queue changes' },
    empty: { id: 'app.resources.operation.empty', defaultMessage: 'No file changes are needed.' },
    manualDownload: {
        id: 'app.resources.operation.manual-download',
        defaultMessage: '{name} requires a manual download and cannot be installed automatically.',
    },
    pathOccupied: {
        id: 'app.resources.operation.path-occupied',
        defaultMessage: '{path} is already used by another file, so it cannot be installed.',
    },
    sourceUnavailable: {
        id: 'app.resources.operation.source-unavailable',
        defaultMessage: 'The source cannot provide this item right now.',
    },
    unresolvedOptional: {
        id: 'app.resources.operation.unresolved-optional',
        defaultMessage: 'Some optional dependencies could not be resolved.',
    },
    missingDependency: {
        id: 'app.resources.operation.missing-dependency',
        defaultMessage: 'A required dependency could not be resolved.',
    },
    dependencyAnalysisFailed: {
        id: 'app.resources.operation.dependency-analysis-failed',
        defaultMessage: 'Dependency analysis failed. The instance may not work correctly.',
    },
    unselectedDependency: {
        id: 'app.resources.operation.unselected-dependency',
        defaultMessage:
            'Automatic dependency installation is off. Add this required dependency to continue.',
    },
    versionConflict: {
        id: 'app.resources.operation.version-conflict',
        defaultMessage: 'A version conflict prevents installing this item.',
    },
    artifactMismatch: {
        id: 'app.resources.operation.artifact-mismatch',
        defaultMessage: 'The selected installed entry does not match this item.',
    },
    installSelectedOnly: {
        id: 'app.resources.operation.install-selected-only',
        defaultMessage: 'Force install selected content only',
    },
    selectedOnly: {
        id: 'app.resources.operation.selected-only',
        defaultMessage:
            'Only the selected content will be installed. Dependencies will not be checked or installed.',
    },
    skipUnwritable: {
        id: 'app.resources.operation.skip-unwritable',
        defaultMessage: 'Force install writable files',
    },
    fileNotWritable: {
        id: 'app.resources.operation.file-not-writable',
        defaultMessage: 'Cannot write {path}. Close any program using this file or skip it.',
    },
    fileWriteSkipped: {
        id: 'app.resources.operation.file-write-skipped',
        defaultMessage: '{path} will be skipped because it cannot be written.',
    },
    noWritableFiles: {
        id: 'app.resources.operation.no-writable-files',
        defaultMessage: 'No writable files remain to install.',
    },
})

type OperationInput = {
    analysis?: ContentAnalysisReview
    importFiles?: Record<string, string>
    locators?: Record<string, string>
    instanceId: string
    instanceName: string
    entryIds: string[]
    selections?: ResourceSelection[]
    intent: ResourceOperationIntent
    release?: SourceReleaseRef | null
    installDependencies: boolean
}
const modal = ref<InstanceType<typeof NewModal> | null>(null)
const plan = ref<ResourceOperationPlan | null>(null)
const progressNodes = shallowRef(new Map<string, ResourceDependencyNode>())
const displayedNodes = computed(() => {
    const nodes = new Map((plan.value?.nodes ?? []).map((node) => [node.id, node]))
    for (const [id, node] of progressNodes.value) nodes.set(id, node)
    return [...nodes.values()]
})
const input = ref<OperationInput | null>(null)
const busy = ref(false)
const worldOptions = ref<{ value: string; label: string }[]>([])
const targetWorld = ref<string | null>(null)
const needsWorld = computed(
    () =>
        input.value?.selections?.some(
            (selection) =>
                selection.resourceType === 'dataPack' && selection.scope.kind === 'instance',
        ) ?? false,
)
const queueing = ref(false)
const closing = ref(false)
const queueFailed = ref(false)
const dependenciesIgnored = ref(false)
const error = ref<string | null>(null)
const names = ref<Record<string, { title: string; iconUrl: string | null; summary?: string }>>({})
let controller: AbortController | null = null
let generation = 0
let resolveShow: ((job: InstallJobSnapshot | null) => void) | null = null
const submittedRoots = ref<string[]>([])
let retryAction: (() => Promise<void>) | null = null

const active = computed(() => {
    return displayedNodes.value.filter((node) => node.state !== 'optionalCandidate')
})
const candidates = computed(() =>
    displayedNodes.value.filter((node) => node.state === 'optionalCandidate'),
)
type PlanIssue = ResourceOperationPlan['issues'][number]
type Notice = { text: string; blocking: boolean }
function issueNotice(issue: PlanIssue): Notice | null {
    if (issue.code === 'compatibilityUnknown' || issue.code === 'compatibilityMismatch') return null
    const node = plan.value?.nodes.find((candidate) => candidate.id === issue.selectionId)
    const text = (() => {
        switch (issue.code) {
            case 'fileNotWritable':
                return formatMessage(messages.fileNotWritable, { path: issue.message })
            case 'fileWriteSkipped':
                return formatMessage(messages.fileWriteSkipped, { path: issue.message })
            case 'manualDownloadRequired':
                return formatMessage(messages.manualDownload, {
                    name: node ? name(node) : (issue.selectionId ?? ''),
                })
            case 'invalidPath':
                return issue.message.startsWith('target already exists: ')
                    ? formatMessage(messages.pathOccupied, {
                          path: issue.message.slice('target already exists: '.length),
                      })
                    : formatMessage(messages.invalidPath)
            case 'sourceUnavailable':
                return issue.blocking
                    ? formatMessage(messages.sourceUnavailable)
                    : formatMessage(messages.unresolvedOptional)
            case 'missingDependency':
                if (node?.state === 'missing') return formatMessage(messages.unselectedDependency)
                return formatMessage(messages.missingDependency)
            case 'dependencyAnalysisFailed':
                return formatMessage(messages.dependencyAnalysisFailed)
            case 'versionConflict':
                if (
                    issue.message ===
                    'installed dependency is disabled, missing or does not satisfy this release; review the installed entry'
                )
                    return formatMessage(messages.installedDependencyUnavailable)
                return issue.blocking
                    ? formatMessage(messages.versionConflict)
                    : formatMessage(messages.unresolvedOptional)
            case 'artifactMismatch':
                return formatMessage(messages.artifactMismatch)
            case 'identityUnconfirmed':
                return formatMessage(messages.identityUnconfirmed)
            case 'staleRevision':
                return formatMessage(messages.staleRevision)
            default:
                return resourceOperationError(issue.message, formatMessage)
        }
    })()
    return { text, blocking: issue.blocking }
}
const notices = computed(() => {
    const nodes = plan.value?.nodes ?? []
    const bound = new Map<string, Notice[]>()
    const other: Notice[] = []
    for (const issue of plan.value?.issues ?? []) {
        const notice = issueNotice(issue)
        if (!notice) continue
        const node = nodes.find((candidate) => candidate.id === issue.selectionId)
        if (!node) {
            other.push(notice)
            continue
        }
        const list = bound.get(node.id) ?? []
        if (!list.some((existing) => existing.text === notice.text)) list.push(notice)
        bound.set(node.id, list)
    }
    return { bound, other }
})
function noticesFor(node: ResourceDependencyNode) {
    return notices.value.bound.get(node.id) ?? []
}
function noticeClass(notice: Notice) {
    return notice.blocking ? 'text-[var(--color-red)]' : 'text-[var(--color-text-tertiary)]'
}
const canApply = computed(
    () =>
        !busy.value &&
        !queueing.value &&
        !dependencyAnalysisFailed.value &&
        (plan.value?.actions.length ?? 0) > 0 &&
        !plan.value?.issues.some((issue) => issue.blocking),
)
const dependencyAnalysisFailed = computed(
    () => plan.value?.issues.some((issue) => issue.code === 'dependencyAnalysisFailed') ?? false,
)
const canIgnoreDependencies = computed(() => {
    if (busy.value || queueing.value || dependenciesIgnored.value) return false
    if (!plan.value)
        return !!error.value && !queueFailed.value && (!needsWorld.value || !!targetWorld.value)
    return (
        plan.value.issues.some(
            (issue) =>
                issue.code === 'dependencyAnalysisFailed' ||
                (issue.blocking && ['versionConflict', 'missingDependency'].includes(issue.code)),
        ) &&
        !plan.value.issues.some(
            (issue) =>
                issue.blocking &&
                ![
                    'dependencyAnalysisFailed',
                    'versionConflict',
                    'missingDependency',
                    'fileNotWritable',
                ].includes(issue.code),
        )
    )
})
const canSkipUnwritable = computed(
    () =>
        !busy.value &&
        !queueing.value &&
        !dependencyAnalysisFailed.value &&
        plan.value?.issues.some((issue) => issue.code === 'fileNotWritable') &&
        !plan.value.issues.some((issue) => issue.blocking && issue.code !== 'fileNotWritable'),
)
const hasSkippedWrites = computed(
    () => plan.value?.issues.some((issue) => issue.code === 'fileWriteSkipped') ?? false,
)
function name(node: ResourceDependencyNode) {
    return (
        names.value[node.id]?.title ??
        node.artifact?.fileName ??
        (node.resource.kind === 'source' ? node.resource.project.projectId : node.resource.id)
    )
}
function explicitRoots(node: ResourceDependencyNode): string[] {
    return node.rootSelectionIds?.length
        ? node.rootSelectionIds
        : plan.value?.activeRoots.includes(node.id)
          ? [node.id]
          : []
}
function requiredBy(id: string, kind = 'required') {
    return [
        ...new Set(
            (plan.value?.edges ?? [])
                .filter((edge) => edge.child === id && edge.kind === kind)
                .map((edge) => plan.value?.nodes.find((node) => node.id === edge.parent))
                .filter((node): node is ResourceDependencyNode => !!node)
                .map(name),
        ),
    ].join(', ')
}
async function enrich(value: ResourceOperationPlan, signal: AbortSignal, current: number) {
    const refs = value.nodes
        .filter((node) => node.state !== 'satisfied' && !names.value[node.id])
        .flatMap((node) => {
            const project =
                node.artifact?.reference.release.project ??
                (node.resource.kind === 'source' ? node.resource.project : null)
            return project ? [project] : []
        })
    await Promise.allSettled(
        [...new Set(refs.map((project) => project.source))]
            .filter((source) => source === 'modrinth' || source === 'curseforge')
            .map((source) =>
                resourceProjectDocuments(
                    source,
                    [
                        ...new Set(
                            refs
                                .filter((project) => project.source === source)
                                .map((project) => project.projectId),
                        ),
                    ],
                    signal,
                ),
            ),
    )
    if (current !== generation || signal.aborted) return
    await Promise.allSettled(
        value.nodes.map(async (node) => {
            if (names.value[node.id]) return
            const cachedRef =
                node.artifact?.reference.release.project ??
                (node.resource.kind === 'source' ? node.resource.project : null)
            if (cachedRef?.source === 'local') return
            if (node.state === 'satisfied' && cachedRef) {
                const [document] = await resourceProjectDocuments(
                    cachedRef.source,
                    [cachedRef.projectId],
                    signal,
                    'onlyCached',
                )
                if (document && current === generation && !signal.aborted)
                    names.value = {
                        ...names.value,
                        [node.id]: {
                            title: document.metadata.title,
                            iconUrl: document.metadata.iconUrl,
                            summary: document.metadata.summary,
                        },
                    }
                return
            }
            const resource = await resourceGet(node.resource, undefined, undefined, signal)
            if (current !== generation || signal.aborted) return
            const project =
                resource.projects.find(
                    (project) =>
                        project.project.source === node.artifact?.reference.release.project.source,
                ) ?? resource.projects[0]
            if (project)
                names.value = {
                    ...names.value,
                    [node.id]: {
                        title: project.title,
                        iconUrl: project.iconUrl,
                        summary: project.summary,
                    },
                }
        }),
    )
}
async function run(
    action: (
        signal: AbortSignal,
        onProgress: (progress: ResourceOperationProgress) => void,
    ) => Promise<ResourceOperationPlan>,
    ignoreDependencies = dependenciesIgnored.value,
) {
    const current = generation
    controller?.abort()
    controller = new AbortController()
    const signal = controller.signal
    busy.value = true
    progressNodes.value = new Map()
    error.value = null
    retryAction = () => run(action, ignoreDependencies)
    let acceptingProgress = true
    try {
        const value = await action(signal, (progress) => {
            if (!acceptingProgress || current !== generation || signal.aborted) return
            progressNodes.value.set(progress.node.id, progress.node)
            triggerRef(progressNodes)
            if (progress.metadata)
                names.value[progress.node.id] = {
                    title: progress.metadata.title,
                    iconUrl: progress.metadata.iconUrl,
                    summary:
                        progress.node.state === 'satisfied' ? undefined : progress.metadata.summary,
                }
        })
        if (current !== generation || signal.aborted) return
        plan.value = value
        dependenciesIgnored.value = ignoreDependencies
        void enrich(value, signal, current)
        return value
    } catch (failure) {
        if (current === generation && !signal.aborted)
            error.value = resourceOperationError(failure, formatMessage)
    } finally {
        acceptingProgress = false
        if (current === generation && !signal.aborted) {
            busy.value = false
            progressNodes.value = new Map()
        }
    }
}
async function revise(decision: ResourcePlanDecision) {
    const value = plan.value
    if (!value || busy.value) return
    return await run(
        (signal, onProgress) =>
            resourceRevisePlan(value.id, value.revision, [decision], signal, onProgress),
        decision.kind === 'ignoreDependencies' || dependenciesIgnored.value,
    )
}
function hide() {
    generation++
    input.value?.analysis?.signal.removeEventListener('abort', invalidateAnalysis)
    controller?.abort()
    const resolve = resolveShow
    resolveShow = null
    resolve?.(null)
}
function show(value: OperationInput): Promise<InstallJobSnapshot | null> {
    if (queueing.value) return Promise.resolve(null)
    hide()
    input.value = value
    plan.value = null
    progressNodes.value = new Map()
    submittedRoots.value = []
    names.value = {}
    error.value = null
    queueFailed.value = false
    dependenciesIgnored.value = false
    retryAction = null
    worldOptions.value = []
    targetWorld.value = null
    modal.value?.show()
    value.analysis?.signal.addEventListener('abort', invalidateAnalysis, { once: true })
    const result = new Promise<InstallJobSnapshot | null>((resolve) => {
        resolveShow = resolve
    })
    if (needsWorld.value) {
        const current = generation
        busy.value = true
        const loadWorlds = async () => {
            busy.value = true
            error.value = null
            await get_instance_worlds(value.instanceId)
                .then((worlds) => {
                    if (current !== generation) return
                    worldOptions.value = worlds.flatMap((world) =>
                        world.type === 'singleplayer'
                            ? [
                                  {
                                      value: `saves/${world.path.replace(/^saves\//, '')}`,
                                      label: world.name,
                                  },
                              ]
                            : [],
                    )
                })
                .catch((failure) => {
                    if (current === generation)
                        error.value = resourceOperationError(failure, formatMessage)
                })
                .finally(() => {
                    if (current === generation) busy.value = false
                })
        }
        retryAction = loadWorlds
        void loadWorlds()
        return result
    }
    void prepareInput(value)
    return result
}
function invalidateAnalysis() {
    void cancel()
}
async function prepareInput(
    value: OperationInput,
    resolveDependencies = !dependenciesIgnored.value,
) {
    return await run(
        (signal, onProgress) =>
            value.analysis && resolveDependencies
                ? value.analysis.prepare(signal, onProgress, value.selections)
                : value.selections
                  ? resourcePrepareOperation(
                        value.instanceId,
                        'install',
                        value.selections,
                        value.installDependencies,
                        signal,
                        value.importFiles,
                        value.locators,
                        onProgress,
                        resolveDependencies,
                    )
                  : resourcePrepareEntryOperation(
                        value.instanceId,
                        value.intent,
                        value.entryIds,
                        value.release ?? null,
                        value.installDependencies,
                        signal,
                        onProgress,
                        resolveDependencies,
                    ),
        !resolveDependencies,
    )
}
async function ignoreDependencies() {
    if (plan.value) await revise({ kind: 'ignoreDependencies' })
    else if (input.value) await prepareInput(withTargetWorld(input.value), false)
}
function withTargetWorld(value: OperationInput): OperationInput {
    return {
        ...value,
        selections: value.selections?.map((selection) =>
            selection.resourceType === 'dataPack' && targetWorld.value
                ? {
                      ...selection,
                      scope: { kind: 'world' as const, relativePath: targetWorld.value },
                  }
                : selection,
        ),
    }
}
function selectWorld(path: string) {
    if (!input.value || busy.value) return
    targetWorld.value = path
    plan.value = null
    void prepareInput(withTargetWorld(input.value))
}
async function apply() {
    if (!plan.value || !canApply.value) return
    const reviewed = plan.value
    const current = generation
    const resolve = resolveShow
    busy.value = true
    queueing.value = true
    error.value = null
    try {
        const job = await resourceQueuePlan(reviewed.id, reviewed.revision)
        if (generation !== current) return
        const skippedRoots = new Set(
            reviewed.issues
                .filter((issue) => issue.code === 'fileWriteSkipped')
                .flatMap((issue) => {
                    const node = reviewed.nodes.find((node) => node.id === issue.selectionId)
                    return node?.rootSelectionIds?.length
                        ? node.rootSelectionIds
                        : issue.selectionId
                          ? [issue.selectionId]
                          : []
                }),
        )
        submittedRoots.value = reviewed.activeRoots.filter((root) => !skippedRoots.has(root))
        resolveShow = null
        closing.value = true
        await nextTick()
        await modal.value?.hide()
        resolve?.(job)
    } catch (failure) {
        if (generation !== current) return
        if (resourceOperationError(failure).includes('instance changed after review')) {
            busy.value = false
            queueing.value = false
            await refresh()
            return
        }
        error.value = resourceOperationError(failure, formatMessage)
        retryAction = apply
        queueFailed.value = true
    } finally {
        if (generation === current || (closing.value && generation === current + 1)) {
            busy.value = false
            queueing.value = false
            closing.value = false
        }
    }
}
async function refresh() {
    const current = plan.value
    const value = input.value
    if (!current || !value || busy.value) return
    queueFailed.value = false
    await run(async (signal, onProgress) => {
        await refresh_content(value.instanceId)
        signal.throwIfAborted()
        const prepared = await resourcePrepareOperation(
            value.instanceId,
            current.intent,
            current.selections,
            value.installDependencies,
            signal,
            value.importFiles,
            value.locators,
            onProgress,
            !dependenciesIgnored.value,
        )
        const retainedExceptions = current.exceptions.filter((exception) =>
            prepared.nodes.some(
                (node) =>
                    node.id === exception.selectionId &&
                    node.artifact?.reference.release.project.source ===
                        exception.release.project.source &&
                    node.artifact.reference.release.project.projectId ===
                        exception.release.project.projectId &&
                    node.artifact.reference.release.releaseId === exception.release.releaseId,
            ),
        )
        if (!retainedExceptions.length) return prepared
        return resourceRevisePlan(
            prepared.id,
            prepared.revision,
            retainedExceptions.map((exception) => ({ kind: 'acceptCompatibility', exception })),
            signal,
            onProgress,
        )
    })
}
function accept(node: ResourceDependencyNode, dimension: 'gameVersion' | 'loader') {
    if (!node.artifact) return
    void revise({
        kind: 'acceptCompatibility',
        exception: {
            selectionId: node.id,
            release: node.artifact.reference.release,
            dimensions: [dimension],
        },
    })
}
onBeforeUnmount(hide)
function showInstall(value: {
    analysis?: ContentAnalysisReview
    importFiles?: Record<string, string>
    locators?: Record<string, string>
    instanceId: string
    instanceName: string
    selections: ResourceSelection[]
    installDependencies: boolean
}) {
    return show({ ...value, intent: 'install', entryIds: [] })
}
async function showInstallSelection(value: Parameters<typeof showInstall>[0]) {
    const job = await showInstall(value)
    return job ? { job, activeRoots: [...submittedRoots.value] } : null
}
async function cancel() {
    if (queueing.value) return
    hide()
    const current = generation
    busy.value = false
    plan.value = null
    input.value = null
    progressNodes.value = new Map()
    await nextTick()
    if (generation !== current) return
    await modal.value?.hide()
}
defineExpose({ show, showInstall, showInstallSelection, cancel })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.header)"
        :on-hide="hide"
        :disable-close="busy && !closing"
        scrollable
        width="min(42rem, 90vw)"
    >
        <div class="flex flex-col gap-3">
            <div v-if="needsWorld" class="flex flex-col gap-2">
                <label>{{ formatMessage(messages.world) }}</label>
                <Combobox
                    :model-value="targetWorld"
                    :options="worldOptions"
                    :disabled="busy"
                    @update:model-value="selectWorld"
                />
                <p v-if="!busy && !worldOptions.length" class="m-0">
                    {{ formatMessage(messages.noWorlds) }}
                </p>
            </div>
            <p v-if="plan && !busy" class="m-0">
                {{
                    formatMessage(messages.changes, {
                        count: plan.actions.length,
                        instance: input?.instanceName ?? '',
                    })
                }}
            </p>
            <p
                v-if="dependenciesIgnored && plan && !busy"
                class="m-0 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.selectedOnly) }}
            </p>
            <div v-if="error" class="flex flex-col gap-2" role="alert">
                <p class="m-0">{{ error }}</p>
                <Button v-if="retryAction" :disabled="busy" @click="retryAction?.()">{{
                    formatMessage(messages.retry)
                }}</Button>
                <Button v-if="queueFailed" :disabled="busy" @click="refresh">{{
                    formatMessage(messages.refresh)
                }}</Button>
            </div>
            <div
                v-for="node in active"
                :key="node.id"
                class="flex flex-col gap-2 rounded-lg border border-surface-4 bg-surface-2 p-3"
            >
                <div class="flex items-center gap-2">
                    <Avatar
                        :src="names[node.id]?.iconUrl ?? undefined"
                        :alt="name(node)"
                        size="32px"
                    />
                    <span class="grow font-semibold">{{ name(node) }}</span>
                    <Button
                        v-for="rootId in explicitRoots(node)"
                        :key="rootId"
                        size="sm"
                        :disabled="busy"
                        :aria-label="formatMessage(messages.remove, { name: name(node) })"
                        @click="revise({ kind: 'removeRoot', selectionId: rootId })"
                        >×</Button
                    >
                </div>
                <p
                    v-if="node.state !== 'satisfied' && names[node.id]?.summary"
                    class="m-0 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ names[node.id].summary }}
                </p>
                <p v-if="node.state !== 'satisfied' && node.artifact" class="m-0 break-all text-sm">
                    {{
                        formatMessage(messages.artifact, {
                            source: resourceSourceName(
                                node.artifact.reference.release.project.source,
                            ),
                            release: node.artifact.reference.release.releaseId,
                            file: node.artifact.fileName ?? '',
                        })
                    }}
                </p>
                <p v-if="node.scope.kind === 'world'" class="m-0 break-all text-sm">
                    {{ node.scope.relativePath }}
                </p>
                <p v-if="node.state === 'satisfied'" class="m-0 text-sm">
                    {{ formatMessage(messages.satisfied) }}
                </p>
                <p v-if="node.state === 'disabled'" class="m-0 text-sm">
                    {{ formatMessage(messages.disabledDependency) }}
                </p>
                <Button
                    v-if="node.state === 'missing'"
                    size="sm"
                    :disabled="busy"
                    :aria-label="formatMessage(messages.add, { name: name(node) })"
                    @click="revise({ kind: 'addRequired', nodeId: node.id })"
                    >＋</Button
                >
                <p v-if="requiredBy(node.id)" class="m-0 text-sm">
                    {{ formatMessage(messages.requiredBy, { names: requiredBy(node.id) }) }}
                </p>
                <p v-else-if="requiredBy(node.id, 'optional')" class="m-0 text-sm">
                    {{
                        formatMessage(messages.optionalFor, {
                            names: requiredBy(node.id, 'optional'),
                        })
                    }}
                </p>
                <p
                    v-for="(notice, index) in noticesFor(node)"
                    :key="index"
                    class="m-0 text-sm"
                    :class="noticeClass(notice)"
                >
                    {{ notice.text }}
                </p>
                <p
                    v-for="action in plan?.actions.filter((action) => action.nodeId === node.id)"
                    :key="action.entryId"
                    class="m-0 break-all text-sm"
                >
                    {{ action.targetRelativePath }}
                </p>
                <template
                    v-if="
                        node.artifact &&
                        (node.compatibility.gameVersion !== 'compatible' ||
                            node.compatibility.loader !== 'compatible')
                    "
                >
                    <p class="m-0 text-sm">
                        {{
                            formatMessage(messages.assessment, {
                                game: formatMessage(messages.state, {
                                    state: node.compatibility.gameVersion,
                                }),
                                loader: formatMessage(messages.state, {
                                    state: node.compatibility.loader,
                                }),
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
                                !plan?.exceptions.some(
                                    (exception) =>
                                        exception.selectionId === node.id &&
                                        exception.dimensions.includes(dimension),
                                )
                            "
                            :disabled="busy"
                            @click="accept(node, dimension)"
                            >{{ formatMessage(messages.acceptDimension, { dimension }) }}</Button
                        >
                    </template>
                </template>
            </div>
            <template v-if="candidates.length">
                <h3 class="m-0">{{ formatMessage(messages.optional) }}</h3>
                <div
                    v-for="node in candidates"
                    :key="node.id"
                    class="flex items-center gap-2 rounded-lg border border-surface-4 p-3"
                >
                    <Avatar
                        :src="names[node.id]?.iconUrl ?? undefined"
                        :alt="name(node)"
                        size="32px"
                    />
                    <div class="flex min-w-0 grow flex-col gap-1">
                        <span>{{ name(node) }}</span>
                        <span
                            v-if="requiredBy(node.id, 'optional')"
                            class="text-sm text-[var(--color-text-tertiary)]"
                        >
                            {{
                                formatMessage(messages.optionalFor, {
                                    names: requiredBy(node.id, 'optional'),
                                })
                            }}
                        </span>
                    </div>
                    <Button
                        size="sm"
                        :disabled="busy"
                        :aria-label="formatMessage(messages.add, { name: name(node) })"
                        @click="revise({ kind: 'addOptional', nodeId: node.id })"
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
                <span class="sr-only">{{ formatMessage(messages.loading) }}</span>
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
            <p
                v-for="(notice, index) in notices.other"
                :key="index"
                class="m-0 text-sm"
                :class="noticeClass(notice)"
            >
                {{ notice.text }}
            </p>
            <p
                v-if="
                    plan &&
                    !busy &&
                    !plan.actions.length &&
                    !plan.issues.some((issue) => issue.blocking)
                "
                class="m-0"
            >
                {{ formatMessage(hasSkippedWrites ? messages.noWritableFiles : messages.empty) }}
            </p>
        </div>
        <template #actions>
            <div class="flex items-center justify-end gap-2">
                <Button :disabled="busy" @click="modal?.hide()">{{
                    formatMessage(messages.cancel)
                }}</Button>
                <Button v-if="canIgnoreDependencies" color="orange" @click="ignoreDependencies">
                    {{ formatMessage(messages.installSelectedOnly) }}
                </Button>
                <Button
                    v-if="canSkipUnwritable"
                    color="orange"
                    @click="revise({ kind: 'skipUnwritableFiles' })"
                >
                    {{ formatMessage(messages.skipUnwritable) }}
                </Button>
                <Button
                    v-tooltip="
                        dependencyAnalysisFailed
                            ? formatMessage(messages.dependencyAnalysisFailed)
                            : undefined
                    "
                    color="brand"
                    :disabled="!canApply"
                    @click="apply"
                    >{{ formatMessage(messages.apply) }}</Button
                >
            </div>
        </template>
    </NewModal>
</template>
