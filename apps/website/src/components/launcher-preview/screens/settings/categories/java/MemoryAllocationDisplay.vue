<script setup lang="ts">
import SparklesIcon from '@modrinth/assets/icons/sparkles.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { usePreview } from '../../../../lib/state/index'
import { messages } from '../../messages'
import { type MemoryStatus, useJre } from './jre'

/**
 * The memory meter under the memory slider (apps/app-frontend/src/components/ui/
 * MemoryAllocationDisplay.vue): memory in use, the game's allocation and what remains,
 * polled once a second from `get_memory_status`. On macOS memory optimization is
 * unsupported, so its button stays disabled with the app's explanation.
 */
const props = withDefaults(
    defineProps<{
        instanceId?: string
        memory: { maximum: number; automatic: boolean; optimize_before_launch?: boolean }
        showOptimizeButton?: boolean
    }>(),
    { instanceId: undefined, showOptimizeButton: false },
)

const { addNotification, handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { clock } = usePreview()
const jre = useJre()

const status = ref<MemoryStatus | null>(null)
const optimizing = ref(false)
let stopTimer: (() => void) | undefined

const totalGiB = computed(() => (status.value?.total_bytes ?? 0) / 1024 ** 3)
const availableGiB = computed(() => (status.value?.available_bytes ?? 0) / 1024 ** 3)
const usedGiB = computed(() => Math.max(totalGiB.value - availableGiB.value, 0))
const allocatedGiB = computed(() => (status.value?.allocated_mb ?? props.memory.maximum) / 1024)
const allocatedAvailableGiB = computed(() => Math.min(allocatedGiB.value, availableGiB.value))
const remainingGiB = computed(() => Math.max(availableGiB.value - allocatedGiB.value, 0))
const allocationLimited = computed(() => allocatedGiB.value > availableGiB.value)
const optimizationSupported = computed(() => status.value?.optimization_supported ?? false)

function percentage(value: number) {
    return totalGiB.value > 0
        ? `${Math.max(0, Math.min(100, (value / totalGiB.value) * 100))}%`
        : '0%'
}

function formatGiB(value: number) {
    return `${value.toFixed(1)} GB`
}

async function refresh() {
    try {
        status.value = await jre.getMemoryStatus(
            props.instanceId ?? null,
            props.memory.maximum,
            props.memory.automatic,
        )
    } catch {
        // The display retries on the next interval while the backend is starting.
    }
}

async function handleOptimize() {
    if (optimizing.value || !optimizationSupported.value) return
    optimizing.value = true
    try {
        const result = await jre.optimizeMemory()
        if (result?.supported) {
            await refresh()
            addNotification({
                type: 'success',
                title: formatMessage(messages.appMemoryDisplayOptimized),
                text: formatMessage(messages.appMemoryDisplayReclaimed, {
                    memory: formatGiB(result.reclaimed_bytes / 1024 ** 3),
                }),
            })
        }
    } catch (error) {
        handleError(error)
    } finally {
        optimizing.value = false
    }
}

watch(
    () => [props.instanceId, props.memory.maximum, props.memory.automatic],
    () => void refresh(),
)

onMounted(() => {
    void refresh()
    // `setInterval(refresh, 1000)` on the window's clock.
    let elapsed = 0
    stopTimer = clock.every((delta) => {
        elapsed += delta
        if (elapsed < 1000) return
        elapsed = 0
        void refresh()
    })
})

onBeforeUnmount(() => {
    stopTimer?.()
})
</script>

<template>
    <div class="mt-2 min-w-0">
        <div v-if="!status" class="h-10 animate-pulse rounded-lg bg-surface-4" />
        <template v-else>
            <div
                class="bg-bg-gray flex h-2 w-full overflow-hidden rounded-full"
                role="meter"
                :aria-label="formatMessage(messages.appMemoryDisplayUsed)"
                :aria-valuenow="usedGiB + allocatedAvailableGiB"
                aria-valuemin="0"
                :aria-valuemax="totalGiB"
            >
                <span
                    class="bg-gray transition-[width] duration-500"
                    :style="{ width: percentage(usedGiB) }"
                />
                <span
                    class="bg-brand transition-[width] duration-500"
                    :style="{ width: percentage(allocatedAvailableGiB) }"
                />
            </div>
            <div class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs">
                <span class="flex min-w-0 items-center gap-1.5">
                    <span class="size-2 shrink-0 rounded-full bg-gray" />
                    <span class="text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.appMemoryDisplayUsed)
                    }}</span>
                    <span
                        class="whitespace-nowrap font-semibold tabular-nums text-[var(--color-text-primary)]"
                    >
                        {{ formatGiB(usedGiB) }} / {{ formatGiB(totalGiB) }}
                    </span>
                </span>
                <span class="flex min-w-0 items-center gap-1.5">
                    <span class="size-2 shrink-0 rounded-full bg-brand" />
                    <span class="text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.appMemoryDisplayGame)
                    }}</span>
                    <span
                        class="whitespace-nowrap font-semibold tabular-nums text-[var(--color-text-primary)]"
                    >
                        {{ formatGiB(allocatedGiB) }}
                        <span v-if="allocationLimited"
                            >({{
                                formatMessage(messages.appMemoryDisplayAvailable, {
                                    memory: formatGiB(availableGiB),
                                })
                            }})</span
                        >
                    </span>
                </span>
                <span class="flex min-w-0 items-center gap-1.5">
                    <span
                        class="bg-bg-gray size-2 shrink-0 rounded-full ring-1 ring-inset ring-divider"
                    />
                    <span class="text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.appMemoryDisplayRemaining)
                    }}</span>
                    <span
                        class="whitespace-nowrap font-semibold tabular-nums text-[var(--color-text-primary)]"
                    >
                        {{ formatGiB(remainingGiB) }}
                    </span>
                </span>
            </div>
            <div
                v-if="showOptimizeButton"
                class="mt-3 flex flex-col items-start justify-between gap-3 border-t border-divider pt-3 sm:flex-row sm:items-center"
            >
                <p
                    class="m-0 min-w-0 flex-1 text-xs leading-tight text-[var(--color-text-tertiary)]"
                >
                    {{
                        formatMessage(
                            optimizationSupported
                                ? messages.appMemoryDisplayOptimizationDescription
                                : messages.appMemoryDisplayUnsupported,
                        )
                    }}
                </p>
                <Button :disabled="optimizing || !optimizationSupported" @click="handleOptimize"
                    ><SpinnerIcon v-if="optimizing" class="animate-spin" />
                    <SparklesIcon v-else />
                    {{
                        formatMessage(
                            optimizing
                                ? messages.appMemoryDisplayOptimizing
                                : messages.appMemoryDisplayOptimize,
                        )
                    }}
                </Button>
            </div>
        </template>
    </div>
</template>
