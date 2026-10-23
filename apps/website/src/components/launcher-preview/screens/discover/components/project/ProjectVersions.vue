<script setup lang="ts">
import SwapIcon from '@modrinth/assets/icons/arrow-left-right.svg?component'
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import AutoLink from '@modrinth/ui/src/components/base/AutoLink.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import ButtonLink from '@modrinth/ui/src/components/base/buttons/ButtonLink.vue'
import Pagination from '@modrinth/ui/src/components/base/Pagination.vue'
import Table, { type TableColumn } from '@modrinth/ui/src/components/base/Table.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import TagOverflowPopover from '@modrinth/ui/src/components/project/TagOverflowPopover.vue'
import VersionChannelIndicator from '@modrinth/ui/src/components/version/VersionChannelIndicator.vue'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useCompactNumber } from '@modrinth/ui/src/composables/format-number.ts'
import { useRelativeTime } from '@modrinth/ui/src/composables/how-ago.ts'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { formatTag } from '@modrinth/ui/src/utils/tag-messages.ts'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import type { ProjectDetail, ProjectVersion } from '../../../../data'
import { messages as discoverMessages } from '../../messages'
import { getLoaderIcon } from '../../search'
import { formatVersionsForDisplay, type GameVersionTag } from '../../versions'
import VersionFilterControl from './VersionFilterControl.vue'

