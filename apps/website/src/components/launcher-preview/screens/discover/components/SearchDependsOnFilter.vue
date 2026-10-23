<script setup lang="ts">
import PackageIcon from '@modrinth/assets/icons/package.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import MultiSelect from '@modrinth/ui/src/components/base/MultiSelect.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type { FilterValue } from '@modrinth/ui/src/utils/search.ts'
import { computed, ref, watch } from 'vue'

import { injectBrowseContext } from '../browse-context'
import ProjectCombobox from './ProjectCombobox.vue'

/**
 * Port of `SearchDependsOnFilter` (packages/ui/src/components/search/SearchDependsOnFilter.vue):
 * "Included content" for modpacks and "Depends on" for other types. Project names come from
 * the snapshot instead of the Labrinth API.
 */
type DependencyType = 'required' | 'optional' | 'embedded'
const FILTER_TYPE_ID = 'compatible_dependency_project_ids'
const DEPENDENCY_TYPES: DependencyType[] = ['required', 'optional', 'embedded']

const props = defineProps<{
    projectType: string
    innerPanelClass?: string
}>()
const selectedFilters = defineModel<FilterValue[]>('selectedFilters', { required: true })
const { formatMessage } = useVIntl()
const ctx = injectBrowseContext()
const selectableProjectTypes = ['mod', 'resourcepack', 'shader', 'datapack', 'plugin']
const isModpack = computed(() => props.projectType === 'modpack')

function parseOption(option: string): { projectId: string; dependencyTypes: DependencyType[] } {
    const separator = option.indexOf(':')
    if (separator === -1) return { projectId: option, dependencyTypes: ['required'] }
    const dependencyTypes = option
        .slice(0, separator)
        .split(',')
        .filter((type): type is DependencyType => DEPENDENCY_TYPES.includes(type as DependencyType))
    const projectId = option.slice(separator + 1)
    if (!dependencyTypes.length || !projectId)
        return { projectId: option, dependencyTypes: ['required'] }
    return { projectId, dependencyTypes }
}

function formatOption(projectId: string, dependencyTypes: readonly DependencyType[]) {
    return `${dependencyTypes.join(',')}:${projectId}`
}

const dependencyProjectIds = computed(() => [
    ...new Set(
        selectedFilters.value
            .filter((filter) => filter.type === FILTER_TYPE_ID)
            .map((filter) => parseOption(filter.option).projectId),
    ),
])
const pendingProjectId = ref<string>()
const selectedDependencyFilter = computed(() =>
    selectedFilters.value.find((filter) => filter.type === FILTER_TYPE_ID),
)
const selectedProjectId = computed(() =>
    selectedDependencyFilter.value
        ? parseOption(selectedDependencyFilter.value.option).projectId
        : undefined,
)
const dependencyTypes = computed<DependencyType[]>(() =>
    selectedDependencyFilter.value
        ? parseOption(selectedDependencyFilter.value.option).dependencyTypes
        : ['required'],
)
const draftDependencyTypes = ref<DependencyType[]>([...dependencyTypes.value])
const projectCombobox = ref<{
    selectedProject: { title: string; icon_url?: string } | null
} | null>(null)
const selectedProject = computed(() => {
    if (projectCombobox.value?.selectedProject) return projectCombobox.value.selectedProject
    const id = selectedProjectId.value
    return id ? ctx.findProject(id) : null
})
const showSelectedProject = computed(() =>
    Boolean(selectedProjectId.value && selectedProject.value),
)
watch(dependencyTypes, (types) => (draftDependencyTypes.value = [...types]), { immediate: true })

const loadedDependentProjects = computed(() =>
    dependencyProjectIds.value.flatMap((projectId) => {
        const project = ctx.findProject(projectId)
        return project ? [{ projectId, project }] : []
    }),
)

const dependencyTypeOptions = computed(() => [
    { value: 'required' as const, label: formatMessage(text.required) },
    { value: 'optional' as const, label: formatMessage(text.optional) },
    { value: 'embedded' as const, label: formatMessage(text.embedded) },
])

function setSelectedProjectId(projectId: string | undefined) {
    const other = selectedFilters.value.filter((filter) => filter.type !== FILTER_TYPE_ID)
    selectedFilters.value = projectId
        ? [...other, { type: FILTER_TYPE_ID, option: formatOption(projectId, ['required']) }]
        : other
}

function addIncludedProject(projectId: string | undefined) {
    if (projectId && !dependencyProjectIds.value.includes(projectId))
        selectedFilters.value = [
            ...selectedFilters.value,
            { type: FILTER_TYPE_ID, option: projectId },
        ]
    pendingProjectId.value = undefined
}

