<script setup lang="ts">
import ChevronLeftIcon from '@modrinth/assets/icons/chevron-left.svg?component'
import FilterIcon from '@modrinth/assets/icons/filter.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import XCircleIcon from '@modrinth/assets/icons/x-circle.svg?component'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import FormattedTag from '@modrinth/ui/src/components/base/FormattedTag.vue'
import MultiSelect from '@modrinth/ui/src/components/base/MultiSelect.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { formatLoader } from '@modrinth/ui/src/utils/tag-messages.ts'
import { computed, ref } from 'vue'
import type { LocationQueryValue } from 'vue-router'
import { useRoute } from 'vue-router'

import type { ProjectVersion } from '../../../../data'
import type { GameVersionTag } from '../../versions'

/** Port of the shared `VersionFilterControl` (packages/ui/src/components/version). */
const props = defineProps<{
    versions: ProjectVersion[]
    gameVersions: GameVersionTag[]
}>()

const text = defineMessages({
    gameVersions: { id: 'project.versions.filter.gameVersions', defaultMessage: 'Game versions' },
    projectChannels: { id: 'project.versions.filter.projectChannels', defaultMessage: 'Channels' },
})

const emit = defineEmits<{
    'update:query': [query: Record<string, string | string[] | undefined | null>]
}>()

const { formatMessage } = useVIntl()
const allChannels = ['release', 'beta', 'alpha']
const route = useRoute()
const showSnapshots = ref(false)

type FilterType = 'channel' | 'gameVersion' | 'platform'

const gameVersionTags = computed(() => new Map(props.gameVersions.map((tag) => [tag.version, tag])))

const availableGameVersions = computed(() => {
    const set = new Set<string>()
    for (const version of props.versions)
        for (const gameVersion of version.game_versions) set.add(gameVersion)
    const known = props.gameVersions.filter((tag) => set.has(tag.version)).map((tag) => tag.version)
    const knownSet = new Set(known)
    return [...known, ...[...set].filter((version) => !knownSet.has(version))]
})

function isReleaseGameVersion(version: string) {
    return gameVersionTags.value.get(version)?.version_type === 'release'
}

const hasAnyReleaseGameVersions = computed(() =>
    availableGameVersions.value.some(isReleaseGameVersion),
)
const hasAnyNonReleaseGameVersions = computed(() =>
    availableGameVersions.value.some((version) => !isReleaseGameVersion(version)),
)

const filterOptions = computed(() => {
    const platformSet = new Set<string>()
    const channelSet = new Set<string>()
    for (const version of props.versions) {
        for (const loader of version.loaders) platformSet.add(loader)
        channelSet.add(version.version_type)
    }
    return {
        channel: [...channelSet].sort((a, b) => allChannels.indexOf(a) - allChannels.indexOf(b)),
        gameVersion: availableGameVersions.value.filter((version) =>
            showSnapshots.value || !hasAnyReleaseGameVersions.value
                ? true
                : isReleaseGameVersion(version),
        ),
        platform: [...platformSet],
    }
})

const gameVersionOptions = computed(() =>
    filterOptions.value.gameVersion.map((version) => ({ value: version, label: version })),
)
const channelOptions = computed(() =>
    filterOptions.value.channel.map((channel) => ({
        value: channel,
        label: channel === 'release' ? 'Release' : channel === 'beta' ? 'Beta' : 'Alpha',
    })),
)
const platformOptions = computed(() =>
    filterOptions.value.platform.map((platform) => ({
        value: platform,
        label: formatLoader(formatMessage, platform),
    })),
)

function getArrayOrString(value: LocationQueryValue | LocationQueryValue[] | undefined): string[] {
    if (value === null || value === undefined) return []
    if (typeof value === 'string') return [value]
    return value.filter((entry): entry is string => entry !== null)
}

const selectedChannels = ref<string[]>(getArrayOrString(route.query.c))
const selectedGameVersions = ref<string[]>(getArrayOrString(route.query.g))
const selectedPlatforms = ref<string[]>(getArrayOrString(route.query.l))
if (selectedGameVersions.value.some((version) => !isReleaseGameVersion(version)))
    showSnapshots.value = true

function toggleFilters(type: FilterType, filters: string[]) {
    for (const filter of filters) toggleFilter(type, filter, true)
    updateFilters()
}

function toggleFilter(type: FilterType, filter: string, bulk = false) {
    const list =
        type === 'channel'
            ? selectedChannels
            : type === 'gameVersion'
              ? selectedGameVersions
              : selectedPlatforms
    list.value = list.value.includes(filter)
        ? list.value.filter((entry) => entry !== filter)
        : [...list.value, filter]
    if (!bulk) updateFilters()
}

function updateSelectedGameVersions(versions: string[]) {
    selectedGameVersions.value = versions
    updateFilters()
}

function updateSelectedChannels(channels: string[]) {
    selectedChannels.value = channels
    updateFilters()
}

function updateSelectedPlatforms(platforms: string[]) {
    selectedPlatforms.value = platforms
    updateFilters()
}

function updateShowSnapshots(value: boolean) {
    showSnapshots.value = value
    if (value || !hasAnyReleaseGameVersions.value) return
    const releases = selectedGameVersions.value.filter(isReleaseGameVersion)
    if (releases.length !== selectedGameVersions.value.length) {
        selectedGameVersions.value = releases
        updateFilters()
    }
}

function clearFilters() {
    selectedChannels.value = []
    selectedGameVersions.value = []
    selectedPlatforms.value = []
    updateFilters()
}

