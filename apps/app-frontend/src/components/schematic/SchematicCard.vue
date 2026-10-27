<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import type { SchematicItem } from '@/helpers/schematic-browser'

const props = defineProps<{
    item: SchematicItem
}>()

const emit = defineEmits<{
    (event: 'select'): void
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    download: {
        id: 'app.schematic.card.download',
        defaultMessage: 'Download',
    },
    sharePage: {
        id: 'app.schematic.card.share-page',
        defaultMessage: 'Share page',
    },
    external: {
        id: 'app.schematic.card.external',
        defaultMessage: 'External',
    },
})

const thumbnail = computed(() => props.item.images[0] ?? null)

const subtitle = computed(() => {
    const parts: string[] = []

    if (props.item.description) parts.push(props.item.description)
    if (props.item.versions?.length) parts.push(props.item.versions.join(', '))

    return parts.slice(0, 2).join(' · ')
})

const actionLabel = computed(() => {
    switch (props.item.download.delivery) {
        case 'quark':
            return formatMessage(messages.sharePage)
        case 'external':
            return formatMessage(messages.external)
        default:
            return formatMessage(messages.download)
    }
})
</script>

<template>
    <article
        class="group relative flex cursor-pointer gap-3 rounded-md border border-surface-4 bg-surface-2 p-2 hover:border-surface-5 hover:bg-surface-3"
        @click="emit('select')"
    >
        <!-- Thumbnail -->
        <div class="size-16 shrink-0 overflow-hidden rounded-md bg-black/5">
            <img
                v-if="thumbnail"
                :src="thumbnail"
                :alt="item.title"
                class="size-full object-cover"
                loading="lazy"
            />
            <span v-else class="flex h-full items-center justify-center text-xs">{{
                item.format.toUpperCase()
            }}</span>
        </div>

        <!-- Content -->
        <div class="flex min-w-0 flex-1 flex-col gap-0.5">
            <p class="m-0 truncate font-semibold">{{ item.title }}</p>
            <p
                v-if="item.description"
                class="m-0 truncate text-sm text-[var(--color-text-tertiary)]"
            >
                {{ item.description }}
            </p>
            <p v-if="subtitle" class="m-0 text-xs text-[var(--color-text-tertiary)]">
                {{ subtitle }}
            </p>
        </div>

        <!-- Action chip -->
        <span class="shrink-0 rounded px-2 py-1 text-xs">{{ actionLabel }}</span>
    </article>
</template>