function removeIncludedProject(projectId: string) {
    selectedFilters.value = selectedFilters.value.filter(
        (filter) =>
            filter.type !== FILTER_TYPE_ID || parseOption(filter.option).projectId !== projectId,
    )
}

function resetDraftDependencyTypes() {
    draftDependencyTypes.value = [...dependencyTypes.value]
}

function setDraftDependencyTypes(types: DependencyType[]) {
    draftDependencyTypes.value = types
}

function commitDependencyTypes() {
    if (!selectedProjectId.value) return
    const types: DependencyType[] = draftDependencyTypes.value.length
        ? draftDependencyTypes.value
        : ['required']
    selectedFilters.value = selectedFilters.value.map((filter) =>
        filter.type === FILTER_TYPE_ID
            ? { ...filter, option: formatOption(selectedProjectId.value!, types) }
            : filter,
    )
}

const text = defineMessages({
    searchContentPlaceholder: {
        id: 'search.filter.included_content.search_placeholder',
        defaultMessage: 'Search content...',
    },
    searchProjectPlaceholder: {
        id: 'search.filter.dependent_project.search_placeholder',
        defaultMessage: 'Search for a project...',
    },
    removeIncludedProjectTooltip: {
        id: 'search.filter.included_content.remove_project.tooltip',
        defaultMessage: 'Remove project',
    },
    required: { id: 'search.filter.dependency_type.required', defaultMessage: 'Required' },
    optional: { id: 'search.filter.dependency_type.optional', defaultMessage: 'Optional' },
    embedded: { id: 'search.filter.dependency_type.embedded', defaultMessage: 'Embedded' },
})
</script>

<template>
    <div :class="innerPanelClass" class="flex flex-col gap-3">
        <ProjectCombobox
            v-if="isModpack"
            v-model="pendingProjectId"
            :project-types="selectableProjectTypes"
            :exclude-project-ids="dependencyProjectIds"
            :search-placeholder="formatMessage(text.searchContentPlaceholder)"
            :show-chevron="false"
            :sync-with-selection="false"
            :show-search-icon="true"
            @update:model-value="addIncludedProject"
        />
        <ProjectCombobox
            v-else-if="!showSelectedProject"
            ref="projectCombobox"
            :model-value="selectedProjectId"
            :project-types="selectableProjectTypes"
            :search-placeholder="formatMessage(text.searchProjectPlaceholder)"
            :show-chevron="false"
            :sync-with-selection="false"
            :show-search-icon="true"
            @update:model-value="setSelectedProjectId"
        />
        <div v-if="isModpack" class="flex flex-col gap-1">
            <div
                v-for="{ projectId, project } in loadedDependentProjects"
                :key="projectId"
                class="group flex min-w-0 items-center gap-2 rounded-xl px-1 py-1"
            >
                <img
                    v-if="project.icon_url"
                    :src="project.icon_url"
                    :alt="project.title"
                    class="size-6 shrink-0 rounded-md object-cover"
                />
                <PackageIcon v-else class="size-6 shrink-0 text-[var(--color-text-tertiary)]" />
                <span
                    class="min-w-0 flex-1 truncate font-medium text-[var(--color-text-primary)]"
                    >{{ project.title }}</span
                >
                <button
                    type="button"
                    class="rounded border-0 bg-transparent p-1 text-[var(--color-text-tertiary)] hover:text-red"
                    :aria-label="formatMessage(text.removeIncludedProjectTooltip)"
                    @click="removeIncludedProject(projectId)"
                >
                    <XIcon class="size-4" />
                </button>
            </div>
        </div>
        <div
            v-else-if="showSelectedProject"
            class="flex items-center gap-2 rounded-2xl bg-surface-2 p-2.5"
        >
            <img
                v-if="selectedProject?.icon_url"
                :src="selectedProject.icon_url"
                :alt="selectedProject.title"
                class="size-12 shrink-0 rounded-xl object-cover"
            />
            <PackageIcon v-else class="size-12 shrink-0 text-[var(--color-text-tertiary)]" />
            <div class="min-w-0 flex-1">
                <div class="truncate text-base font-bold text-[var(--color-text-primary)]">
                    {{ selectedProject?.title ?? selectedProjectId }}
                </div>
                <MultiSelect
                    :model-value="draftDependencyTypes"
                    :options="dependencyTypeOptions"
                    :clearable="false"
                    fit-content
                    @open="resetDraftDependencyTypes"
                    @close="commitDependencyTypes"
                    @update:model-value="setDraftDependencyTypes"
                />
            </div>
            <button
                type="button"
                class="rounded border-0 bg-transparent p-1 text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]"
                @click="setSelectedProjectId(undefined)"
            >
                <XIcon class="size-5" />
            </button>
        </div>
    </div>
</template>