function updateFilters() {
    emit('update:query', {
        c: selectedChannels.value,
        g: selectedGameVersions.value,
        l: selectedPlatforms.value,
        page: undefined,
    })
}

defineExpose({
    toggleFilter,
    toggleFilters,
    selectedChannels,
    selectedGameVersions,
    selectedPlatforms,
})
</script>

<template>
    <div class="flex flex-col gap-3">
        <div class="flex flex-wrap items-center gap-2">
            <MultiSelect
                v-if="filterOptions.platform.length > 1"
                :model-value="selectedPlatforms"
                :options="platformOptions"
                fit-content
                :dropdown-min-width="180"
                trigger-class="!min-h-9 !px-3 !py-0"
                @update:model-value="updateSelectedPlatforms"
            >
                <template #input-content="{ isOpen, openDirection }">
                    <div class="flex items-center gap-2">
                        <FilterIcon class="h-5 w-5 text-[var(--color-text-tertiary)]" />
                        <span class="font-semibold text-[var(--color-text-default)]"
                            >Platforms</span
                        >
                        <ChevronLeftIcon
                            class="h-5 w-5 text-[var(--color-text-tertiary)] transition-transform duration-150"
                            :class="
                                isOpen
                                    ? openDirection === 'down'
                                        ? 'rotate-90'
                                        : '-rotate-90'
                                    : '-rotate-90'
                            "
                        />
                    </div>
                </template>
            </MultiSelect>
            <MultiSelect
                v-if="availableGameVersions.length > 1"
                :model-value="selectedGameVersions"
                :options="gameVersionOptions"
                searchable
                search-placeholder="Search..."
                fit-content
                :dropdown-min-width="240"
                trigger-class="!min-h-9 !px-3 !py-0"
                @update:model-value="updateSelectedGameVersions"
            >
                <template #input-content="{ isOpen, openDirection }">
                    <div class="flex items-center gap-2">
                        <FilterIcon class="h-5 w-5 text-[var(--color-text-tertiary)]" />
                        <span class="font-semibold text-[var(--color-text-default)]">{{
                            formatMessage(text.gameVersions)
                        }}</span>
                        <ChevronLeftIcon
                            class="h-5 w-5 text-[var(--color-text-tertiary)] transition-transform duration-150"
                            :class="
                                isOpen
                                    ? openDirection === 'down'
                                        ? 'rotate-90'
                                        : '-rotate-90'
                                    : '-rotate-90'
                            "
                        />
                    </div>
                </template>
                <template v-if="hasAnyNonReleaseGameVersions" #bottom>
                    <div class="border-0 border-t border-solid border-t-surface-5 px-3 py-3">
                        <Checkbox
                            :model-value="showSnapshots"
                            class="mx-1"
                            :label="`Show all versions`"
                            @update:model-value="updateShowSnapshots"
                        />
                    </div>
                </template>
            </MultiSelect>
            <MultiSelect
                v-if="filterOptions.channel.length > 1"
                :model-value="selectedChannels"
                :options="channelOptions"
                fit-content
                :dropdown-min-width="180"
                trigger-class="!min-h-9 !px-3 !py-0"
                @update:model-value="updateSelectedChannels"
            >
                <template #input-content="{ isOpen, openDirection }">
                    <div class="flex items-center gap-2">
                        <FilterIcon class="h-5 w-5 text-[var(--color-text-tertiary)]" />
                        <span class="font-semibold text-[var(--color-text-default)]">{{
                            formatMessage(text.projectChannels)
                        }}</span>
                        <ChevronLeftIcon
                            class="h-5 w-5 text-[var(--color-text-tertiary)] transition-transform duration-150"
                            :class="
                                isOpen
                                    ? openDirection === 'down'
                                        ? 'rotate-90'
                                        : '-rotate-90'
                                    : '-rotate-90'
                            "
                        />
                    </div>
                </template>
            </MultiSelect>
        </div>
        <div class="flex flex-wrap items-center gap-1 empty:hidden">
            <TagItem
                v-if="
                    selectedChannels.length +
                        selectedGameVersions.length +
                        selectedPlatforms.length >
                    1
                "
                class="transition-transform active:scale-[0.95]"
                :action="clearFilters"
            >
                <XCircleIcon />
                Clear all filters
            </TagItem>
            <TagItem
                v-for="channel in selectedChannels"
                :key="`remove-filter-${channel}`"
                :style="`--_color: var(--color-${channel === 'alpha' ? 'red' : channel === 'beta' ? 'orange' : 'green'});--_bg-color: var(--color-${channel === 'alpha' ? 'red' : channel === 'beta' ? 'orange' : 'green'}-highlight)`"
                :action="() => toggleFilter('channel', channel)"
            >
                <XIcon />
                {{ channel.slice(0, 1).toUpperCase() + channel.slice(1) }}
            </TagItem>
            <TagItem
                v-for="version in selectedGameVersions"
                :key="`remove-filter-${version}`"
                :action="() => toggleFilter('gameVersion', version)"
            >
                <XIcon />
                {{ version }}
            </TagItem>
            <TagItem
                v-for="platform in selectedPlatforms"
                :key="`remove-filter-${platform}`"
                :style="`--_color: var(--color-platform-${platform})`"
                :action="() => toggleFilter('platform', platform)"
            >
                <XIcon />
                <FormattedTag :tag="platform" enforce-type="loader" />
            </TagItem>
        </div>
    </div>
</template>
