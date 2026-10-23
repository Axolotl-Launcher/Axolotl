<script setup lang="ts">
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import StarIcon from '@modrinth/assets/icons/star.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref, watch } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

import zhCN from '~/locales/launcher-preview/lab-a.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'
import { usePreview, usePreviewDomain } from '../../lib/state'

registerPreviewMessages('zh-CN', zhCN)

/**
 * The Lab hub (apps/app-frontend/src/pages/Lab.vue): six tool cards with their
 * covers, a search field, a category filter and a favorites filter. The
 * favorite ids live in the shared launcher state (the app persists them to
 * localStorage); the two filters survive navigation in this screen's domain.
 */
type LabCategoryFilter = 'all' | 'creation' | 'maintenance' | 'world'
type LabFavoriteFilter = 'all' | 'favorite' | 'unfavorite'

interface LabTool {
    id: string
    category: Exclude<LabCategoryFilter, 'all'>
    route: string
    cover: string
}

const labTools: readonly LabTool[] = [
    {
        id: 'skin-editor',
        category: 'creation',
        route: '/lab/skin-editor',
        cover: '/launcher-preview/lab/skin-editor-tool-cover.webp',
    },
    {
        id: 'gradient-text',
        category: 'creation',
        route: '/lab/gradient-text',
        cover: '/launcher-preview/lab/gradient-text-tool-cover.webp',
    },
    {
        id: 'seed-map',
        category: 'world',
        route: '/lab/seed-map',
        cover: '/launcher-preview/lab/seed-map-tool-cover.webp',
    },
    {
        id: 'schematic-preview',
        category: 'creation',
        route: '/lab/schematic-preview',
        cover: '/launcher-preview/lab/schematic-preview-cover.webp',
    },
    {
        id: 'mod-translation',
        category: 'maintenance',
        route: '/lab/mod-translation',
        cover: '/launcher-preview/lab/mod-translation-cover.webp',
    },
    {
        id: 'recipe-generator',
        category: 'creation',
        route: '/lab/recipe-generator',
        cover: '/launcher-preview/lab/recipe-generator-tool-cover.webp',
    },
    {
        id: 'command-generator',
        category: 'creation',
        route: '/lab/command-generator',
        cover: '/launcher-preview/lab/command-generator-tool-cover.webp',
    },
]

const messages = defineMessages({
    title: { id: 'app.lab.title', defaultMessage: 'Lab' },
    toolCount: { id: 'app.lab.tool-count', defaultMessage: '{count} tool' },
    toolCountPlural: { id: 'app.lab.tool-count-plural', defaultMessage: '{count} tools' },
    search: { id: 'app.lab.search', defaultMessage: 'Search tools' },
    allTools: { id: 'app.lab.category.all', defaultMessage: 'All tools' },
    creation: { id: 'app.lab.category.creation', defaultMessage: 'Creation' },
    maintenance: { id: 'app.lab.category.maintenance', defaultMessage: 'Maintenance' },
    world: { id: 'app.lab.category.world', defaultMessage: 'World' },
    enter: { id: 'app.lab.enter', defaultMessage: 'Enter' },
    favoriteFilterAll: { id: 'app.lab.favorite-filter.all', defaultMessage: 'All' },
    favoriteFilterFavorite: {
        id: 'app.lab.favorite-filter.favorite',
        defaultMessage: 'Favorited',
    },
    favoriteFilterUnfavorite: {
        id: 'app.lab.favorite-filter.unfavorite',
        defaultMessage: 'Not favorited',
    },
    favoriteAdd: { id: 'app.lab.favorite.add', defaultMessage: 'Add to favorites' },
    favoriteRemove: { id: 'app.lab.favorite.remove', defaultMessage: 'Remove from favorites' },
    noResults: { id: 'app.lab.no-results', defaultMessage: 'No tools match your search.' },
    noFavorites: {
        id: 'app.lab.no-favorites',
        defaultMessage: 'You have not favorited any tools yet.',
    },
    gradientTextTitle: {
        id: 'app.lab.gradient-text.title',
        defaultMessage: 'Gradient text generator',
    },
    gradientTextDescription: {
        id: 'app.lab.gradient-text.description',
        defaultMessage: 'Create Minecraft-ready gradient text without a browser.',
    },
    recipeGeneratorTitle: {
        id: 'app.lab.recipe-generator.title',
        defaultMessage: 'Recipe generator',
    },
    recipeGeneratorDescription: {
        id: 'app.lab.recipe-generator.description',
        defaultMessage: 'Create Minecraft Java data pack recipes from local item and tag data.',
    },
    seedMapTitle: { id: 'app.lab.seed-map.title', defaultMessage: 'Seed map' },
    seedMapDescription: {
        id: 'app.lab.seed-map.description',
        defaultMessage:
            'Explore a Minecraft seed locally with biomes, structures, and saved markers.',
    },
    schematicPreviewTitle: {
        id: 'app.lab.schematic-preview.title',
        defaultMessage: 'Schematic workshop',
    },
    schematicPreviewDescription: {
        id: 'app.lab.schematic-preview.description',
        defaultMessage: 'Quickly preview and edit your schematics.',
    },
    modTranslationTitle: {
        id: 'app.lab.mod-translation.title',
        defaultMessage: 'Mod translation',
    },
    modTranslationDescription: {
        id: 'app.lab.mod-translation.description',
        defaultMessage: 'Translate any Minecraft mod JAR into Simplified Chinese.',
    },
    skinEditorTitle: { id: 'app.lab.skin-editor.title', defaultMessage: 'Skin editor' },
    skinEditorDescription: {
        id: 'app.lab.skin-editor.description',
        defaultMessage: 'Create and edit Minecraft player skins locally.',
    },
    commandGeneratorTitle: {
        id: 'app.lab.command-generator.title',
        defaultMessage: 'Command generator',
    },
    commandGeneratorDescription: {
        id: 'app.lab.command-generator.description',
        // The app shows its en-US catalog text, not Lab.vue's default.
        defaultMessage: 'Build Minecraft Java Edition commands visually.',
    },
})

