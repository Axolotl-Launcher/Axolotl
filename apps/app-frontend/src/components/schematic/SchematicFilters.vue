<script setup lang="ts">
import { Button, defineMessages, useVIntl } from '@modrinth/ui'

import type { SchematicSourceId } from '@/helpers/schematic-browser'

const props = defineProps<{
    sources: SchematicSourceId[]
    labels: Record<SchematicSourceId, string>
    status: { state: string; message?: string } | null
    cached: boolean
    loading: boolean
}>()

const emit = defineEmits<{
    (event: 'select', sourceId: SchematicSourceId): void
    (event: 'refresh'): void
}>()

const source = defineModel<SchematicSourceId>('source', { required: true })
const query = defineModel<string>('query', { required: true })

const { formatMessage } = useVIntl()

const messages = defineMessages({
    placeholder: {
        id: 'app.schematic.search.placeholder',
        defaultMessage: 'Search schematics…',
    },
    refresh: {
        id: 'app.schematic.refresh',
        defaultMessage: 'Refresh',
    },
    cached: {
        id: 'app.schematic.cached',
        defaultMessage: 'cached',
    },
})

function pick(id: SchematicSourceId) {
    emit('select', id)
}
</script>

<template>
    <div class="flex flex-col gap-2 p-4">
        <!-- Source chips -->
        <div v-if="props.sources.length" class="flex flex-wrap items-center gap-2 text-sm">
            <Button
                v-for="sourceId in props.sources"
                :key="sourceId"
                size="sm"
                :type="source === sourceId ? 'colored' : 'quiet'"
                @click="pick(sourceId)"
            >
                {{ props.labels[sourceId] }}
            </Button>
        </div>

        <!-- Search input -->
        <input
            v-model="query"
            class="input w-full rounded-md border border-surface-5 bg-surface-2 px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-brand/50"
            type="search"
            :placeholder="formatMessage(messages.placeholder)"
            :disabled="props.loading"
        />

        <!-- Actions row -->
        <div class="flex items-center justify-between gap-2">
            <Button size="sm" :disabled="props.loading" type="quiet" @click="emit('refresh')">
                {{ formatMessage(messages.refresh) }}
            </Button>

            <span v-if="props.cached" class="text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.cached) }}
            </span>
            <span
                v-if="props.status && props.status.state !== 'ok'"
                class="text-xs text-[var(--color-text-tertiary)]"
                :title="props.status.message"
            >
                {{ props.status.state }}
            </span>
        </div>
    </div>
</template>
