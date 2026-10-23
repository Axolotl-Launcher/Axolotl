<script setup lang="ts">
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import type { StorageNode, StorageNodeType } from './storageData'
import { storageLabels, storageMessages } from './storageMessages'

/** components/ui/settings/storage/StorageUsageChart.vue: the donut beside the total. */
const props = defineProps<{ categories: StorageNode[] }>()
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const hovered = ref<string | null>(null)
const colors: Partial<Record<StorageNodeType, string>> = {
    instances: '--color-green',
    cache: '--color-orange',
    meta: '--color-purple',
    database: '--color-blue',
    backups: '--color-red',
    servers: '--color-blue',
    synced_options: '--color-green',
    components: '--color-purple',
}
const slices = computed(() => {
    const total = props.categories.reduce((sum, category) => sum + category.size.actual, 0)
    let offset = 0
    return props.categories
        .filter((category) => category.size.actual > 0)
        .map((category) => {
            const percent = total > 0 ? (category.size.actual / total) * 100 : 0
            const slice = {
                category,
                percent,
                offset,
                color: `var(${colors[category.type] ?? '--color-text-tertiary'})`,
            }
            offset += percent
            return slice
        })
})
</script>

<template>
    <div class="flex min-w-0 items-center gap-4">
        <svg
            viewBox="0 0 100 100"
            class="size-28 shrink-0 -rotate-90"
            role="img"
            :aria-label="formatMessage(storageMessages.usageChart)"
        >
            <circle cx="50" cy="50" r="36" fill="none" class="stroke-surface-4" stroke-width="16" />
            <circle
                v-for="slice in slices"
                :key="slice.category.id"
                cx="50"
                cy="50"
                r="36"
                fill="none"
                :stroke="slice.color"
                stroke-width="16"
                pathLength="100"
                :stroke-dasharray="`${slice.percent} ${100 - slice.percent}`"
                :stroke-dashoffset="-slice.offset"
                class="transition-opacity duration-150"
                :opacity="hovered && hovered !== slice.category.id ? 0.3 : 1"
                @mouseenter="hovered = slice.category.id"
                @mouseleave="hovered = null"
            >
                <title>
                    {{ formatMessage(storageLabels[slice.category.type]) }} ·
                    {{ formatBytes(slice.category.size.actual) }} · {{ slice.percent.toFixed(1) }}%
                </title>
            </circle>
        </svg>
        <div class="grid min-w-0 grid-cols-1 gap-x-4 gap-y-2 sm:grid-cols-2">
            <div
                v-for="slice in slices"
                :key="slice.category.id"
                class="flex min-w-0 items-start gap-2 transition-opacity duration-150"
                :class="hovered && hovered !== slice.category.id ? 'opacity-30' : ''"
                @mouseenter="hovered = slice.category.id"
                @mouseleave="hovered = null"
            >
                <span
                    class="mt-1 size-2 shrink-0 rounded-full"
                    :style="{ backgroundColor: slice.color }"
                />
                <div class="min-w-0 text-xs">
                    <span class="block">{{
                        formatMessage(storageLabels[slice.category.type])
                    }}</span>
                    <span class="tabular-nums text-[var(--color-text-tertiary)]"
                        >{{ formatBytes(slice.category.size.actual) }} ·
                        {{ slice.percent.toFixed(1) }}%</span
                    >
                </div>
            </div>
        </div>
    </div>
</template>
