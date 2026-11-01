<script setup lang="ts">
import { MaximizeIcon, ZoomInIcon, ZoomOutIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { clampPreviewScale, fitPreviewScale } from './image-preview-geometry'

const props = defineProps<{
    kind: 'image' | 'video'
    url: string
    name: string
    bytes: number
    loading: boolean
    failed: boolean
}>()

const emit = defineEmits<{ openInSystem: [] }>()

const messages = defineMessages({
    loading: {
        id: 'instance.files.studio.preview-loading',
        defaultMessage: 'Loading preview...',
    },
    previewFailed: {
        id: 'instance.files.studio.preview-failed',
        defaultMessage: 'This file could not be previewed.',
    },
    openInSystem: {
        id: 'instance.files.studio.open-in-system',
        defaultMessage: 'Open in system',
    },
    zoomOut: {
        id: 'instance.files.studio.preview-zoom-out',
        defaultMessage: 'Zoom out',
    },
    zoomIn: {
        id: 'instance.files.studio.preview-zoom-in',
        defaultMessage: 'Zoom in',
    },
    fitToWindow: {
        id: 'instance.files.studio.preview-fit',
        defaultMessage: 'Fit to window',
    },
    actualSize: {
        id: 'instance.files.studio.preview-actual-size',
        defaultMessage: 'Actual size',
    },
})

const { formatMessage } = useVIntl()

const stage = ref<HTMLElement | null>(null)
const natural = ref({ width: 0, height: 0 })
const viewport = ref({ width: 0, height: 0 })
const zoom = ref(1)
const fitting = ref(true)
const renderFailed = ref(false)
const panning = ref(false)

let stageObserver: ResizeObserver | null = null

const unavailable = computed(() => props.failed || renderFailed.value)
const interactive = computed(() => props.kind === 'image' && !unavailable.value && !props.loading)

const fitScale = computed(() => fitPreviewScale(natural.value, viewport.value))
const displayScale = computed(() => (fitting.value ? fitScale.value : zoom.value))
const imageStyle = computed(() => ({
    width: `${Math.max(1, Math.round(natural.value.width * displayScale.value))}px`,
    height: `${Math.max(1, Math.round(natural.value.height * displayScale.value))}px`,
}))

function formatByteSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function measureStage() {
    const element = stage.value
    if (!element) return
    viewport.value = { width: element.clientWidth, height: element.clientHeight }
}

function onImageLoad(event: Event) {
    const image = event.target as HTMLImageElement
    natural.value = { width: image.naturalWidth, height: image.naturalHeight }
    measureStage()
}

function zoomBy(factor: number) {
    fitting.value = false
    zoom.value = clampPreviewScale(displayScale.value * factor)
}

function showActualSize() {
    fitting.value = false
    zoom.value = 1
}

function onWheel(event: WheelEvent) {
    if (!interactive.value || !event.ctrlKey) return
    event.preventDefault()
    zoomBy(event.deltaY < 0 ? 1.1 : 1 / 1.1)
}

function onPointerDown(event: PointerEvent) {
    const element = stage.value
    if (!interactive.value || event.button !== 0 || !element) return
    const scrollable =
        element.scrollWidth > element.clientWidth || element.scrollHeight > element.clientHeight
    if (!scrollable) return

    panning.value = true
    const origin = {
        x: event.clientX,
        y: event.clientY,
        left: element.scrollLeft,
        top: element.scrollTop,
    }
    element.setPointerCapture(event.pointerId)

    const move = (moveEvent: PointerEvent) => {
        element.scrollLeft = origin.left - (moveEvent.clientX - origin.x)
        element.scrollTop = origin.top - (moveEvent.clientY - origin.y)
    }
    const stop = () => {
        panning.value = false
        element.removeEventListener('pointermove', move)
        element.removeEventListener('pointerup', stop)
        element.removeEventListener('pointercancel', stop)
    }
    element.addEventListener('pointermove', move)
    element.addEventListener('pointerup', stop)
    element.addEventListener('pointercancel', stop)
}

watch(
    () => props.url,
    () => {
        renderFailed.value = false
        natural.value = { width: 0, height: 0 }
        zoom.value = 1
        fitting.value = true
    },
)

onMounted(() => {
    stageObserver = new ResizeObserver(measureStage)
    if (stage.value) stageObserver.observe(stage.value)
    measureStage()
})

onBeforeUnmount(() => {
    stageObserver?.disconnect()
})
</script>

<template>
    <div class="flex size-full min-h-0 flex-col bg-surface-1">
        <div
            v-if="interactive"
            class="flex h-9 shrink-0 items-center gap-1 border-0 border-b border-solid border-surface-4 px-2 text-xs text-[var(--color-text-tertiary)]"
        >
            <Button
                v-tooltip="formatMessage(messages.zoomOut)"
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.zoomOut)"
                @click="zoomBy(1 / 1.25)"
                ><ZoomOutIcon class="size-4"
            /></Button>
            <span class="min-w-14 text-center tabular-nums"
                >{{ Math.round(displayScale * 100) }}%</span
            >
            <Button
                v-tooltip="formatMessage(messages.zoomIn)"
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.zoomIn)"
                @click="zoomBy(1.25)"
                ><ZoomInIcon class="size-4"
            /></Button>
            <Button
                v-tooltip="formatMessage(messages.fitToWindow)"
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.fitToWindow)"
                @click="fitting = true"
                ><MaximizeIcon class="size-4"
            /></Button>
            <Button type="quiet" size="2xs" @click="showActualSize">
                {{ formatMessage(messages.actualSize) }}
            </Button>
            <span class="ml-auto shrink-0 tabular-nums">
                {{ natural.width }} × {{ natural.height }} · {{ formatByteSize(bytes) }}
            </span>
        </div>
        <div
            ref="stage"
            class="relative min-h-0 flex-1 overflow-auto p-4"
            :class="panning ? 'cursor-grabbing' : interactive ? 'cursor-grab' : ''"
            @wheel="onWheel"
            @pointerdown="onPointerDown"
        >
            <div
                v-if="unavailable"
                class="flex size-full flex-col items-center justify-center gap-3 text-center"
                role="status"
            >
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.previewFailed) }}
                </p>
                <Button type="outlined" @click="emit('openInSystem')">
                    {{ formatMessage(messages.openInSystem) }}
                </Button>
            </div>
            <p
                v-else-if="loading"
                class="m-0 flex size-full items-center justify-center text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.loading) }}
            </p>
            <img
                v-else-if="kind === 'image'"
                :src="url"
                :alt="name"
                :style="imageStyle"
                class="preview-transparency m-auto max-w-none select-none"
                :class="{ '[image-rendering:pixelated]': displayScale >= 1 }"
                draggable="false"
                @load="onImageLoad"
                @error="renderFailed = true"
            />
            <video
                v-else
                :src="url"
                controls
                class="m-auto max-h-full max-w-full"
                @error="renderFailed = true"
            />
        </div>
    </div>
</template>

<style scoped>
/* Transparency needs a backdrop, or a texture's cut-out edges are invisible
   against the pane. Utilities cannot express a two-tone tile pattern. */
.preview-transparency {
    background-image:
        linear-gradient(45deg, rgb(128 128 128 / 18%) 25%, transparent 25%),
        linear-gradient(-45deg, rgb(128 128 128 / 18%) 25%, transparent 25%),
        linear-gradient(45deg, transparent 75%, rgb(128 128 128 / 18%) 75%),
        linear-gradient(-45deg, transparent 75%, rgb(128 128 128 / 18%) 75%);
    background-position:
        0 0,
        0 8px,
        8px -8px,
        -8px 0;
    background-size: 16px 16px;
}
</style>
