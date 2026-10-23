<script setup lang="ts">
import BanIcon from '@modrinth/assets/icons/ban.svg?component'
import DropdownIcon from '@modrinth/assets/icons/dropdown.svg?component'
import LockOpenIcon from '@modrinth/assets/icons/lock-open.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import UpdatedIcon from '@modrinth/assets/icons/updated.svg?component'
import Accordion from '@modrinth/ui/src/components/base/Accordion.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import ScrollablePanel from '@modrinth/ui/src/components/base/ScrollablePanel.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import SearchFilterOption from '@modrinth/ui/src/components/search/SearchFilterOption.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type { FilterOption, FilterType, FilterValue } from '@modrinth/ui/src/utils/search.ts'
import { computed, ref } from 'vue'

import SearchDependsOnFilter from './SearchDependsOnFilter.vue'

/**
 * Port of the shared `SearchSidebarFilter` (packages/ui/src/components/search/
 * SearchSidebarFilter.vue): one collapsible filter section with include/exclude options,
 * search, scrolling, "Show more", toggle groups and the locked (provided) state.
 */
const { formatMessage } = useVIntl()

const selectedFilters = defineModel<FilterValue[]>('selectedFilters', { required: true })
const toggledGroups = defineModel<string[]>('toggledGroups', { required: true })
const overriddenProvidedFilterTypes = defineModel<string[]>('overriddenProvidedFilterTypes', {
    required: false,
    default: [],
})

const props = defineProps<{
    filterType: FilterType
    projectType?: string
    buttonClass?: string
    contentClass?: string
    innerPanelClass?: string
    openByDefault?: boolean
    providedFilters: FilterValue[]
}>()

const emit = defineEmits<{ onOpen: []; onClose: [] }>()

defineOptions({ inheritAttrs: false })

const query = ref('')
const showMore = ref(false)
const accordion = ref<InstanceType<typeof Accordion> | null>()

const defaultValues = computed(() =>
    props.filterType.display === 'expandable' ? props.filterType.default_values : [],
)

const selectedFilterOptions = computed(() =>
    props.filterType.options.filter((option) =>
        locked.value ? isProvided(option, false) : isIncluded(option),
    ),
)
const selectedNegativeFilterOptions = computed(() =>
    props.filterType.options.filter((option) =>
        locked.value ? isProvided(option, true) : isExcluded(option),
    ),
)
const visibleOptions = computed(() =>
    props.filterType.options
        .filter((option) => isVisible(option) || isIncluded(option) || isExcluded(option))
        .slice()
        .sort((a, b) => {
            if (props.filterType.display === 'expandable') {
                const aDefault = defaultValues.value.includes(a.id)
                const bDefault = defaultValues.value.includes(b.id)
                if (aDefault && !bDefault) return -1
                if (!aDefault && bDefault) return 1
            }
            return 0
        }),
)

const hasProvidedFilter = computed(() =>
    props.providedFilters.some((filter) => filter.type === props.filterType.id),
)
const locked = computed(
    () =>
        hasProvidedFilter.value &&
        !overriddenProvidedFilterTypes.value.includes(props.filterType.id),
)

const scrollable = computed(
    () => visibleOptions.value.length >= 10 && props.filterType.display === 'scrollable',
)

function iconStyle(option: FilterOption) {
    if (
        [
            'mod_loader',
            'modpack_loader',
            'plugin_loader',
            'plugin_platform',
            'shader_loader',
        ].includes(props.filterType.id)
    ) {
        return { color: `var(--color-platform-${option.id})` }
    }
    return undefined
}

function groupEnabled(group: string) {
    return toggledGroups.value.includes(group)
}

function toggleGroup(group: string) {
    toggledGroups.value = toggledGroups.value.includes(group)
        ? toggledGroups.value.filter((entry) => entry !== group)
        : [...toggledGroups.value, group]
}

function isIncluded(filter: FilterOption) {
    return selectedFilters.value.some((value) => value.option === filter.id && !value.negative)
}

function isExcluded(filter: FilterOption) {
    return selectedFilters.value.some((value) => value.option === filter.id && value.negative)
}

function isVisible(filter: FilterOption) {
    const filterKey = filter.formatted_name?.toLowerCase() ?? filter.id.toLowerCase()
    const matchesQuery = !query.value || filterKey.includes(query.value.toLowerCase())
    if (props.filterType.display === 'expandable') {
        return matchesQuery && (showMore.value || defaultValues.value.includes(filter.id))
    }
    if (filter.toggle_group)
        return toggledGroups.value.includes(filter.toggle_group) && matchesQuery
    return matchesQuery
}

