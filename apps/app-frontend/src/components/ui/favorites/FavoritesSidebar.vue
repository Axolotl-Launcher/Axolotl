<script setup lang="ts">
import { EditIcon, FolderIcon, PlusIcon, TrashIcon } from '@modrinth/assets'
import {
    Button,
    ConfirmModal,
    defineMessages,
    type FilterType,
    type FilterValue,
    IconButton,
    SearchSidebarFilter,
    TagItem,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import {
    getFavoriteCategoryFilterOpen,
    setFavoriteCategoryFilterOpen,
} from '@/helpers/favorites-filter-state'

export interface FavoriteGroupEntry {
    id: string
    name: string
    count: number
    custom: boolean
}

const props = defineProps<{
    entries: FavoriteGroupEntry[]
    activeGroup: string
    categoryFilterType: FilterType
    categoryFilters: FilterValue[]
}>()

const emit = defineEmits<{
    (e: 'select-group' | 'rename-group' | 'delete-group', groupId: string): void
    (e: 'create-group'): void
    (e: 'update:categoryFilters', filters: FilterValue[]): void
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    groupsTitle: {
        id: 'app.content-favorites.groups.title',
        defaultMessage: 'Groups',
    },
    categoriesTitle: {
        id: 'app.content-favorites.filters.categories',
        defaultMessage: 'Categories',
    },
    newGroup: {
        id: 'app.content-favorites.groups.new',
        defaultMessage: 'New group',
    },
    renameGroup: {
        id: 'app.content-favorites.groups.rename',
        defaultMessage: 'Rename group',
    },
    deleteGroup: {
        id: 'app.content-favorites.groups.delete',
        defaultMessage: 'Delete group',
    },
    deleteDescription: {
        id: 'app.content-favorites.groups.delete-description',
        defaultMessage: 'Favorites in {name} will not be deleted. They become ungrouped instead.',
    },
    noCategories: {
        id: 'app.content-favorites.filters.no-categories',
        defaultMessage: 'No categories to filter yet.',
    },
})

const toggledGroups = ref<string[]>([])
const categoryFilterOpen = ref(getFavoriteCategoryFilterOpen())

function setCategoryFilterOpen(open: boolean) {
    categoryFilterOpen.value = open
    setFavoriteCategoryFilterOpen(open)
}

const selectedFilters = computed<FilterValue[]>({
    get: () => props.categoryFilters,
    set: (filters) => emit('update:categoryFilters', filters),
})

const deleteTarget = ref<FavoriteGroupEntry | null>(null)
const deleteModal = ref<InstanceType<typeof ConfirmModal> | null>(null)

function requestDelete(entry: FavoriteGroupEntry) {
    if (entry.count === 0) {
        emit('delete-group', entry.id)
        return
    }
    deleteTarget.value = entry
    deleteModal.value?.show()
}

function confirmDelete() {
    const target = deleteTarget.value
    deleteTarget.value = null
    if (target) emit('delete-group', target.id)
}
</script>