const { formatMessage } = useVIntl()
const router = useRouter()
const state = usePreview()

const domain = usePreviewDomain('lab.hub', () => {
    const category = ref<LabCategoryFilter>('all')
    const favoriteFilter = ref<LabFavoriteFilter>('all')
    return { value: { category, favoriteFilter } }
})
const { category, favoriteFilter } = domain

const search = ref('')
const favoriteToolIds = state.ui.labFavorites

function isFavorite(toolId: string) {
    return favoriteToolIds.value.includes(toolId)
}

function toggleFavorite(toolId: string) {
    state.ui.toggleLabFavorite(toolId)
}

const categoryOptions: LabCategoryFilter[] = ['all', 'creation', 'maintenance', 'world']
const favoriteFilterOptions: LabFavoriteFilter[] = ['all', 'favorite', 'unfavorite']

const visibleTools = computed(() => {
    const normalizedSearch = search.value.trim().toLocaleLowerCase()
    const favoriteSet = new Set(favoriteToolIds.value)

    const filtered = labTools.filter((tool) => {
        const matchingCategory = category.value === 'all' || tool.category === category.value
        const matchingFavorite =
            favoriteFilter.value === 'all' ||
            (favoriteFilter.value === 'favorite' && favoriteSet.has(tool.id)) ||
            (favoriteFilter.value === 'unfavorite' && !favoriteSet.has(tool.id))
        const matchingSearch =
            !normalizedSearch ||
            [toolTitle(tool), toolDescription(tool)]
                .join(' ')
                .toLocaleLowerCase()
                .includes(normalizedSearch)

        return matchingCategory && matchingFavorite && matchingSearch
    })

    const favorited = filtered.filter((tool) => favoriteSet.has(tool.id))
    const unfavorited = filtered.filter((tool) => !favoriteSet.has(tool.id))
    return [...favorited, ...unfavorited]
})

const emptyHeading = computed(() => {
    if (favoriteFilter.value === 'favorite' && favoriteToolIds.value.length === 0) {
        return formatMessage(messages.noFavorites)
    }
    return formatMessage(messages.noResults)
})