function isProvided(filter: FilterOption, negative: boolean) {
    return props.providedFilters.some(
        (provided) =>
            provided.type === props.filterType.id &&
            provided.option === filter.id &&
            !provided.negative === !negative,
    )
}

type FilterState = 'include' | 'exclude' | 'ignore'

function toggleFilter(filter: FilterOption) {
    setFilter(filter, isIncluded(filter) || isExcluded(filter) ? 'ignore' : 'include')
}

function toggleNegativeFilter(filter: FilterOption) {
    setFilter(filter, isExcluded(filter) ? 'ignore' : 'exclude')
}

function setFilter(filter: FilterOption, state: FilterState) {
    const next = selectedFilters.value.filter((selected) => selected.option !== filter.id)
    if (state === 'include')
        next.push({ type: props.filterType.id, option: filter.id, negative: false })
    else if (state === 'exclude')
        next.push({ type: props.filterType.id, option: filter.id, negative: true })
    selectedFilters.value = next
}

function syncProvidedFilter() {
    overriddenProvidedFilterTypes.value = overriddenProvidedFilterTypes.value.filter(
        (id) => id !== props.filterType.id,
    )
    accordion.value?.close()
    selectedFilters.value = selectedFilters.value.filter(
        (filter) => filter.type !== props.filterType.id,
    )
}

const text = defineMessages({
    searchPlaceholder: {
        id: 'search.filter.option.search.placeholder',
        defaultMessage: 'Search...',
    },
    showFewer: { id: 'search.filter.option.show_fewer', defaultMessage: 'Show fewer' },
    showMore: { id: 'search.filter.option.show_more', defaultMessage: 'Show more' },
    unlockFilterButton: {
        id: 'search.filter.locked.default.unlock',
        defaultMessage: 'Unlock filter',
    },
    syncFilterButton: { id: 'search.filter.locked.default.sync', defaultMessage: 'Sync filter' },
    lockedTitle: { id: 'search.filter.locked.default.title', defaultMessage: '{type} is locked' },
    lockedDescription: {
        id: 'search.filter.locked.default.description',
        defaultMessage: 'Unlocking this filter may allow you to install incompatible content.',
    },
})
</script>

