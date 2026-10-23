<script setup lang="ts">
import SwapIcon from '@modrinth/assets/icons/arrow-left-right.svg?component'
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ChevronLeftIcon from '@modrinth/assets/icons/chevron-left.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import FileIcon from '@modrinth/assets/icons/file.svg?component'
import MoreVerticalIcon from '@modrinth/assets/icons/more-vertical.svg?component'
import ReportIcon from '@modrinth/assets/icons/report.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import AutoLink from '@modrinth/ui/src/components/base/AutoLink.vue'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import ButtonLink from '@modrinth/ui/src/components/base/buttons/ButtonLink.vue'
import OverflowMenu from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Table from '@modrinth/ui/src/components/base/Table.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import TagTagItem from '@modrinth/ui/src/components/base/TagTagItem.vue'
import VersionChannelTag from '@modrinth/ui/src/components/version/VersionChannelTag.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useCompactNumber, useFormatNumber } from '@modrinth/ui/src/composables/format-number.ts'
import { useRelativeTime } from '@modrinth/ui/src/composables/how-ago.ts'
import { defineMessage, defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import {
    commonMessages,
    projectCompatibilityMessages,
} from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, onScopeDispose, ref, shallowRef, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'

import type { ProjectDetail, ProjectVersion } from '../../../../data'
import { loadInstanceContent, projectIcon } from '../../../../data'
import { usePreview } from '../../../../lib/state'
import { injectPreviewWindow } from '../../../../lib/window'
import { messages as discoverMessages } from '../../messages'
import { useDiscover } from '../../state'
import { formatVersionsForDisplay, type GameVersionTag } from '../../versions'
import ReleaseChangelog from './ReleaseChangelog.vue'

/**
 * The version page: apps/app-frontend/src/pages/project/Version.vue around the shared
 * `VersionPage` (packages/ui/src/components/version/VersionPage.vue). Dependencies are
 * resolved against the snapshot (its project pages and the instance fixtures' resolved
 * pack files); dependencies the snapshot does not know are left out.
 */
const props = defineProps<{
    project: ProjectDetail
    versions: ProjectVersion[]
    gameVersions: GameVersionTag[]
    install: (versionId: string | null) => void
    installed: boolean
    installing: boolean
    installedVersion: string | null
}>()

const { formatMessage } = useVIntl()
const preview = usePreview()
const discover = useDiscover()
const route = useRoute()
const { layer } = injectPreviewWindow()

const formatRelativeTime = useRelativeTime()
const formatDateTime = useFormatDateTime({ timeStyle: 'short', dateStyle: 'long' })
const formatBytes = useFormatBytes()
const formatNumber = useFormatNumber()
const { formatCompactNumber } = useCompactNumber()

const messages = defineMessages({
    compatibility: { id: 'version.section.compatibility', defaultMessage: 'Compatibility' },
    required: { id: 'version.section.required-content', defaultMessage: 'Required content' },
    optionalDeps: {
        id: 'version.section.optional-dependencies',
        defaultMessage: 'Optional dependencies',
    },
    knownIncompatibilities: {
        id: 'version.section.known-incompatibilities',
        defaultMessage: 'Known incompatibilities',
    },
    content: { id: 'version.section.content', defaultMessage: 'Content' },
    includedContent: { id: 'version.section.included-content', defaultMessage: 'Included content' },
    noModpackLoader: {
        id: 'version.section.no-modpack-mod-loader',
        defaultMessage: 'No mod loader',
    },
    changes: { id: 'version.section.changes', defaultMessage: 'Changes' },
    supplementaryResources: {
        id: 'version.section.supplementary-resources',
        defaultMessage: 'Supplementary resources',
    },
    searchContent: {
        id: 'version.section.content.search-placeholder',
        defaultMessage: 'Search content...',
    },
    anyVersion: { id: 'version.section.dependencies.any-version', defaultMessage: 'Any version' },
    anyCompatibleVersion: {
        id: 'version.section.dependencies.any-compatible-version',
        defaultMessage: 'Any compatible version',
    },
})

const version = computed(() =>
    props.versions.find((candidate) => candidate.id === route.params.version),
)

watch(
    () => version.value?.name,
    (name) => {
        if (name) preview.breadcrumbs.setName('Version', name)
    },
    { immediate: true },
)

function buildProjectHref(path: string) {
    const params = new URLSearchParams()
    for (const [key, value] of Object.entries(route.query)) {
        if (Array.isArray(value)) {
            for (const entry of value) if (entry != null) params.append(key, entry)
        } else if (value) {
            params.append(key, String(value))
        }
    }
    const query = params.toString()
    return query ? `${path}?${query}` : path
}

// Dependency enrichment ---------------------------------------------------------------------------
type Dependency = NonNullable<ProjectVersion['dependencies']>[number]
interface KnownProject {
    id: string
    title: string
    icon_url?: string
    versions: Map<string, string>
}
interface DependencyContext {
    dependency: Dependency
    project?: KnownProject
    versionNumber?: string
}

const enrichment = shallowRef<Map<string, KnownProject> | null>(null)
const enrichmentLoading = ref(false)
let enrichmentRequest = 0
onScopeDispose(() => {
    enrichmentRequest++
})

async function refreshEnrichment() {
    const request = ++enrichmentRequest
    enrichment.value = null
    const current = version.value
    if (!current) return
    const projectIds = new Set<string>()
    for (const dependency of current.dependencies ?? [])
        if (dependency.project_id) projectIds.add(dependency.project_id)
    if (projectIds.size === 0) {
        enrichment.value = new Map()
        return
    }
    enrichmentLoading.value = true
    try {
        const known = new Map<string, KnownProject>()
        const details = await Promise.all([...projectIds].map((id) => discover.ensureDetail(id)))
        for (const detail of details) {
            if (!detail) continue
            known.set(detail.id, {
                id: detail.id,
                title: detail.title,
                icon_url: projectIcon(detail.id),
                versions: new Map(detail.versions.map((entry) => [entry.id, entry.version_number])),
            })
        }
        if ([...projectIds].some((id) => !known.has(id))) {
            const content = await loadInstanceContent()
            const withoutIcon = new Set(content.withoutIcon)
            for (const rows of Object.values(content.instances)) {
                for (const [, , , projectId, versionId, versionNumber, title] of rows) {
                    if (!projectId || !title || !projectIds.has(projectId)) continue
                    let entry = known.get(projectId)
                    if (!entry) {
                        entry = {
                            id: projectId,
                            title,
                            icon_url: withoutIcon.has(projectId)
                                ? undefined
                                : projectIcon(projectId),
                            versions: new Map(),
                        }
                        known.set(projectId, entry)
                    }
                    if (versionId && versionNumber && !entry.versions.has(versionId))
                        entry.versions.set(versionId, versionNumber)
                }
            }
        }
        if (request === enrichmentRequest) enrichment.value = known
    } finally {
        if (request === enrichmentRequest) enrichmentLoading.value = false
    }
}

watch(
    () => version.value?.id,
    () => void refreshEnrichment(),
)
await refreshEnrichment()

const dependencies = computed<DependencyContext[]>(() =>
    (version.value?.dependencies ?? []).flatMap((dependency) => {
        const project = dependency.project_id
            ? enrichment.value?.get(dependency.project_id)
            : undefined
        if (dependency.project_id && !project && !enrichmentLoading.value) return []
        if (!dependency.project_id && !dependency.file_name) return []
        return [
            {
                dependency,
                project,
                versionNumber: dependency.version_id
                    ? project?.versions.get(dependency.version_id)
                    : undefined,
            },
        ]
    }),
)

const requiredContent = computed(() =>
    dependencies.value.filter((context) => context.dependency.dependency_type === 'required'),
)
const optionalContent = computed(() =>
    dependencies.value.filter((context) => context.dependency.dependency_type === 'optional'),
)
const incompatibleContent = computed(() =>
    dependencies.value.filter((context) => context.dependency.dependency_type === 'incompatible'),
)
const embeddedDependencies = computed(() =>
    dependencies.value.filter((context) => context.dependency.dependency_type === 'embedded'),
)

function dependencyLink(context: DependencyContext) {
    if (!context.project || !discover.details.value[context.project.id]) return undefined
    if (context.dependency.version_id && context.versionNumber)
        return buildProjectHref(
            `/project/${context.project.id}/version/${context.dependency.version_id}`,
        )
    return buildProjectHref(`/project/${context.project.id}`)
}

const includedContentLoading = computed(
    () =>
        enrichmentLoading.value &&
        embeddedDependencies.value.some(
            (context) => context.dependency.project_id && !context.project,
        ),
)

type IncludedContentLoadingRow = { loading: true; id: string }
type IncludedContentRow = {
    id: string
    icon_url?: string
    name?: string
    version?: string
    link?: string
    hasProject: boolean
}

function embeddedId(context: DependencyContext) {
    return (
        context.dependency.project_id ??
        context.dependency.file_name ??
        context.dependency.version_id ??
        'included'
    )
}

function isIncludedContentLoadingRow(
    row: IncludedContentLoadingRow | IncludedContentRow,
): row is IncludedContentLoadingRow {
    return 'loading' in row
}

const contentSearchQuery = ref('')

const includedContent = computed((): (IncludedContentLoadingRow | IncludedContentRow)[] => {
    if (includedContentLoading.value)
        return embeddedDependencies.value.map((context) => ({
            loading: true as const,
            id: embeddedId(context),
        }))
    return embeddedDependencies.value
        .map((context) => ({
            id: embeddedId(context),
            icon_url: context.project?.icon_url,
            name: context.project
                ? context.project.title
                : (context.dependency.file_name ?? undefined),
            version: context.versionNumber,
            link: dependencyLink(context),
            hasProject: !!context.project,
        }))
        .sort((a, b) => {
            const priority = (row: IncludedContentRow) => (row.hasProject ? 0 : row.link ? 1 : 2)
            const difference = priority(a) - priority(b)
            if (difference !== 0) return difference
            const sortName = (name: string | undefined) => (name ?? '').replace(/[^a-zA-Z0-9]/g, '')
            return sortName(a.name).localeCompare(sortName(b.name))
        })
})

const contentTableColumns = computed(() => {
    const columns: { key: string; label?: string; width?: string }[] = [
        { key: 'icon', width: '3.75rem' },
        {
            key: 'name',
            label: formatMessage(
                defineMessage({ id: 'version.content.name', defaultMessage: 'Name' }),
            ),
        },
    ]
    if (
        includedContentLoading.value
            ? embeddedDependencies.value.some((context) => context.dependency.version_id)
            : includedContent.value.some((row) => 'version' in row && row.version)
    ) {
        columns.push({
            key: 'version',
            label: formatMessage(
                defineMessage({ id: 'version.content.version', defaultMessage: 'Version' }),
            ),
        })
    }
    return columns
})

// Header ----------------------------------------------------------------------------------------------
const gameVersionsToDisplay = computed(() =>
    version.value
        ? props.gameVersions.length
            ? formatVersionsForDisplay(version.value.game_versions, props.gameVersions)
            : version.value.game_versions
        : [],
)
const platforms = computed(() => version.value?.loaders ?? [])
const primaryFile = computed(
    () => version.value?.files.find((file) => file.primary) ?? version.value?.files[0] ?? null,
)
const supplementaryResources = computed(() =>
    (version.value?.files ?? [])
        .filter((file) => file.primary === false && file !== primaryFile.value)
        .map((file) => ({ file })),
)
const supplementaryResourcesTableColumns = computed(() => [
    {
        key: 'file',
        label: formatMessage(
            defineMessage({ id: 'version.supplementary-resources.file', defaultMessage: 'File' }),
        ),
        width: '20rem',
    },
    {
        key: 'type',
        label: formatMessage(
            defineMessage({ id: 'version.supplementary-resources.type', defaultMessage: 'Type' }),
        ),
        width: '14rem',
    },
    {
        key: 'size',
        label: formatMessage(
            defineMessage({ id: 'version.supplementary-resources.size', defaultMessage: 'Size' }),
        ),
        width: '5rem',
    },
    { key: 'actions', label: '', width: '14rem' },
])

const formattedDownloads = computed(() => formatNumber(version.value?.downloads ?? 0))
const compactDownloads = computed(() => formatCompactNumber(version.value?.downloads ?? 0))

function capitalize(value: string) {
    return value ? value.charAt(0).toUpperCase() + value.slice(1) : value
}

/** A version file on Modrinth's CDN (what the app's `file.url` points to). */
function fileDownloadUrl(filename: string) {
    return `https://cdn.modrinth.com/data/${props.project.id}/versions/${version.value?.id}/${encodeURIComponent(filename)}`
}

const installedThis = computed(
    () => props.installed && props.installedVersion === version.value?.id,
)
</script>

<template>
    <div class="flex flex-col">
        <RouterLink
            class="mb-4 flex w-fit items-center gap-2 rounded-lg px-2 py-0.5 pl-0 text-link"
            :to="buildProjectHref(`/project/${route.params.id}/versions`)"
        >
            <ChevronLeftIcon class="shrink-0" /> {{ formatMessage(discoverMessages.allVersions) }}
        </RouterLink>
        <div v-if="version" class="flex flex-col gap-4">
            <div class="flex flex-wrap items-center justify-between gap-4">
                <div class="flex flex-col gap-1.5">
                    <div class="flex flex-wrap items-center gap-2">
                        <h2 class="m-0 font-semibold leading-tight">
                            {{ version.version_number }}
                        </h2>
                        <div>
                            <VersionChannelTag :channel="version.version_type" />
                        </div>
                    </div>
                    <div class="flex flex-row items-center gap-2">
                        <span>{{ version.name }}</span>
                        <span class="block size-1.5 rounded-full bg-surface-5" />
                        <span class="flex items-center gap-2">
                            <span v-tooltip="formatDateTime(version.date_published)">{{
                                formatRelativeTime(version.date_published)
                            }}</span>
                            <span class="size-1.5 rounded-full bg-surface-5" />
                            <span
                                v-tooltip="
                                    compactDownloads !== formattedDownloads
                                        ? capitalize(
                                              formatMessage(commonMessages.projectDownloads, {
                                                  count: version.downloads,
                                              }),
                                          )
                                        : undefined
                                "
                                class="flex items-center gap-1"
                            >
                                <DownloadIcon class="size-5" /> {{ compactDownloads }}
                            </span>
                        </span>
                    </div>
                </div>
                <div class="flex flex-wrap items-center gap-2">
                    <Button
                        type="colored"
                        color="brand"
                        :disabled="installing || installedThis"
                        @click="() => version && install(version.id)"
                        ><DownloadIcon v-if="!installed" />
                        <SwapIcon v-else-if="installedVersion !== version.id" />
                        <CheckIcon v-else />
                        {{
                            installing
                                ? formatMessage(discoverMessages.versionInstalling)
                                : installedThis
                                  ? formatMessage(commonMessages.installedLabel)
                                  : installed
                                    ? formatMessage(commonMessages.switchToVersionButton)
                                    : formatMessage(commonMessages.installButton)
                        }}
                    </Button>
                    <OverflowMenu
                        :tooltip="formatMessage(commonMessages.moreOptionsButton)"
                        :container="layer ?? undefined"
                        :options="[
                            {
                                id: 'open-in-browser',
                                link: `https://modrinth.com/${project.project_type}/${project.slug}/version/${version.id}`,
                                external: true,
                            },
                            {
                                id: 'report',
                                color: 'red',
                                hoverFilled: true,
                                link: `https://modrinth.com/report?item=version&itemID=${version.id}`,
                                external: true,
                            },
                        ]"
                        :aria-label="formatMessage(commonMessages.moreOptionsButton)"
                    >
                        <MoreVerticalIcon aria-hidden="true" />
                        <template #open-in-browser>
                            <ExternalIcon aria-hidden="true" />
                            {{ formatMessage(commonMessages.openInBrowserButton) }}
                        </template>
                        <template #report>
                            <ReportIcon aria-hidden="true" />
                            {{ formatMessage(commonMessages.reportButton) }}
                        </template>
                    </OverflowMenu>
                </div>
            </div>
            <hr class="m-0 h-[1px] w-full border-none bg-surface-4" />
            <section id="compatibility">
                <h3 class="mb-2 mt-0 text-lg font-semibold">
                    {{ formatMessage(messages.compatibility) }}
                </h3>
                <div class="grid grid-cols-2 gap-4">
                    <div class="rounded-2xl border border-solid border-surface-4 bg-surface-3 p-4">
                        {{ formatMessage(projectCompatibilityMessages.minecraftJava) }}
                        <div class="mt-2 flex flex-wrap gap-1">
                            <TagItem
                                v-for="gameVersion in gameVersionsToDisplay"
                                :key="`version-compat-game-version-${gameVersion}`"
                                >{{ gameVersion }}</TagItem
                            >
                        </div>
                    </div>
                    <div class="rounded-2xl border border-solid border-surface-4 bg-surface-3 p-4">
                        {{
                            formatMessage(projectCompatibilityMessages.platformsPlural, {
                                count: platforms.length,
                            })
                        }}
                        <div class="mt-2 flex flex-wrap gap-1">
                            <TagTagItem
                                v-for="platform in platforms"
                                :key="`version-compat-platform-${platform}`"
                                :tag="platform"
                            />
                        </div>
                    </div>
                </div>
            </section>
            <section v-if="requiredContent.length > 0" id="dependencies">
                <h3 class="mb-2 mt-0 text-lg font-semibold">
                    {{ formatMessage(messages.required) }}
                </h3>
                <div class="grid grid-cols-2 gap-4">
                    <div
                        v-for="context in requiredContent"
                        :key="`required-dep-${context.dependency.version_id ?? context.dependency.project_id ?? context.dependency.file_name}`"
                        class="flex items-center gap-3 rounded-2xl border border-solid border-surface-4 bg-surface-3 p-4"
                    >
                        <AutoLink
                            :to="dependencyLink(context)"
                            class="group flex items-center gap-3"
                            tabindex="-1"
                        >
                            <Avatar :src="context.project?.icon_url" alt="" size="48px" no-shadow />
                            <div class="flex flex-col gap-1">
                                <span
                                    class="font-medium text-[var(--color-text-primary)]"
                                    :class="{ 'group-hover:underline': !!dependencyLink(context) }"
                                    >{{
                                        context.project?.title ?? context.dependency.file_name
                                    }}</span
                                >
                                <span
                                    class="flex items-center gap-1 text-[var(--color-text-tertiary)]"
                                >
                                    <template v-if="!context.dependency.version_id">
                                        {{ formatMessage(messages.anyCompatibleVersion) }}
                                    </template>
                                    <template v-else-if="context.versionNumber">
                                        {{ context.versionNumber }}
                                    </template>
                                </span>
                            </div>
                        </AutoLink>
                    </div>
                </div>
            </section>
            <section id="changes">
                <h3 class="mb-2 mt-0 text-lg font-semibold">
                    {{ formatMessage(messages.changes) }}
                </h3>
                <div class="rounded-2xl border border-solid border-surface-4 bg-surface-3 p-4">
                    <ReleaseChangelog :release-id="version.id" :changelog="version.changelog" />
                </div>
            </section>
            <section v-if="optionalContent.length > 0" id="optional-dependencies">
                <h3 class="mb-2 mt-0 text-lg font-semibold">
                    {{ formatMessage(messages.optionalDeps) }}
                </h3>
                <div class="grid grid-cols-2 gap-4">
                    <div
                        v-for="context in optionalContent"
                        :key="`optional-dep-${context.dependency.version_id ?? context.dependency.project_id ?? context.dependency.file_name}`"
                        class="flex items-center gap-3 rounded-2xl border border-solid border-surface-4 bg-surface-3 p-4"
                    >
                        <AutoLink
                            :to="dependencyLink(context)"
                            class="group flex items-center gap-3"
                            tabindex="-1"
                        >
                            <Avatar :src="context.project?.icon_url" alt="" size="48px" no-shadow />
                            <div class="flex flex-col gap-1">
                                <span
                                    class="font-medium text-[var(--color-text-primary)]"
                                    :class="{ 'group-hover:underline': !!dependencyLink(context) }"
                                    >{{
                                        context.project?.title ?? context.dependency.file_name
                                    }}</span
                                >
                                <span
                                    class="flex items-center gap-1 text-[var(--color-text-tertiary)]"
                                >
                                    <template v-if="!context.dependency.version_id">
                                        {{ formatMessage(messages.anyCompatibleVersion) }}
                                    </template>
                                    <template v-else-if="context.versionNumber">
                                        {{ context.versionNumber }}
                                    </template>
                                </span>
                            </div>
                        </AutoLink>
                    </div>
                </div>
            </section>
            <section v-if="incompatibleContent.length > 0" id="known-incompatibilities">
                <h3 class="mb-2 mt-0 text-lg font-semibold">
                    {{ formatMessage(messages.knownIncompatibilities) }}
                </h3>
                <div class="grid grid-cols-2 gap-4">
                    <div
                        v-for="context in incompatibleContent"
                        :key="`incompatible-dep-${context.dependency.version_id ?? context.dependency.project_id ?? context.dependency.file_name}`"
                        class="flex items-center gap-3 rounded-2xl border border-solid border-surface-4 bg-surface-3 p-4"
                    >
                        <AutoLink
                            :to="dependencyLink(context)"
                            class="group flex items-center gap-3"
                            tabindex="-1"
                        >
                            <Avatar :src="context.project?.icon_url" alt="" size="48px" no-shadow />
                            <div class="flex flex-col gap-1">
                                <span
                                    class="font-medium text-[var(--color-text-primary)]"
                                    :class="{ 'group-hover:underline': !!dependencyLink(context) }"
                                    >{{
                                        context.project?.title ?? context.dependency.file_name
                                    }}</span
                                >
                                <span
                                    class="flex items-center gap-1 text-[var(--color-text-tertiary)]"
                                >
                                    <template v-if="!context.dependency.version_id">
                                        {{ formatMessage(messages.anyVersion) }}
                                    </template>
                                    <template v-else-if="context.versionNumber">
                                        {{ context.versionNumber }}
                                    </template>
                                </span>
                            </div>
                        </AutoLink>
                    </div>
                </div>
            </section>
            <section v-if="embeddedDependencies.length > 0" id="content">
                <h3 class="mb-2 mt-0 text-lg font-semibold">
                    {{
                        formatMessage(
                            version.loaders.includes('mrpack')
                                ? messages.content
                                : messages.includedContent,
                        )
                    }}
                </h3>
                <Table :columns="contentTableColumns" :data="includedContent" row-key="id">
                    <template #header-actions>
                        <StyledInput
                            v-model="contentSearchQuery"
                            :icon="SearchIcon"
                            :placeholder="formatMessage(messages.searchContent)"
                            :disabled="includedContentLoading"
                            clearable
                            wrapper-class="w-64"
                        />
                    </template>
                    <template #cell-icon="{ row }">
                        <div
                            v-if="isIncludedContentLoadingRow(row)"
                            class="size-[2rem] shrink-0 animate-pulse rounded-[16%] bg-surface-3"
                        />
                        <AutoLink v-else :to="row.link" tabindex="-1" class="flex">
                            <Avatar
                                v-if="row.hasProject || row.icon_url"
                                :src="row.icon_url"
                                alt=""
                                size="2rem"
                            />
                            <div v-else class="flex size-[2rem] items-center justify-center">
                                <FileIcon class="size-5 text-[var(--color-text-tertiary)]" />
                            </div>
                        </AutoLink>
                    </template>
                    <template #cell-name="{ row }">
                        <div
                            v-if="isIncludedContentLoadingRow(row)"
                            class="h-4 max-w-[12rem] animate-pulse rounded-lg bg-surface-3"
                        />
                        <AutoLink
                            v-else
                            :to="row.link"
                            class="flex w-fit"
                            link-class="hover:underline hover:text-[var(--color-text-primary)]"
                        >
                            {{ row.name }}
                        </AutoLink>
                    </template>
                    <template #cell-version="{ row }">
                        <div
                            v-if="isIncludedContentLoadingRow(row)"
                            class="h-4 w-16 animate-pulse rounded-lg bg-surface-3"
                        />
                        <AutoLink
                            v-else
                            :to="row.version ? row.link : undefined"
                            class="flex w-fit"
                            tabindex="-1"
                            link-class="hover:underline hover:text-[var(--color-text-primary)]"
                        >
                            {{ row.version ?? '—' }}
                        </AutoLink>
                    </template>
                </Table>
            </section>
            <section v-if="supplementaryResources.length > 0" id="supplementary-resources">
                <h3 class="mb-2 mt-0 text-lg font-semibold">
                    {{ formatMessage(messages.supplementaryResources) }}
                </h3>
                <Table :columns="supplementaryResourcesTableColumns" :data="supplementaryResources">
                    <template #cell-file="{ row }">
                        {{ row.file.filename }}
                    </template>
                    <template #cell-type>
                        {{ formatMessage(commonMessages.unknownLabel) }}
                    </template>
                    <template #cell-size="{ row }">
                        {{ formatBytes(row.file.size) }}
                    </template>
                    <template #cell-actions="{ row }">
                        <div class="flex items-center justify-end gap-2">
                            <ButtonLink
                                :href="fileDownloadUrl(row.file.filename)"
                                :download="row.file.filename"
                                target="_blank"
                            >
                                <DownloadIcon aria-hidden="true" />
                                {{ formatMessage(discoverMessages.downloadInBrowser) }}
                            </ButtonLink>
                        </div>
                    </template>
                </Table>
            </section>
        </div>
    </div>
</template>