/**
 * The Versions tab: apps/app-frontend/src/pages/project/Versions.vue around the shared
 * `ProjectPageVersions` table (packages/ui/src/components/project/ProjectPageVersions.vue),
 * which the window always shows at its desktop width.
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
const formatRelativeTime = useRelativeTime({ style: 'narrow' })
const { formatCompactNumber } = useCompactNumber()
const formatDateTime = useFormatDateTime({ timeStyle: 'short', dateStyle: 'long' })
const route = useRoute()
const router = useRouter()

const MAX_GAME_VERSION_TAGS = 5
const MAX_PLATFORM_TAGS = 3
const MOD_LOADER_PROJECT_TYPES = ['mod', 'modpack', 'plugin']

const messages = defineMessages({
    noMatchingVersions: {
        id: 'project.versions.no-matching-versions',
        defaultMessage: 'No versions match the selected filters.',
    },
    toggleFilterTooltip: {
        id: 'project.versions.filter.toggle-tooltip',
        defaultMessage: 'Toggle filter for {filter}',
    },
    modloaderShort: {
        id: 'project.versions.platform.modloader.short',
        defaultMessage: 'ModLoader',
    },
    tableVersion: { id: 'project.versions.table.version', defaultMessage: 'Version' },
    tableGameVersion: { id: 'project.versions.table.game-version', defaultMessage: 'Game version' },
    tablePlatform: { id: 'project.versions.table.platform', defaultMessage: 'Platform' },
    tablePublished: { id: 'project.versions.table.published', defaultMessage: 'Published' },
    tableDownloads: { id: 'project.versions.table.downloads', defaultMessage: 'Downloads' },
})

type VersionTableColumn =
    'channel' | 'name' | 'gameVersions' | 'platforms' | 'published' | 'downloads' | 'actions'
type DisplayVersion = ProjectVersion & { noModLoader: boolean }
type VersionTableRow = DisplayVersion & Record<string, unknown>

const visibleCellClass = '!overflow-visible py-3 align-middle pr-2.5'

const versionColumns = computed<TableColumn<VersionTableColumn>[]>(() => [
    {
        key: 'channel',
        width: '4.5rem',
        headerClass: 'text-[var(--color-text-tertiary)]',
        cellClass: visibleCellClass,
    },
    {
        key: 'name',
        label: formatMessage(messages.tableVersion),
        cellClass: '!overflow-visible py-3 pr-4 min-w-[7rem]',
    },
    {
        key: 'gameVersions',
        label: formatMessage(messages.tableGameVersion),
        cellClass: '!overflow-visible py-3 align-middle pr-2.5 min-w-0 max-w-[12rem]',
    },
    {
        key: 'platforms',
        label: formatMessage(messages.tablePlatform),
        cellClass: '!overflow-visible py-3 align-middle pr-2.5 min-w-0 max-w-[12rem]',
    },
    {
        key: 'published',
        label: formatMessage(messages.tablePublished),
        cellClass: '!overflow-visible align-middle pr-2.5 w-max',
        width: '12%',
    },
    {
        key: 'downloads',
        label: formatMessage(messages.tableDownloads),
        cellClass: '!overflow-visible align-middle',
        width: '12%',
    },
    {
        key: 'actions',
        align: 'right',
        headerClass: 'text-[var(--color-text-tertiary)]',
        width: '1%',
        cellClass: '!overflow-visible align-middle',
    },
])

function getModpackLoaders(version: ProjectVersion): string[] {
    const loaders = Array.isArray(version.loaders) ? version.loaders : []
    if (props.project.project_type !== 'modpack') return loaders
    return loaders.filter((loader) => loader !== 'mrpack')
}

function hasNoModLoader(loaders: string[]) {
    return (
        (props.project.project_type === 'modpack' &&
            loaders.length === 1 &&
            loaders[0] === 'minecraft') ||
        (MOD_LOADER_PROJECT_TYPES.includes(props.project.project_type) && loaders.length === 0)
    )
}

function getDisplayGameVersions(version: DisplayVersion) {
    return formatVersionsForDisplay(version.game_versions, props.gameVersions)
}

function getFilterTooltip(filter: string) {
    return formatMessage(messages.toggleFilterTooltip, { filter })
}

function getPlatformLabel(platform: string) {
    if (platform === 'modloader') return formatMessage(messages.modloaderShort)
    return formatTag(formatMessage, platform, 'loader')
}

function getPlatformTooltip(platform: string) {
    return getFilterTooltip(formatTag(formatMessage, platform, 'loader'))
}

const normalizedVersions = computed<DisplayVersion[]>(() =>
    props.versions.map((version) => {
        const loaders = getModpackLoaders(version)
        const noModLoader = hasNoModLoader(loaders)
        return {
            ...version,
            game_versions: Array.isArray(version.game_versions) ? version.game_versions : [],
            loaders: noModLoader ? [] : loaders,
            noModLoader,
        }
    }),
)

const currentPage = ref(1)
const pageSize = 20
const versionFilters = ref<InstanceType<typeof VersionFilterControl> | null>(null)

const selectedGameVersions = computed<string[]>(
    () => versionFilters.value?.selectedGameVersions ?? [],
)
const selectedPlatforms = computed<string[]>(() => versionFilters.value?.selectedPlatforms ?? [])
const selectedChannels = computed<string[]>(() => versionFilters.value?.selectedChannels ?? [])

function hasAnySelected(values: string[], selected: string[]) {
    return selected.length === 0 || selected.some((value) => values.includes(value))
}

const filteredVersions = computed(() =>
    normalizedVersions.value.filter(
        (version) =>
            hasAnySelected(version.game_versions, selectedGameVersions.value) &&
            hasAnySelected(version.loaders, selectedPlatforms.value) &&
            (selectedChannels.value.length === 0 ||
                selectedChannels.value.includes(version.version_type)),
    ),
)

const currentVersionRows = computed<VersionTableRow[]>(
    () =>
        filteredVersions.value.slice(
            (currentPage.value - 1) * pageSize,
            currentPage.value * pageSize,
        ) as VersionTableRow[],
)

if (route.query.page) currentPage.value = Number(route.query.page) || 1

watch(
    [() => route.query.page, () => filteredVersions.value.length],
    ([page]) => {
        const requestedPage = Number(page) || 1
        currentPage.value = Math.min(
            Math.max(1, Math.floor(requestedPage)),
            Math.max(1, Math.ceil(filteredVersions.value.length / pageSize)),
        )
    },
    { immediate: true },
)

/** The app scrolls `window`, which never scrolls inside the desktop app's viewport. */
function switchPage(page: number) {
    currentPage.value = page
    void router.replace({
        query: { ...route.query, page: currentPage.value !== 1 ? currentPage.value : undefined },
    })
}

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

function versionLink(version: ProjectVersion) {
    return buildProjectHref(`/project/${props.project.id}/version/${version.id}`)
}

function getVersionRowClass() {
    return 'group version-row-link cursor-pointer transition-[filter]'
}

function openVersionRow(version: VersionTableRow) {
    void router.push(versionLink(version))
}

function updateQuery(newQueries: Record<string, string | string[] | undefined | null>) {
    if (newQueries.page) currentPage.value = Number(newQueries.page)
    else if (newQueries.page === undefined) currentPage.value = 1
    void router.replace({ query: { ...route.query, ...newQueries } })
}

function installTooltip(version: ProjectVersion) {
    if (!props.installed) return formatMessage(commonMessages.installButton)
    if (version.id !== props.installedVersion)
        return formatMessage(commonMessages.switchToVersionButton)
    return formatMessage(discoverMessages.versionsAlreadyInstalled)
}
</script>

