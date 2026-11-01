<script setup lang="ts">
import { CropIcon, MaximizeIcon, ZoomInIcon, ZoomOutIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { canCropImage, cropImageBytes } from './image-crop'
import { clampPreviewScale, fitPreviewScale, type PreviewRect } from './image-preview-geometry'
import { studioMediaMimeType } from './useStudioMediaPreview'

const props = defineProps<{
    kind: 'image' | 'video'
    url: string
    name: string
    bytes: number
    loading: boolean
    failed: boolean
}>()

const emit = defineEmits<{
    openInSystem: []
    crop: [bytes: Uint8Array]
    cropFailed: [error: unknown]
}>()

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
    crop: {
        id: 'instance.files.studio.preview-crop',
        defaultMessage: 'Crop',
    },
    applyCrop: {
        id: 'instance.files.studio.preview-apply-crop',
        defaultMessage: 'Crop image',
    },
    cancelCrop: {
        id: 'instance.files.studio.preview-cancel-crop',
        defaultMessage: 'Cancel',
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
const cropping = ref(false)
const applyingCrop = ref(false)
const selection = ref<PreviewRect | null>(null)

let stageObserver: ResizeObserver | null = null
let stopPointerDrag: (() => void) | null = null

const unavailable = computed(() => props.failed || renderFailed.value)
const interactive = computed(() => props.kind === 'image' && !unavailable.value && !props.loading)

const fitScale = computed(() => fitPreviewScale(natural.value, viewport.value))
const displayScale = computed(() => (fitting.value ? fitScale.value : zoom.value))
const imageStyle = computed(() => ({
    width: `${Math.max(1, Math.round(natural.value.width * displayScale.value))}px`,
    height: `${Math.max(1, Math.round(natural.value.height * displayScale.value))}px`,
}))
const selectionStyle = computed(() => {
    const rect = selection.value
    if (!rect) return undefined
    return {
        left: `${rect.left}px`,
        top: `${rect.top}px`,
        width: `${rect.width}px`,
        height: `${rect.height}px`,
    }
})
const canCrop = computed(
    () =>
        interactive.value &&
        natural.value.width > 0 &&
        canCropImage(studioMediaMimeType(props.name)),
)

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

/**
 * Follows a drag on the window rather than the element it started on, so a
 * gesture that leaves the image keeps working instead of stopping at the edge.
 */
function trackPointer(onMove: (event: PointerEvent) => void, onEnd?: () => void) {
    stopPointerDrag?.()
    const stop = () => {
        window.removeEventListener('pointermove', onMove)
        window.removeEventListener('pointerup', stop)
        window.removeEventListener('pointercancel', stop)
        stopPointerDrag = null
        onEnd?.()
    }
    stopPointerDrag = stop
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', stop)
    window.addEventListener('pointercancel', stop)
}

function onPointerDown(event: PointerEvent) {
    const element = stage.value
    if (cropping.value || !interactive.value || event.button !== 0 || !element) return
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
    trackPointer(
        (moveEvent) => {
            element.scrollLeft = origin.left - (moveEvent.clientX - origin.x)
            element.scrollTop = origin.top - (moveEvent.clientY - origin.y)
        },
        () => {
            panning.value = false
        },
    )
}

function toggleCropping() {
    cropping.value = !cropping.value
    selection.value = null
}

function startSelection(event: PointerEvent) {
    const overlay = event.currentTarget as HTMLElement
    const bounds = overlay.getBoundingClientRect()
    if (bounds.width <= 0 || bounds.height <= 0) return
    // The stage pans on pointerdown; a selection must not drag the image instead.
    event.stopPropagation()
    event.preventDefault()

    const origin = { x: event.clientX - bounds.left, y: event.clientY - bounds.top }
    selection.value = { left: origin.x, top: origin.y, width: 0, height: 0 }

    trackPointer(
        (moveEvent) => {
            const x = Math.min(Math.max(moveEvent.clientX - bounds.left, 0), bounds.width)
            const y = Math.min(Math.max(moveEvent.clientY - bounds.top, 0), bounds.height)
            selection.value = {
                left: Math.min(origin.x, x),
                top: Math.min(origin.y, y),
                width: Math.abs(x - origin.x),
                height: Math.abs(y - origin.y),
            }
        },
        () => {
            const rect = selection.value
            // A click without a drag is not a selection.
            if (rect && (rect.width < 1 || rect.height < 1)) selection.value = null
        },
    )
}

async function applyCrop() {
    const rect = selection.value
    if (!rect || applyingCrop.value) return
    applyingCrop.value = true
    try {
        const bytes = await cropImageBytes(
            props.url,
            rect,
            displayScale.value,
            natural.value,
            studioMediaMimeType(props.name),
        )
        emit('crop', bytes)
        cropping.value = false
        selection.value = null
    } catch (error) {
        emit('cropFailed', error)
    } finally {
        applyingCrop.value = false
    }
}

watch(
    () => props.url,
    () => {
        renderFailed.value = false
        natural.value = { width: 0, height: 0 }
        zoom.value = 1
        fitting.value = true
        cropping.value = false
        selection.value = null
    },
)

onMounted(() => {
    stageObserver = new ResizeObserver(measureStage)
    if (stage.value) stageObserver.observe(stage.value)
    measureStage()
})

onBeforeUnmount(() => {
    stageObserver?.disconnect()
    stopPointerDrag?.()
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
            <template v-if="canCrop">
                <Button
                    v-if="!cropping"
                    type="quiet"
                    size="2xs"
                    :aria-label="formatMessage(messages.crop)"
                    @click="toggleCropping"
                    ><CropIcon class="size-4" />{{ formatMessage(messages.crop) }}</Button
                >
                <template v-else>
                    <Button type="quiet" size="2xs" @click="toggleCropping">
                        {{ formatMessage(messages.cancelCrop) }}
                    </Button>
                    <Button
                        type="colored"
                        color="brand"
                        size="2xs"
                        :disabled="!selection || applyingCrop"
                        @click="applyCrop"
                    >
                        {{ formatMessage(messages.applyCrop) }}
                    </Button>
                </template>
            </template>
            <span class="ml-auto shrink-0 tabular-nums">
                {{ natural.width }} × {{ natural.height }} · {{ formatByteSize(bytes) }}
            </span>
        </div>
        <div
            ref="stage"
            class="relative flex min-h-0 flex-1 overflow-auto p-4"
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
            <div v-else-if="kind === 'image'" class="relative m-auto shrink-0" :style="imageStyle">
                <img
                    :src="url"
                    :alt="name"
                    class="preview-transparency block size-full select-none"
                    :class="{ '[image-rendering:pixelated]': displayScale >= 1 }"
                    draggable="false"
                    @load="onImageLoad"
                    @error="renderFailed = true"
                />
                <div
                    v-if="cropping"
                    class="absolute inset-0 cursor-crosshair"
                    @pointerdown="startSelection"
                >
                    <div v-if="!selection" class="absolute inset-0 bg-black/40" />
                    <div
                        v-else
                        class="absolute border-2 border-solid border-brand bg-brand/10"
                        :style="selectionStyle"
                    />
                </div>
            </div>
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
