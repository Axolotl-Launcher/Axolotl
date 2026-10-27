<script setup lang="ts">
import { Button, defineMessages, EmptyState, SpinnerIcon, useVIntl } from '@modrinth/ui'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import {
    getSchematicCapabilities,
    listSchematics,
    SCHEMATIC_SOURCE_IDS,
    SCHEMATIC_SOURCE_LABELS,
    type SchematicItem,
    type SchematicSourceId,
    type SchematicSourceStatus,
} from '@/helpers/schematic-browser'

import SchematicCard from './SchematicCard.vue'
import SchematicDetailModal from './SchematicDetailModal.vue'
import SchematicFilters from './SchematicFilters.vue'

const props = defineProps<{
    /** Target instance for downloads; absent means browsing only. */
    instanceId?: string | null
}>()

const PAGE_SIZE = 24

const selectedSource = ref<SchematicSourceId>('reden')
const items = ref<SchematicItem[]>([])
const page = ref(1)
const hasMore = ref(false)
const loading = ref(false)
const error = ref<string | null>(null)
const cached = ref(false)
const searchQuery = ref('')
const selectedItem = ref<SchematicItem | null>(null)
const detailVisible = ref(false)
const sourceStatus = ref<SchematicSourceStatus | null>(null)

let requestSeq = 0

const canLoadMore = computed(() => hasMore.value && !loading.value)

const visibleItems = computed(() => items.value)

async function load(reset: boolean) {
    requestSeq += 1
    const seq = requestSeq
    loading.value = true
    error.value = null

    const targetPage = reset ? 1 : page.value + 1
    try {
        const payload = await listSchematics({
            sourceId: selectedSource.value,
            page: targetPage,
            pageSize: PAGE_SIZE,
            query: searchQuery.value || undefined,
        })
        if (seq !== requestSeq) return
        cached.value = payload.cached
        if (reset) items.value = []
        items.value.push(...payload.items)
        page.value = targetPage
        hasMore.value = payload.has_more
        sourceStatus.value = payload.source_status ?? null
    } catch (e) {
        if (!isUnmounted.value) error.value = e as string
    } finally {
        loading.value = false
    }
}

onMounted(async () => {
    // Capabilities only gate which controls render; a failure here must not
    // stop the listing.
    await getSchematicCapabilities().catch(() => [])
    await load(true)
})

watch(selectedSource, () => void load(true))
watch(searchQuery, () => void load(true))

const isUnmounted = ref(false)
onUnmounted(() => {
    isUnmounted.value = true
})

const { formatMessage } = useVIntl()

const messages = defineMessages({
    loading: { id: 'app.schematic.loading', defaultMessage: 'Loading…' },
    loadMore: { id: 'app.schematic.load-more', defaultMessage: 'Load more' },
    noResults: { id: 'app.schematic.no-results', defaultMessage: 'No schematics found.' },
})

function openDetail(item: SchematicItem) {
    selectedItem.value = item
    detailVisible.value = true
}
</script>

<template>
    <div class="flex min-h-0 flex-1 flex-col gap-3 p-6">
        <!-- Filters -->
        <SchematicFilters
            v-model:source="selectedSource"
            v-model:query="searchQuery"
            :sources="SCHEMATIC_SOURCE_IDS"
            :labels="SCHEMATIC_SOURCE_LABELS"
            :status="sourceStatus"
            :cached="cached"
            :loading="loading"
            @select="load(true)"
            @refresh="load(true)"
        />

        <!-- Loading empty state -->
        <div
            v-if="loading && visibleItems.length === 0"
            class="flex flex-1 items-center justify-center py-16"
        >
            <SpinnerIcon class="animate-spin text-lg" />
        </div>

        <!-- Empty / error states -->
        <EmptyState
            v-else-if="!visibleItems.length && !loading"
            type="empty-inbox"
            :heading="formatMessage(messages.noResults)"
        />

        <!-- List -->
        <ul v-else class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
            <li
                v-for="item in visibleItems"
                :key="item.id + '-' + item.updatedAt"
                class="group/card"
            >
                <SchematicCard :item="item" @select="openDetail(item)" />
            </li>
        </ul>

        <!-- Load more -->
        <div v-if="canLoadMore" class="flex justify-center py-4">
            <Button :disabled="loading" type="quiet" @click="load(false)">
                {{ loading ? formatMessage(messages.loading) : formatMessage(messages.loadMore) }}
            </Button>
        </div>

        <!-- Detail modal -->
        <SchematicDetailModal
            v-model:visible="detailVisible"
            :item="selectedItem"
            :instance-id="props.instanceId ?? null"
            @close="detailVisible = false"
        />
    </div>
</template>
