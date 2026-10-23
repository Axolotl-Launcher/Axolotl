<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onBeforeUnmount, onMounted, ref, useTemplateRef } from 'vue'

const props = defineProps<{ open: boolean }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    resize: {
        id: 'app.lab.seed-map.split.resize',
        defaultMessage: 'Resize map and structure details',
    },
})
const root = useTemplateRef<HTMLElement>('root')
const fraction = ref(0.62)
const narrow = ref(false)
const dragging = ref(false)
const width = ref(1200)
const height = ref(0)
const minimum = computed(() => Math.max(0.35, 320 / width.value))
const maximum = computed(() => Math.min(0.72, 1 - 296 / width.value))
const visibleSize = computed(() => ({
    width: props.open && !narrow.value ? width.value * fraction.value : width.value,
    height: props.open && narrow.value ? height.value * 0.45 : height.value,
}))
function clamp(value: number) {
    return Math.max(minimum.value, Math.min(maximum.value, value))
}
let observer: ResizeObserver | undefined
function move(event: PointerEvent) {
    if (!dragging.value || !root.value) return
    const rect = root.value.getBoundingClientRect()
    fraction.value = clamp((event.clientX - rect.left) / rect.width)
}
function start(event: PointerEvent) {
    if (event.button !== 0) return
    dragging.value = true
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
}
function stop() {
    dragging.value = false
}
function keydown(event: KeyboardEvent) {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
    event.preventDefault()
    fraction.value =
        event.key === 'Home'
            ? minimum.value
            : event.key === 'End'
              ? maximum.value
              : clamp(fraction.value + (event.key === 'ArrowLeft' ? -0.03 : 0.03))
}
onMounted(() => {
    observer = new ResizeObserver(([entry]) => {
        width.value = entry.contentRect.width
        height.value = entry.contentRect.height
        narrow.value = entry.contentRect.width < 760
        if (!narrow.value) {
            fraction.value = clamp(fraction.value)
        }
    })
    if (root.value) observer.observe(root.value)
})
onBeforeUnmount(() => observer?.disconnect())
</script>

<template>
    <div
        ref="root"
        class="flex min-h-0 min-w-0 flex-1 overflow-hidden"
        :class="{ 'flex-col': narrow, 'select-none': dragging }"
    >
        <div
            class="relative flex min-h-0 min-w-0 flex-1 overflow-hidden"
            :style="open ? { flex: `0 0 ${narrow ? 45 : fraction * 100}%` } : undefined"
        >
            <slot
                :canvas-size="height > 0 ? { width, height } : undefined"
                :visible-size="visibleSize"
            />
        </div>
        <template v-if="open">
            <div
                v-if="!narrow"
                role="separator"
                tabindex="0"
                aria-orientation="vertical"
                :aria-label="formatMessage(messages.resize)"
                :aria-valuenow="Math.round(fraction * 100)"
                :aria-valuemin="Math.round(minimum * 100)"
                :aria-valuemax="Math.round(maximum * 100)"
                class="z-20 flex w-2 shrink-0 cursor-col-resize touch-none items-center justify-center border-x border-surface-4 bg-surface-3 hover:bg-surface-4 focus-visible:bg-surface-4"
                data-testid="seed-map-details-divider"
                @pointerdown="start"
                @pointermove="move"
                @pointerup="stop"
                @pointercancel="stop"
                @lostpointercapture="stop"
                @keydown.stop="keydown"
                @dblclick="fraction = clamp(0.62)"
            >
                <span class="h-8 w-0.5 rounded bg-surface-5" />
            </div>
            <div
                class="flex min-h-0 min-w-0 flex-1 overflow-hidden"
                :class="{ 'border-t border-surface-4': narrow }"
            >
                <slot name="details" />
            </div>
        </template>
    </div>
</template>
