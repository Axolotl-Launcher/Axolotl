<script setup lang="ts">
import SparklesIcon from '@modrinth/assets/icons/sparkles.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { usePreview } from '../../../../lib/state'
import { memoryAllocationMessages as messages } from '../../messages'
import { MACHINE_TOTAL_MEMORY, useSimulatedMachine } from './machine'
import { automaticMemoryMaxMb, countMods, isModdedLoader } from './memory'

/**
 * Port of `apps/app-frontend/src/components/ui/MemoryAllocationDisplay.vue`.
 * `get_memory_status` reports the simulated Mac; an automatic allocation is
 * computed with the backend's own formula (`automatic_memory_max_mb`) from the
 * available memory and the instance's installed mods.
 */
const props = withDefaults(
    defineProps<{
        instanceId?: string
        memory: { maximum: number; automatic: boolean; optimize_before_launch?: boolean }
        showOptimizeButton?: boolean
    }>(),
    { instanceId: undefined, showOptimizeButton: false },
)

const { formatMessage } = useVIntl()
const preview = usePreview()
const machine = useSimulatedMachine()

type MemoryStatus = {
    total_bytes: number
    available_bytes: number
    allocated_mb: number
    optimization_supported: boolean
}

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

/** `get_memory_status(instanceId, maximum, automatic)`. */
function getMemoryStatus(): MemoryStatus {
    const availableBytes = machine.sampleAvailable()
    const instance = props.instanceId ? preview.instances.get(props.instanceId) : undefined
    const modded = instance ? isModdedLoader(instance.loader) : false
    const modCount = instance ? countMods(preview.downloads.contents.value[instance.id] ?? []) : 0
    return {
        total_bytes: MACHINE_TOTAL_MEMORY,
        available_bytes: availableBytes,
        allocated_mb: props.memory.automatic
            ? automaticMemoryMaxMb(availableBytes, modCount, modded)
            : props.memory.maximum,
        optimization_supported: false,
    }
}

function refresh() {
    status.value = getMemoryStatus()
}

async function handleOptimize() {
    if (optimizing.value || !optimizationSupported.value) return
}

watch(
    () => [props.instanceId, props.memory.maximum, props.memory.automatic],
    () => refresh(),
)

onMounted(() => {
    void preview.downloads.ensureContents().then(() => refresh())
    preview.clock.after(120, () => refresh())
    let elapsed = 0
    stopTimer = preview.clock.every((delta) => {
        elapsed += delta
        if (elapsed < 1000) return
        elapsed = 0
        refresh()
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
                :aria-label="formatMessage(messages.used)"
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
                        formatMessage(messages.used)
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
                        formatMessage(messages.game)
                    }}</span>
                    <span
                        class="whitespace-nowrap font-semibold tabular-nums text-[var(--color-text-primary)]"
                    >
                        {{ formatGiB(allocatedGiB) }}
                        <span v-if="allocationLimited"
                            >({{
                                formatMessage(messages.available, {
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
                        formatMessage(messages.remaining)
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
                class="mt-3 flex flex-row items-center justify-between gap-3 border-t border-divider pt-3"
            >
                <p
                    class="m-0 min-w-0 flex-1 text-xs leading-tight text-[var(--color-text-tertiary)]"
                >
                    {{
                        formatMessage(
                            optimizationSupported
                                ? messages.optimizationDescription
                                : messages.unsupported,
                        )
                    }}
                </p>
                <Button :disabled="optimizing || !optimizationSupported" @click="handleOptimize"
                    ><SpinnerIcon v-if="optimizing" class="animate-spin" />
                    <SparklesIcon v-else />
                    {{ formatMessage(optimizing ? messages.optimizing : messages.optimize) }}
                </Button>
            </div>
        </template>
    </div>
</template>