<template>
    <Accordion
        v-bind="$attrs"
        ref="accordion"
        :button-class="buttonClass ?? 'flex flex-col gap-2 justify-start items-start'"
        :content-class="contentClass"
        title-wrapper-class="flex flex-col gap-2 justify-start items-start"
        :open-by-default="openByDefault !== undefined ? openByDefault : true"
        @on-open="emit('onOpen')"
        @on-close="emit('onClose')"
    >
        <template #button="{ open }">
            <div class="flex w-full items-center gap-1 text-[var(--color-text-primary)]">
                <slot name="header" :filter="filterType">
                    <h2 class="text-base font-semibold text-red">
                        {{ filterType.formatted_name }}
                    </h2>
                </slot>
                <DropdownIcon
                    class="ml-auto size-5 shrink-0 text-[var(--color-text-default)] transition-transform duration-300 group-hover:text-[var(--color-text-primary)]"
                    :class="{ 'rotate-180': open }"
                />
            </div>
        </template>
        <template
            v-if="
                locked ||
                (!!accordion &&
                    !accordion.isOpen &&
                    (selectedFilterOptions.length > 0 || selectedNegativeFilterOptions.length > 0))
            "
            #summary
        >
            <div class="flex flex-wrap gap-1">
                <div
                    v-for="option in selectedFilterOptions"
                    :key="`selected-filter-${filterType.id}-${option.id}`"
                    class="flex w-fit shrink-0 items-center gap-1 rounded-full bg-surface-4 px-2 py-0.5 text-xs font-bold text-[var(--color-text-tertiary)]"
                >
                    {{ option.formatted_name ?? option.id }}
                </div>
                <div
                    v-for="option in selectedNegativeFilterOptions"
                    :key="`excluded-filter-${filterType.id}-${option.id}`"
                    class="flex w-fit shrink-0 items-center gap-1 rounded-full bg-surface-4 px-2 py-0.5 text-xs font-bold text-[var(--color-text-tertiary)]"
                >
                    <BanIcon class="text-brand-red" /> {{ option.formatted_name ?? option.id }}
                </div>
            </div>
        </template>
        <template v-if="locked" #default>
            <div class="flex flex-col gap-2 rounded-2xl border-2 border-dashed border-divider p-3">
                <p class="m-0 items-center font-bold">
                    <slot :name="`locked-${filterType.id}`">
                        {{ formatMessage(text.lockedTitle, { type: filterType.formatted_name }) }}
                    </slot>
                </p>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(text.lockedDescription) }}
                </p>
                <Button
                    class="w-fit"
                    @click="
                        overriddenProvidedFilterTypes = [
                            ...overriddenProvidedFilterTypes,
                            filterType.id,
                        ]
                    "
                    ><LockOpenIcon />
                    {{ formatMessage(text.unlockFilterButton) }}
                </Button>
            </div>
        </template>
        <template v-else #default>
            <SearchDependsOnFilter
                v-if="filterType.display === 'depends-on-project'"
                v-model:selected-filters="selectedFilters"
                :project-type="projectType ?? ''"
                :inner-panel-class="innerPanelClass"
            />
            <slot name="prefix" />
            <div
                v-if="filterType.display === 'toggle'"
                :class="innerPanelClass ? innerPanelClass : ''"
                class="flex flex-col gap-3"
            >
                <label
                    v-for="option in filterType.options"
                    :key="`${filterType.id}-toggle-${option.id}`"
                    class="flex cursor-pointer items-center justify-between gap-3 font-semibold text-[var(--color-text-tertiary)]"
                >
                    <span class="text-sm">{{ option.formatted_name ?? option.id }}</span>
                    <Toggle
                        :model-value="isExcluded(option)"
                        small
                        class="shrink-0"
                        @update:model-value="toggleNegativeFilter(option)"
                    />
                </label>
            </div>
            <template
                v-if="
                    filterType.display !== 'toggle' && filterType.display !== 'depends-on-project'
                "
            >
                <StyledInput
                    v-if="filterType.searchable"
                    :id="`search-${filterType.id}`"
                    v-model="query"
                    :icon="SearchIcon"
                    type="text"
                    :placeholder="formatMessage(text.searchPlaceholder)"
                    autocomplete="off"
                    clearable
                    size="small"
                    input-class="!bg-surface-4"
                    wrapper-class="mx-2 my-1 w-[calc(100%-1rem)]"
                />
                <ScrollablePanel
                    :class="{ 'h-[16rem]': scrollable }"
                    :disable-scrolling="!scrollable"
                >
                    <div
                        :class="innerPanelClass ? innerPanelClass : ''"
                        class="flex flex-col gap-1"
                    >
                        <SearchFilterOption
                            v-for="option in visibleOptions"
                            :key="`${filterType.id}-${option.id}`"
                            :option="option"
                            :included="isIncluded(option)"
                            :excluded="isExcluded(option)"
                            :supports-negative-filter="filterType.supports_negative_filter"
                            :class="{ 'mr-3': scrollable }"
                            @toggle="toggleFilter"
                            @toggle-exclude="toggleNegativeFilter"
                        >
                            <span
                                v-if="option.icon || option.icon_url"
                                class="inline-flex h-4 w-4 shrink-0 items-center justify-center"
                                :style="iconStyle(option)"
                            >
                                <img
                                    v-if="option.icon_url"
                                    :src="option.icon_url"
                                    alt=""
                                    class="h-4 w-4 object-contain"
                                />
                                <component
                                    :is="option.icon"
                                    v-else-if="option.icon"
                                    class="h-4 w-4"
                                />
                            </span>
                            <span class="truncate text-sm" :style="iconStyle(option)">
                                {{ option.formatted_name ?? option.id }}
                            </span>
                        </SearchFilterOption>
                        <button
                            v-if="filterType.display === 'expandable'"
                            class="flex !w-full cursor-pointer items-center gap-2 truncate rounded-xl border-none bg-transparent px-2 py-1 text-sm font-semibold text-[var(--color-text-tertiary)] transition-all hover:text-[var(--color-text-primary)] focus-visible:text-[var(--color-text-primary)] active:scale-[0.98]"
                            @click="showMore = !showMore"
                        >
                            <DropdownIcon
                                class="h-4 w-4 transition-transform"
                                :class="{ 'rotate-180': showMore }"
                            />
                            <span class="truncate text-sm">
                                {{
                                    showMore
                                        ? formatMessage(text.showFewer)
                                        : formatMessage(text.showMore)
                                }}
                            </span>
                        </button>
                    </div>
                </ScrollablePanel>
            </template>
            <div :class="innerPanelClass ? innerPanelClass : ''" class="empty:hidden">
                <Checkbox
                    v-for="group in filterType.toggle_groups"
                    :key="`toggle-group-${group.id}`"
                    class="mx-2"
                    :model-value="groupEnabled(group.id)"
                    :label="`${group.formatted_name}`"
                    @update:model-value="toggleGroup(group.id)"
                />
                <div v-if="hasProvidedFilter" class="mx-1 mt-2">
                    <Button class="w-fit" @click="syncProvidedFilter"
                        ><UpdatedIcon />
                        <slot name="sync-button">
                            {{ formatMessage(text.syncFilterButton) }}
                        </slot>
                    </Button>
                </div>
            </div>
        </template>
    </Accordion>
</template>