function toolTitle(tool: LabTool) {
    if (tool.id === 'skin-editor') return formatMessage(messages.skinEditorTitle)
    if (tool.id === 'gradient-text') return formatMessage(messages.gradientTextTitle)
    if (tool.id === 'recipe-generator') return formatMessage(messages.recipeGeneratorTitle)
    if (tool.id === 'seed-map') return formatMessage(messages.seedMapTitle)
    if (tool.id === 'schematic-preview') return formatMessage(messages.schematicPreviewTitle)
    if (tool.id === 'mod-translation') return formatMessage(messages.modTranslationTitle)
    if (tool.id === 'command-generator') return formatMessage(messages.commandGeneratorTitle)
    return tool.id
}

function toolDescription(tool: LabTool) {
    if (tool.id === 'skin-editor') return formatMessage(messages.skinEditorDescription)
    if (tool.id === 'gradient-text') return formatMessage(messages.gradientTextDescription)
    if (tool.id === 'recipe-generator') return formatMessage(messages.recipeGeneratorDescription)
    if (tool.id === 'seed-map') return formatMessage(messages.seedMapDescription)
    if (tool.id === 'schematic-preview') return formatMessage(messages.schematicPreviewDescription)
    if (tool.id === 'mod-translation') return formatMessage(messages.modTranslationDescription)
    if (tool.id === 'command-generator') return formatMessage(messages.commandGeneratorDescription)
    return tool.id
}

function toolOnboardingId(toolId: string) {
    return ['gradient-text', 'recipe-generator', 'seed-map', 'schematic-preview'].includes(toolId)
        ? `lab-${toolId}-card`
        : undefined
}

function categoryLabel(value: LabCategoryFilter | LabTool['category']) {
    if (value === 'all') return formatMessage(messages.allTools)
    return formatMessage(messages[value])
}

function favoriteFilterLabel(value: LabFavoriteFilter) {
    if (value === 'all') return formatMessage(messages.favoriteFilterAll)
    if (value === 'favorite') return formatMessage(messages.favoriteFilterFavorite)
    return formatMessage(messages.favoriteFilterUnfavorite)
}

function openFirstVisible() {
    const first = visibleTools.value[0]
    if (first) router.push(first.route)
}

/**
 * In-window stand-in for the shared `Combobox` menu: the real dropdown is
 * teleported to a fixed layer and cannot track a CSS-scaled window, so the
 * panel is anchored to its trigger inside the card flow instead. Option look
 * and keyboard handling follow Combobox.vue (Enter/Space/Arrows open and move,
 * Enter/Space select, Esc closes and refocuses the trigger).
 */
const openMenu = ref<'category' | 'favorite' | null>(null)
const focusedIndex = ref(-1)

function toggleMenu(which: 'category' | 'favorite') {
    openMenu.value = openMenu.value === which ? null : which
    focusedIndex.value = -1
}

function closeMenu() {
    openMenu.value = null
    focusedIndex.value = -1
}

function menuKeydown(event: KeyboardEvent, which: 'category' | 'favorite', count: number) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault()
        if (openMenu.value !== which) {
            openMenu.value = which
            focusedIndex.value = -1
            return
        }
        const direction = event.key === 'ArrowDown' ? 1 : -1
        focusedIndex.value = (focusedIndex.value + direction + count) % count
        return
    }
    if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault()
        if (openMenu.value === which && focusedIndex.value >= 0) {
            const value =
                which === 'category'
                    ? categoryOptions[focusedIndex.value]
                    : favoriteFilterOptions[focusedIndex.value]
            pickMenu(which, value)
            return
        }
        openMenu.value = openMenu.value === which ? null : which
        focusedIndex.value = -1
        return
    }
    if (event.key === 'Escape' && openMenu.value === which) {
        event.preventDefault()
        closeMenu()
    }
}

function pickMenu(which: 'category' | 'favorite', value: string) {
    if (which === 'category') category.value = value as LabCategoryFilter
    else favoriteFilter.value = value as LabFavoriteFilter
    closeMenu()
}

watch(openMenu, (menu) => {
    if (!menu) return
    const onPointerDown = (event: PointerEvent) => {
        const target = event.target as HTMLElement | null
        if (!target?.closest('[data-lab-filter-menu]')) closeMenu()
    }
    document.addEventListener('pointerdown', onPointerDown, true)
    return () => document.removeEventListener('pointerdown', onPointerDown, true)
})
</script>

