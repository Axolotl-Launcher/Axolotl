<script setup lang="ts">
import { ChevronRightIcon, SearchIcon, StarIcon } from '@modrinth/assets'
import {
    Button,
    Card,
    Combobox,
    defineMessages,
    EmptyState,
    StyledInput,
    TagItem,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

import commandGeneratorToolCover from '@/assets/lab/command-generator-tool-cover.png'
import gradientTextToolCover from '@/assets/lab/gradient-text-tool-cover.png'
import modTranslationCover from '@/assets/lab/mod-translation-cover.png'
import recipeGeneratorToolCover from '@/assets/lab/recipe-generator-tool-cover.png'
import schematicPreviewToolCover from '@/assets/lab/schematic-preview-cover.png'
import seedMapToolCover from '@/assets/lab/seed-map-tool-cover.png'
import skinEditorToolCover from '@/assets/lab/skin-editor-tool-cover.png'
import {
    getLabCategoryFilter,
    getLabFavoriteFilter,
    getLabFavoriteToolIds,
    type LabCategoryFilter,
    type LabFavoriteFilter,
    setLabCategoryFilter,
    setLabFavoriteFilter,
    setLabFavoriteToolIds,
} from '@/helpers/lab-preferences'
import { labTools } from '@/lab/registry'

const { formatMessage } = useVIntl()
const router = useRouter()
const search = ref('')
const category = ref<LabCategoryFilter>(getLabCategoryFilter())
const favoriteFilter = ref<LabFavoriteFilter>(getLabFavoriteFilter())
const favoriteToolIds = ref<string[]>(getLabFavoriteToolIds())
const toolCoverImages: Record<string, string> = {
    'gradient-text': gradientTextToolCover,
    'recipe-generator': recipeGeneratorToolCover,
    'schematic-preview': schematicPreviewToolCover,
    'seed-map': seedMapToolCover,
    'mod-translation': modTranslationCover,
    'skin-editor': skinEditorToolCover,
    'command-generator': commandGeneratorToolCover,
}

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
        defaultMessage: 'Generate Minecraft Java Edition commands with visual NBT editing.',
    },
})

const categoryOptions: LabCategoryFilter[] = ['all', 'creation', 'maintenance', 'world']
const favoriteFilterOptions: LabFavoriteFilter[] = ['all', 'favorite', 'unfavorite']

watch(category, (value) => setLabCategoryFilter(value))
watch(favoriteFilter, (value) => setLabFavoriteFilter(value))
watch(favoriteToolIds, (ids) => setLabFavoriteToolIds(ids))

function isFavorite(toolId: string) {
    return favoriteToolIds.value.includes(toolId)
}

function toggleFavorite(toolId: string) {
    favoriteToolIds.value = isFavorite(toolId)
        ? favoriteToolIds.value.filter((id) => id !== toolId)
        : [...favoriteToolIds.value, toolId]
}

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
            [toolTitle(tool.id, tool.title), toolDescription(tool.id, tool.description)]
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

function toolTitle(toolId: string, fallback: string) {
    if (toolId === 'skin-editor') return formatMessage(messages.skinEditorTitle)
    if (toolId === 'gradient-text') return formatMessage(messages.gradientTextTitle)
    if (toolId === 'recipe-generator') return formatMessage(messages.recipeGeneratorTitle)
    if (toolId === 'seed-map') return formatMessage(messages.seedMapTitle)
    if (toolId === 'schematic-preview') return formatMessage(messages.schematicPreviewTitle)
    if (toolId === 'mod-translation') return formatMessage(messages.modTranslationTitle)
    if (toolId === 'command-generator') return formatMessage(messages.commandGeneratorTitle)
    return fallback
}

function toolDescription(toolId: string, fallback: string) {
    if (toolId === 'skin-editor') return formatMessage(messages.skinEditorDescription)
    if (toolId === 'gradient-text') return formatMessage(messages.gradientTextDescription)
    if (toolId === 'recipe-generator') return formatMessage(messages.recipeGeneratorDescription)
    if (toolId === 'seed-map') return formatMessage(messages.seedMapDescription)
    if (toolId === 'schematic-preview') return formatMessage(messages.schematicPreviewDescription)
    if (toolId === 'mod-translation') return formatMessage(messages.modTranslationDescription)
    if (toolId === 'command-generator') return formatMessage(messages.commandGeneratorDescription)
    return fallback
}

function toolOnboardingId(toolId: string) {
    return ['gradient-text', 'recipe-generator', 'seed-map', 'schematic-preview'].includes(toolId)
        ? `lab-${toolId}-card`
        : undefined
}

function toolIconClasses(toolId: string) {
    if (toolId === 'seed-map') return 'bg-highlight-green text-brand'
    return 'bg-brand-highlight text-brand'
}

function categoryLabel(value: LabCategoryFilter) {
    if (value === 'all') return formatMessage(messages.allTools)
    return formatMessage(messages[value])
}

function favoriteFilterLabel(value: LabFavoriteFilter) {
    if (value === 'all') return formatMessage(messages.favoriteFilterAll)
    if (value === 'favorite') return formatMessage(messages.favoriteFilterFavorite)
    return formatMessage(messages.favoriteFilterUnfavorite)
}
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
                            {
                                count: labTools.length,
                            },
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
            />
            <Combobox
                v-model="category"
                :options="categoryOptions.map((value) => ({ value, label: categoryLabel(value) }))"
                width="custom"
                class="w-48 max-[576px]:w-full"
            />
            <Combobox
                v-model="favoriteFilter"
                :options="
                    favoriteFilterOptions.map((value) => ({
                        value,
                        label: favoriteFilterLabel(value),
                    }))
                "
                width="custom"
                class="w-48 max-[576px]:w-full"
            />
        </div>

        <section
            v-if="visibleTools.length"
            aria-label="Lab tools"
            data-onboarding-id="lab-tools"
            class="flex flex-col gap-3"
        >
            <Card
                v-for="tool in visibleTools"
                :key="tool.id"
                class="!m-0 relative flex items-end gap-4 !p-4 transition-[border-color,filter] duration-200 hover:border-surface-5 hover:brightness-[1.05]"
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
                        class="relative aspect-[2/1] w-56 overflow-hidden rounded-[var(--radius-lg)] bg-surface-2 max-[576px]:w-36"
                    >
                        <img
                            v-if="toolCoverImages[tool.id]"
                            :src="toolCoverImages[tool.id]"
                            alt=""
                            class="absolute inset-0 h-full w-full object-cover"
                        />
                        <div
                            v-else
                            class="flex h-full items-center justify-center"
                            :class="toolIconClasses(tool.id)"
                        >
                            <component :is="tool.icon" class="size-8" />
                        </div>
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
                            {{ toolTitle(tool.id, tool.title) }}
                        </h2>
                        <p
                            class="m-0 mt-1 line-clamp-2 text-sm leading-5 text-[var(--color-text-tertiary)]"
                        >
                            {{ toolDescription(tool.id, tool.description) }}
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
            </Card>
        </section>

        <EmptyState v-else type="no-search-result" :heading="emptyHeading" aria-live="polite" />
    </main>
</template>
