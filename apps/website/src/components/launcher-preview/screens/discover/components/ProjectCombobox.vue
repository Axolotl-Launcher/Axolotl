<script lang="ts" setup>
import PackageIcon from '@modrinth/assets/icons/package.svg?component'
import Combobox, { type ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import { h, markRaw, ref, watch } from 'vue'

import { usePreview } from '../../../lib/state'
import { injectBrowseContext } from '../browse-context'

/**
 * Port of the shared `ProjectCombobox` (packages/ui/src/components/project/ProjectCombobox.vue).
 * The app asks the Labrinth search API; the preview searches the loaded Modrinth snapshot.
 */
export interface SearchHit {
    project_id: string
    title: string
    icon_url?: string
    project_type: string
}

const props = withDefaults(
    defineProps<{
        projectTypes?: string[]
        placeholder?: string
        searchPlaceholder?: string
        loadingMessage?: string
        noResultsMessage?: string
        syncWithSelection?: boolean
        showSearchIcon?: boolean
        showChevron?: boolean
        excludeProjectIds?: string[]
    }>(),
    {
        projectTypes: undefined,
        placeholder: 'Select project',
        searchPlaceholder: 'Search by name or paste ID...',
        loadingMessage: 'Loading...',
        noResultsMessage: 'No results found',
        syncWithSelection: true,
        showSearchIcon: false,
        showChevron: true,
        excludeProjectIds: undefined,
    },
)

const ctx = injectBrowseContext()
const preview = usePreview()
const projectId = defineModel<string>()

const searchLoading = ref(false)
const options = ref<ComboboxOption<string>[]>([])
const selectedProject = ref<SearchHit | null>(null)
const cache = new Map<string, SearchHit>()

function hitToOption(hit: SearchHit): ComboboxOption<string> {
    return {
        label: hit.title,
        value: hit.project_id,
        icon: hit.icon_url
            ? markRaw({
                  setup: () => () =>
                      h('img', { src: hit.icon_url, alt: hit.title, class: 'h-5 w-5 rounded' }),
              })
            : markRaw(PackageIcon),
    }
}

watch(
    projectId,
    (id) => {
        if (!id) {
            selectedProject.value = null
            return
        }
        let hit = cache.get(id) ?? null
        if (!hit) {
            const project = ctx.findProject(id)
            if (project) {
                hit = {
                    project_id: project.id,
                    title: project.title,
                    icon_url: project.icon_url,
                    project_type: '',
                }
                cache.set(id, hit)
            }
        }
        selectedProject.value = hit
        if (hit && !options.value.some((option) => option.value === hit!.project_id)) {
            options.value = [hitToOption(hit), ...options.value]
        }
    },
    { immediate: true },
)

async function search(query: string) {
    query = query.trim()
    if (!query) {
        searchLoading.value = false
        options.value = []
        return
    }
    const hits = await ctx.searchProjects(query, props.projectTypes ?? [])
    const excluded = new Set(props.excludeProjectIds ?? [])
    const unique: SearchHit[] = []
    for (const hit of hits) {
        if (
            excluded.has(hit.project_id) ||
            unique.some((entry) => entry.project_id === hit.project_id)
        )
            continue
        unique.push(hit)
        cache.set(hit.project_id, hit)
    }
    options.value = unique.slice(0, 20).map(hitToOption)
    searchLoading.value = false
}

let cancelPending: (() => void) | null = null

function handleSearch(query: string) {
    searchLoading.value = true
    cancelPending?.()
    cancelPending = preview.clock.after(250, () => {
        cancelPending = null
        void search(query)
    })
}

defineExpose({ selectedProject })
</script>

<template>
    <Combobox
        v-model="projectId"
        :placeholder="placeholder"
        :options="options"
        :searchable="true"
        :search-placeholder="searchPlaceholder"
        :no-options-message="searchLoading ? loadingMessage : noResultsMessage"
        :disable-search-filter="true"
        :sync-with-selection="syncWithSelection"
        :show-search-icon="showSearchIcon"
        :show-chevron="showChevron"
        show-icon-in-selected
        @search-input="handleSearch"
    />
</template>