<template>
    <main class="flex w-full flex-col gap-6 p-6">
        <header class="flex min-w-0 items-start justify-between gap-4">
            <div class="min-w-0">
                <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.title) }}
                </h1>
                <p class="m-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                    {{
                        formatMessage(
                            labTools.length === 1 ? messages.toolCount : messages.toolCountPlural,
                            { count: labTools.length },
                        )
                    }}
                </p>
            </div>
        </header>

        <div class="flex flex-wrap gap-2" aria-label="Lab tool filters">
            <StyledInput
                v-model="search"
                :icon="SearchIcon"
                :placeholder="formatMessage(messages.search)"
                clearable
                wrapper-class="min-w-[14rem] flex-1"
                @keydown.enter.prevent="openFirstVisible"
            />
            <div class="relative w-48" data-lab-filter-menu>
                <div
                    role="button"
                    tabindex="0"
                    class="relative flex min-h-5 w-full cursor-pointer items-center justify-between gap-2.5 overflow-hidden rounded-xl bg-surface-4 px-4 py-2 text-left text-button-text transition-all duration-200 hover:brightness-[115%] active:brightness-[115%]"
                    :aria-expanded="openMenu === 'category'"
                    aria-haspopup="listbox"
                    @click="toggleMenu('category')"
                    @keydown="menuKeydown($event, 'category', categoryOptions.length)"
                >
                    <span
                        class="min-w-0 truncate font-semibold leading-tight text-[var(--color-text-default)]"
                    >
                        {{ categoryLabel(category) }}
                    </span>
                    <ChevronRightIcon
                        class="size-5 shrink-0 -rotate-90 transition-transform duration-150"
                        :class="{ 'rotate-90': openMenu === 'category' }"
                    />
                </div>
                <Transition
                    enter-active-class="transition-opacity duration-150"
                    leave-active-class="transition-opacity duration-150"
                    enter-from-class="opacity-0"
                    leave-to-class="opacity-0"
                >
                    <div
                        v-if="openMenu === 'category'"
                        role="listbox"
                        class="absolute left-0 right-0 top-[calc(100%+8px)] z-[30] flex max-h-[300px] flex-col overflow-y-auto overscroll-contain rounded-[14px] border border-solid border-surface-5 bg-surface-4 shadow-2xl"
                    >
                        <button
                            v-for="(value, index) in categoryOptions"
                            :key="value"
                            type="button"
                            role="option"
                            :aria-selected="category === value"
                            class="flex w-full cursor-pointer items-center justify-between gap-2 border-0 px-4 py-3 text-left font-semibold"
                            :class="[
                                category === value
                                    ? 'bg-brand-highlight text-brand'
                                    : 'bg-transparent text-[var(--color-text-default)]',
                                focusedIndex === index ? 'brightness-110' : '',
                            ]"
                            @click="pickMenu('category', value)"
                        >
                            {{ categoryLabel(value) }}
                        </button>
                    </div>
                </Transition>
            </div>
            <div class="relative w-48" data-lab-filter-menu>
                <div
                    role="button"
                    tabindex="0"
                    class="relative flex min-h-5 w-full cursor-pointer items-center justify-between gap-2.5 overflow-hidden rounded-xl bg-surface-4 px-4 py-2 text-left text-button-text transition-all duration-200 hover:brightness-[115%] active:brightness-[115%]"
                    :aria-expanded="openMenu === 'favorite'"
                    aria-haspopup="listbox"
                    @click="toggleMenu('favorite')"
                    @keydown="menuKeydown($event, 'favorite', favoriteFilterOptions.length)"
                >
                    <span
                        class="min-w-0 truncate font-semibold leading-tight text-[var(--color-text-default)]"
                    >
                        {{ favoriteFilterLabel(favoriteFilter) }}
                    </span>
                    <ChevronRightIcon
                        class="size-5 shrink-0 -rotate-90 transition-transform duration-150"
                        :class="{ 'rotate-90': openMenu === 'favorite' }"
                    />
                </div>
                <Transition
                    enter-active-class="transition-opacity duration-150"
                    leave-active-class="transition-opacity duration-150"
                    enter-from-class="opacity-0"
                    leave-to-class="opacity-0"
                >
                    <div
                        v-if="openMenu === 'favorite'"
                        role="listbox"
                        class="absolute left-0 right-0 top-[calc(100%+8px)] z-[30] flex max-h-[300px] flex-col overflow-y-auto overscroll-contain rounded-[14px] border border-solid border-surface-5 bg-surface-4 shadow-2xl"
                    >
                        <button
                            v-for="(value, index) in favoriteFilterOptions"
                            :key="value"
                            type="button"
                            role="option"
                            :aria-selected="favoriteFilter === value"
                            class="flex w-full cursor-pointer items-center justify-between gap-2 border-0 px-4 py-3 text-left font-semibold"
                            :class="[
                                favoriteFilter === value
                                    ? 'bg-brand-highlight text-brand'
                                    : 'bg-transparent text-[var(--color-text-default)]',
                                focusedIndex === index ? 'brightness-110' : '',
                            ]"
                            @click="pickMenu('favorite', value)"
                        >
                            {{ favoriteFilterLabel(value) }}
                        </button>
                    </div>
                </Transition>
            </div>
        </div>

        <section
            v-if="visibleTools.length"
            aria-label="Lab tools"
            data-onboarding-id="lab-tools"
            class="flex flex-col gap-3"
        >
            <div
                v-for="tool in visibleTools"
                :key="tool.id"
                class="lab-tool-card relative m-0 flex items-end gap-4 p-4 transition-[border-color,filter] duration-200 hover:!border-surface-5 hover:brightness-[1.05]"
            >
                <button
                    class="absolute right-4 top-4 z-20 flex size-8 items-center justify-center rounded-md text-[var(--color-text-tertiary)] transition-colors hover:text-brand focus-visible:outline-none"
                    :aria-label="
                        isFavorite(tool.id)
                            ? formatMessage(messages.favoriteRemove)
                            : formatMessage(messages.favoriteAdd)
                    "
                    :aria-pressed="isFavorite(tool.id)"
                    @click="toggleFavorite(tool.id)"
                >
                    <StarIcon
                        class="size-5"
                        :class="
                            isFavorite(tool.id)
                                ? 'fill-brand text-brand'
                                : 'text-[var(--color-text-tertiary)]'
                        "
                    />
                </button>

                <RouterLink
                    :to="tool.route"
                    aria-hidden="true"
                    tabindex="-1"
                    class="shrink-0 rounded-[var(--radius-lg)] focus-visible:outline-none"
                >
                    <div
                        class="relative aspect-[2/1] w-56 overflow-hidden rounded-[var(--radius-lg)] bg-surface-2"
                    >
                        <img
                            :src="tool.cover"
                            alt=""
                            class="absolute inset-0 h-full w-full object-cover"
                        />
                    </div>
                </RouterLink>

                <div class="flex min-w-0 flex-1 flex-col">
                    <RouterLink
                        :to="tool.route"
                        :data-onboarding-id="toolOnboardingId(tool.id)"
                        class="min-w-0 rounded-[var(--radius-lg)] text-inherit no-underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand"
                    >
                        <h2
                            class="m-0 line-clamp-1 pr-10 text-lg font-bold leading-tight text-[var(--color-text-primary)]"
                        >
                            {{ toolTitle(tool) }}
                        </h2>
                        <p
                            class="m-0 mt-1 line-clamp-2 text-sm leading-5 text-[var(--color-text-tertiary)]"
                        >
                            {{ toolDescription(tool) }}
                        </p>
                    </RouterLink>
                    <div class="mt-auto flex items-center justify-between gap-3 pt-4">
                        <TagItem>{{ categoryLabel(tool.category) }}</TagItem>
                        <Button
                            type="colored"
                            color="brand"
                            size="sm"
                            class="min-w-20 justify-between px-3"
                            @click="router.push(tool.route)"
                        >
                            <ChevronRightIcon />
                            {{ formatMessage(messages.enter) }}
                        </Button>
                    </div>
                </div>
            </div>
        </section>

        <EmptyState v-else type="no-search-result" :heading="emptyHeading" aria-live="polite" />
    </main>
</template>

<style scoped>
/* packages/assets/styles/classes.scss `.card`, reproduced because the website
   does not load the omorphia classes sheet inside the window. */
.lab-tool-card {
    position: relative;
    min-height: var(--font-size-2xl);
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-lg);
    background-color: var(--surface-3);
    outline: 2px solid transparent;
}
</style>
