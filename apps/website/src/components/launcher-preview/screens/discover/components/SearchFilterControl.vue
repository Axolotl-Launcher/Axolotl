<script setup lang="ts">
import BanIcon from '@modrinth/assets/icons/ban.svg?component'
import LockIcon from '@modrinth/assets/icons/lock.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import XCircleIcon from '@modrinth/assets/icons/x-circle.svg?component'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import { defineMessage, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type { FilterOption, FilterType, FilterValue } from '@modrinth/ui/src/utils/search.ts'
import { computed } from 'vue'

import { injectBrowseContext } from '../browse-context'

/**
 * Port of `SearchFilterControl` (packages/ui/src/components/search/SearchFilterControl.vue): the
 * active filter chips under the toolbar. Dependency project names come from the snapshot.
 */
const { formatMessage } = useVIntl()
const ctx = injectBrowseContext()

const selectedFilters = defineModel<FilterValue[]>('selectedFilters', { required: true })

const props = defineProps<{
    filters: FilterType[]
    projectType?: string
    providedFilters: FilterValue[]
    overriddenProvidedFilterTypes: string[]
    providedMessage?: string
}>()

const defaultProvidedMessage = defineMessage({
    id: 'search.filter.locked.default',
    defaultMessage: 'Filter locked',
})
const dependentProjectMessage = defineMessage({
    id: 'search.filter.dependent_project',
    defaultMessage: 'Depends on: {project}',
})
const includedProjectsMessage = defineMessage({
    id: 'search.filter.included_projects',
    defaultMessage: 'Includes: {projects}',
})
const excludedProjectsMessage = defineMessage({
    id: 'search.filter.excluded_projects',
    defaultMessage: 'Excludes: {projects}',
})

type Item = {
    type: string
    option: string
    negative?: boolean
    formatted_name?: string
    provided: boolean
}

function parseProjectId(option: string) {
    const separator = option.indexOf(':')
    if (separator === -1) return option
    const types = option.slice(0, separator).split(',')
    const valid = types.every((type) => ['required', 'optional', 'embedded'].includes(type))
    return valid && option.slice(separator + 1) ? option.slice(separator + 1) : option
}

function projectName(projectId: string) {
    return ctx.findProject(projectId)?.title ?? projectId
}

function filterMatches(type: FilterType, option: FilterOption, list: FilterValue[]) {
    return list.some((provided) => provided.type === type.id && provided.option === option.id)
}

const items = computed<Item[]>(() =>
    props.filters.flatMap((type) => {
        const optionItems = type.options
            .filter(
                (option) =>
                    filterMatches(type, option, selectedFilters.value) ||
                    filterMatches(type, option, props.providedFilters),
            )
            .map((option) => ({
                type: type.id,
                option: option.id,
                negative: selectedFilters.value.find(
                    (selected) => selected.type === type.id && selected.option === option.id,
                )?.negative,
                provided: filterMatches(type, option, props.providedFilters),
                formatted_name: option.formatted_name,
            }))
        if (type.id !== 'compatible_dependency_project_ids') return optionItems
        const customValues = [...selectedFilters.value, ...props.providedFilters].filter(
            (filter) => filter.type === type.id,
        )
        return [
            ...optionItems,
            ...customValues.map((filter) => ({
                type: type.id,
                option: filter.option,
                negative: filter.negative,
                provided: props.providedFilters.some(
                    (provided) => provided.type === type.id && provided.option === filter.option,
                ),
                formatted_name: formatMessage(dependentProjectMessage, {
                    project: projectName(parseProjectId(filter.option)),
                }),
            })),
        ]
    }),
)

const selectedItems = computed(() => items.value.filter((item) => !item.provided))
const selectedIncludedProjectItems = computed(() =>
    props.projectType === 'modpack'
        ? selectedItems.value.filter(
              (item) => item.type === 'compatible_dependency_project_ids' && !item.negative,
          )
        : [],
)
const selectedExcludedProjectItems = computed(() =>
    props.projectType === 'modpack'
        ? selectedItems.value.filter(
              (item) => item.type === 'compatible_dependency_project_ids' && item.negative,
          )
        : [],
)
const otherSelectedItems = computed(() =>
    props.projectType === 'modpack'
        ? selectedItems.value.filter((item) => item.type !== 'compatible_dependency_project_ids')
        : selectedItems.value,
)
const selectedTagCount = computed(
    () =>
        otherSelectedItems.value.length +
        (selectedIncludedProjectItems.value.length > 0 ? 1 : 0) +
        (selectedExcludedProjectItems.value.length > 0 ? 1 : 0),
)

function formatProjectNames(list: Item[]) {
    return list.map((item) => projectName(parseProjectId(item.option))).join(', ')
}

function removeIncludedProjectFilters() {
    selectedFilters.value = selectedFilters.value.filter(
        (filter) => filter.type !== 'compatible_dependency_project_ids' || filter.negative,
    )
}

function removeExcludedProjectFilters() {
    selectedFilters.value = selectedFilters.value.filter(
        (filter) => filter.type !== 'compatible_dependency_project_ids' || !filter.negative,
    )
}

function removeFilter(filter: Item) {
    selectedFilters.value = selectedFilters.value.filter(
        (selected) => selected.type !== filter.type || selected.option !== filter.option,
    )
}

function clearFilters() {
    selectedFilters.value = []
}
</script>

<template>
    <div class="flex flex-wrap items-center gap-1 empty:hidden">
        <TagItem
            v-if="selectedTagCount > 1"
            class="transition-transform active:scale-[0.95]"
            :action="clearFilters"
        >
            <XCircleIcon />
            Clear all filters
        </TagItem>
        <TagItem
            v-if="selectedIncludedProjectItems.length > 0"
            :action="removeIncludedProjectFilters"
        >
            <XIcon />
            {{
                formatMessage(includedProjectsMessage, {
                    projects: formatProjectNames(selectedIncludedProjectItems),
                })
            }}
        </TagItem>
        <TagItem
            v-if="selectedExcludedProjectItems.length > 0"
            :action="removeExcludedProjectFilters"
        >
            <XIcon />
            <BanIcon class="text-brand-red" />
            {{
                formatMessage(excludedProjectsMessage, {
                    projects: formatProjectNames(selectedExcludedProjectItems),
                })
            }}
        </TagItem>
        <TagItem
            v-for="selectedItem in otherSelectedItems"
            :key="`remove-filter-${selectedItem.type}-${selectedItem.option}`"
            :action="() => removeFilter(selectedItem)"
        >
            <XIcon />
            <BanIcon v-if="selectedItem.negative" class="text-brand-red" />
            {{ selectedItem.formatted_name ?? selectedItem.option }}
        </TagItem>
        <TagItem
            v-for="providedItem in items.filter(
                (x) => x.provided && !overriddenProvidedFilterTypes.includes(x.type),
            )"
            :key="`provided-filter-${providedItem.type}-${providedItem.option}`"
            v-tooltip="providedMessage ?? formatMessage(defaultProvidedMessage)"
            :style="{ '--_bg-color': `var(--surface-2)` }"
        >
            <LockIcon />
            {{ providedItem.formatted_name ?? providedItem.option }}
        </TagItem>
    </div>
</template>
