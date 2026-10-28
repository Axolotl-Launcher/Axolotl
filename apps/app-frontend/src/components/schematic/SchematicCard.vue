<script setup lang="ts">
import { ProjectCard } from '@modrinth/ui'
import { computed } from 'vue'

import type { SchematicItem } from '@/helpers/schematic-browser'
import { SCHEMATIC_SOURCE_LABELS } from '@/helpers/schematic-browser'

const props = defineProps<{
    item: SchematicItem
    layout?: 'list' | 'compact' | 'grid'
}>()

const emit = defineEmits<{
    (event: 'select'): void
}>()

const iconUrl = computed(() => props.item.images[0])

const author = computed(() => ({
    name: SCHEMATIC_SOURCE_LABELS[props.item.sourceId] ?? props.item.sourceId,
}))

/**
 * Renders the same card the mods / modpacks tabs use, so the grid keeps their
 * spacing, hover treatment and metadata rows. The source doubles as the author
 * line because a schematic has no uploader field we can rely on.
 */
const tags = computed(() => {
    const result = [props.item.format.toUpperCase()]
    if (props.item.contentKind === 'world') result.push('World')
    return result
})
</script>

<template>
    <ProjectCard
        :layout="props.layout ?? 'list'"
        :link="() => emit('select')"
        :title="props.item.title"
        :icon-url="iconUrl"
        :summary="props.item.description"
        :downloads="props.item.downloads"
        :author="author"
        :tags="tags"
        :all-tags="props.item.versions"
    />
</template>