<template>
    <div>
        <div class="mb-3 flex flex-col gap-3">
            <div class="flex flex-wrap justify-between gap-2">
                <VersionFilterControl
                    ref="versionFilters"
                    :versions="normalizedVersions"
                    :game-versions="gameVersions"
                    @update:query="updateQuery"
                />
                <Pagination
                    :page="currentPage"
                    class="mt-auto"
                    :count="Math.ceil(filteredVersions.length / pageSize)"
                    @switch-page="switchPage"
                />
            </div>
        </div>

        <Table
            v-if="versions.length > 0"
            class="block"
            :columns="versionColumns"
            :data="currentVersionRows"
            row-key="id"
            :row-class="getVersionRowClass"
            :row-clickable="true"
            :row-below-visible="() => false"
            table-layout="auto"
            @row-click="openVersionRow"
        >
            <template #empty-state>
                <p class="m-0 p-4 text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.noMatchingVersions) }}
                </p>
            </template>
            <template #cell-channel="{ row: version }">
                <div class="flex items-center justify-center">
                    <VersionChannelIndicator
                        v-tooltip="getFilterTooltip(version.version_type)"
                        :channel="version.version_type"
                        class="cursor-pointer"
                        data-no-row-click
                        @click.stop="versionFilters?.toggleFilter('channel', version.version_type)"
                    />
                </div>
            </template>

            <template #cell-name="{ row: version }">
                <div class="flex min-w-0 flex-col gap-2">
                    <AutoLink
                        v-tooltip="`${version.version_number} - ${version.name}`"
                        :to="versionLink(version)"
                        class="flex w-fit min-w-0 flex-col gap-1"
                        link-class="focus-visible:underline"
                        :title="`${version.version_number} - ${version.name}`"
                    >
                        <div class="flex min-w-0 items-center gap-2">
                            <div
                                class="version-row-name overflow-hidden text-ellipsis font-medium text-[var(--color-text-primary)]"
                            >
                                {{ version.version_number }}
                            </div>
                        </div>
                    </AutoLink>
                </div>
            </template>

            <template #cell-gameVersions="{ row: version }">
                <div class="flex w-full min-w-0 max-w-[12rem] flex-wrap gap-1">
                    <TagItem
                        v-for="gameVersion in getDisplayGameVersions(version).slice(
                            0,
                            MAX_GAME_VERSION_TAGS,
                        )"
                        :key="`version-tag-${gameVersion}`"
                        v-tooltip="getFilterTooltip(gameVersion)"
                        data-no-row-click
                        class="w-fit max-w-full truncate"
                        :action="
                            () =>
                                versionFilters?.toggleFilters('gameVersion', version.game_versions)
                        "
                    >
                        <span class="min-w-0 truncate">{{ gameVersion }}</span>
                    </TagItem>
                    <TagOverflowPopover
                        v-if="getDisplayGameVersions(version).length > MAX_GAME_VERSION_TAGS"
                        :count="getDisplayGameVersions(version).length - MAX_GAME_VERSION_TAGS"
                        data-no-row-click
                        wrapper-class="w-full min-w-0 cursor-default"
                    >
                        <template #trigger>
                            <TagItem as="span" class="w-fit max-w-full truncate">
                                <span class="min-w-0 truncate">
                                    +{{
                                        getDisplayGameVersions(version).length -
                                        MAX_GAME_VERSION_TAGS
                                    }}
                                </span>
                            </TagItem>
                        </template>
                        <TagItem
                            v-for="gameVersion in getDisplayGameVersions(version).slice(
                                MAX_GAME_VERSION_TAGS,
                            )"
                            :key="`overflow-version-tag-${gameVersion}`"
                            class="w-fit max-w-full truncate"
                            :action="
                                () =>
                                    versionFilters?.toggleFilters(
                                        'gameVersion',
                                        version.game_versions,
                                    )
                            "
                        >
                            <span class="min-w-0 truncate">{{ gameVersion }}</span>
                        </TagItem>
                    </TagOverflowPopover>
                </div>
            </template>

            <template #cell-platforms="{ row: version }">
                <div class="flex w-full min-w-0 max-w-[12rem] flex-wrap gap-1">
                    <template v-if="version.noModLoader">
                        <TagItem
                            class="w-fit max-w-full truncate border !border-solid border-surface-5"
                        >
                            <span class="min-w-0 truncate">No mod loader</span>
                        </TagItem>
                    </template>
                    <template v-else>
                        <TagItem
                            v-for="platform in version.loaders.slice(0, MAX_PLATFORM_TAGS)"
                            :key="`platform-tag-${platform}`"
                            v-tooltip="getPlatformTooltip(platform)"
                            data-no-row-click
                            class="w-fit max-w-full truncate"
                            :style="`--_color: var(--color-platform-${platform})`"
                            :action="() => versionFilters?.toggleFilter('platform', platform)"
                        >
                            <component
                                :is="getLoaderIcon(platform)"
                                v-if="getLoaderIcon(platform)"
                            />
                            <span class="min-w-0 truncate">{{ getPlatformLabel(platform) }}</span>
                        </TagItem>
                        <TagOverflowPopover
                            v-if="version.loaders.length > MAX_PLATFORM_TAGS"
                            :count="version.loaders.length - MAX_PLATFORM_TAGS"
                            data-no-row-click
                            wrapper-class="w-full min-w-0 cursor-default"
                        >
                            <template #trigger>
                                <TagItem as="span" class="w-fit max-w-full truncate">
                                    <span class="min-w-0 truncate">
                                        +{{ version.loaders.length - MAX_PLATFORM_TAGS }}
                                    </span>
                                </TagItem>
                            </template>
                            <TagItem
                                v-for="platform in version.loaders.slice(MAX_PLATFORM_TAGS)"
                                :key="`overflow-platform-tag-${platform}`"
                                v-tooltip="getPlatformTooltip(platform)"
                                class="w-fit max-w-full truncate"
                                :style="`--_color: var(--color-platform-${platform})`"
                                :action="() => versionFilters?.toggleFilter('platform', platform)"
                            >
                                <component
                                    :is="getLoaderIcon(platform)"
                                    v-if="getLoaderIcon(platform)"
                                />
                                <span class="min-w-0 truncate">{{
                                    getPlatformLabel(platform)
                                }}</span>
                            </TagItem>
                        </TagOverflowPopover>
                    </template>
                </div>
            </template>

            <template #cell-published="{ row: version }">
                <div
                    v-tooltip="formatDateTime(version.date_published)"
                    class="flex w-max cursor-default items-center gap-1 text-nowrap font-medium"
                    data-no-row-click
                >
                    {{ formatRelativeTime(new Date(version.date_published)) }}
                </div>
            </template>

            <template #cell-downloads="{ row: version }">
                <div
                    v-tooltip="`${version.downloads} downloads`"
                    class="flex w-max cursor-default items-center gap-1 text-nowrap font-medium"
                    data-no-row-click
                >
                    {{ formatCompactNumber(version.downloads) }}
                </div>
            </template>

            <template #cell-actions="{ row: version }">
                <div
                    class="flex h-full w-max cursor-default items-center justify-end gap-0.5 whitespace-nowrap"
                    data-no-row-click
                >
                    <Button
                        v-tooltip="installTooltip(version)"
                        circular
                        icon-only
                        type="quiet"
                        :color="installed && version.id === installedVersion ? 'base' : 'green'"
                        :disabled="installing || (installed && version.id === installedVersion)"
                        @click.stop="() => install(version.id)"
                    >
                        <DownloadIcon v-if="!installed" />
                        <SwapIcon v-else-if="installed && version.id !== installedVersion" />
                        <CheckIcon v-else />
                    </Button>
                    <ButtonLink
                        v-tooltip="formatMessage(commonMessages.openInBrowserButton)"
                        :href="`https://modrinth.com/${project.project_type}/${project.slug}/version/${version.id}`"
                        target="_blank"
                        type="quiet"
                        circular
                        icon-only
                        :label="formatMessage(commonMessages.openInBrowserButton)"
                    >
                        <ExternalIcon />
                    </ButtonLink>
                </div>
            </template>
        </Table>

        <div class="mt-3 flex">
            <Pagination
                :page="currentPage"
                class="ml-auto"
                :count="Math.ceil(filteredVersions.length / pageSize)"
                @switch-page="switchPage"
            />
        </div>
    </div>
</template>

<style scoped>
:deep(.version-row-link:hover:not(:has([data-no-row-click]:hover))),
:deep(.version-row-link:hover:not(:has([data-no-row-click]:hover)) + .table-row-below),
:deep(.version-row-link:has(+ .table-row-below:hover)),
:deep(.version-row-link:has(+ .table-row-below:hover) + .table-row-below) {
    filter: brightness(115%);
}

:deep(.version-row-link:hover:not(:has([data-no-row-click]:hover)) .version-row-name),
:deep(.version-row-link:has(+ .table-row-below:hover) .version-row-name) {
    text-decoration-line: underline;
}
</style>