<template>
    <aside class="favorites-sidebar">
        <section class="flex min-w-0 flex-col gap-2">
            <h2 class="m-0 text-sm font-semibold text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.groupsTitle) }}
            </h2>

            <nav class="flex min-w-0 flex-col gap-0.5">
                <div
                    v-for="entry in entries"
                    :key="entry.id"
                    class="group/favorite-group relative flex min-w-0 items-center rounded-md transition-colors"
                    :class="
                        activeGroup === entry.id
                            ? 'bg-surface-4 text-[var(--color-text-primary)]'
                            : 'text-[var(--color-text-tertiary)] hover:bg-surface-3'
                    "
                >
                    <button
                        type="button"
                        class="flex min-w-0 flex-1 cursor-pointer items-center gap-2 border-0 bg-transparent px-2 py-1.5 text-left text-sm font-medium"
                        :aria-current="activeGroup === entry.id ? 'true' : undefined"
                        @click="emit('select-group', entry.id)"
                    >
                        <FolderIcon
                            class="size-4 shrink-0"
                            :class="
                                activeGroup === entry.id
                                    ? 'text-[var(--color-text-primary)]'
                                    : 'text-[var(--color-text-tertiary)]'
                            "
                        />
                        <span class="min-w-0 truncate">{{ entry.name }}</span>
                        <TagItem
                            class="ml-auto shrink-0 border-surface-3 bg-surface-2 transition-opacity"
                            :class="{
                                'group-hover/favorite-group:opacity-0 group-focus-within/favorite-group:opacity-0':
                                    entry.custom,
                            }"
                        >
                            {{ entry.count }}
                        </TagItem>
                    </button>
                    <span
                        v-if="entry.custom"
                        class="pointer-events-none absolute right-1 top-1/2 flex -translate-y-1/2 items-center rounded-md px-1 opacity-0 transition-opacity group-hover/favorite-group:pointer-events-auto group-hover/favorite-group:opacity-100 group-focus-within/favorite-group:pointer-events-auto group-focus-within/favorite-group:opacity-100"
                        :class="activeGroup === entry.id ? 'bg-surface-4' : 'bg-surface-3'"
                    >
                        <IconButton
                            v-tooltip="formatMessage(messages.renameGroup)"
                            :label="formatMessage(messages.renameGroup)"
                            type="quiet"
                            size="2xs"
                            @click.stop="emit('rename-group', entry.id)"
                        >
                            <EditIcon />
                        </IconButton>
                        <IconButton
                            v-tooltip="formatMessage(messages.deleteGroup)"
                            :label="formatMessage(messages.deleteGroup)"
                            type="quiet"
                            size="2xs"
                            @click.stop="requestDelete(entry)"
                        >
                            <TrashIcon />
                        </IconButton>
                    </span>
                </div>
            </nav>

            <Button type="outlined" size="sm" class="mt-1 w-full" @click="emit('create-group')">
                <PlusIcon />
                {{ formatMessage(messages.newGroup) }}
            </Button>
        </section>

        <div class="h-px shrink-0 bg-surface-4" />

        <section class="flex min-w-0 flex-col gap-2">
            <SearchSidebarFilter
                v-if="categoryFilterType.options.length"
                v-model:selected-filters="selectedFilters"
                v-model:toggled-groups="toggledGroups"
                :filter-type="categoryFilterType"
                :provided-filters="[]"
                :open-by-default="categoryFilterOpen"
                button-class="flex w-full cursor-pointer flex-col gap-2 border-none bg-transparent p-0"
                content-class="mt-2"
                inner-panel-class="p-1"
                @on-open="setCategoryFilterOpen(true)"
                @on-close="setCategoryFilterOpen(false)"
            >
                <template #header>
                    <h2 class="m-0 text-sm font-semibold text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.categoriesTitle) }}
                    </h2>
                </template>
            </SearchSidebarFilter>
            <template v-else>
                <h2 class="m-0 text-sm font-semibold text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.categoriesTitle) }}
                </h2>
                <p class="m-0 px-1 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.noCategories) }}
                </p>
            </template>
        </section>

        <ConfirmModal
            ref="deleteModal"
            :title="formatMessage(messages.deleteGroup)"
            :description="
                formatMessage(messages.deleteDescription, { name: deleteTarget?.name ?? '' })
            "
            :proceed-label="formatMessage(messages.deleteGroup)"
            :proceed-icon="TrashIcon"
            @proceed="confirmDelete"
        />
    </aside>
</template>

<style scoped>
.favorites-sidebar {
    position: sticky;
    top: var(--favorites-sidebar-top, 12rem);
    align-self: start;
    display: flex;
    flex-direction: column;
    gap: var(--gap-md);
    min-width: 0;
    max-height: calc(100vh - var(--favorites-sidebar-top, 12rem) - 1rem);
    padding: var(--gap-lg);
    overflow-y: auto;
    background: var(--surface-2);
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-lg);
}

@media (max-width: 900px) {
    .favorites-sidebar {
        position: static;
        max-height: none;
    }
}
</style>
