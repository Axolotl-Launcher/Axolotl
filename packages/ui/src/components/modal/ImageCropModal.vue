<template>
    <NewModal
        ref="modal"
        :header="resolvedTitle"
        width="64rem"
        max-width="64rem"
        scrollable
        max-content-height="calc(100vh - 13rem)"
        actions-divider
        :disable-close="exporting"
        :on-after-hide="handleAfterHide"
    >
        <div class="grid gap-5 md:grid-cols-[minmax(0,1fr)_18rem]">
            <div class="flex min-w-0 flex-col gap-3">
                <div
                    :ref="setWorkspaceElement"
                    class="relative h-[clamp(18rem,50vh,34rem)] cursor-grab touch-none select-none overflow-hidden rounded-xl border border-solid border-surface-4 bg-surface-1 outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-focus-ring)] active:cursor-grabbing"
                    style="height: clamp(18rem, 50vh, 34rem)"
                    data-image-crop-workspace
                    role="group"
                    tabindex="0"
                    :aria-label="formatMessage(messages.workspace)"
                    :aria-describedby="helpId"
                    @pointerdown="onPointerDown"
                    @pointermove="onPointerMove"
                    @pointerup="onPointerUp"
                    @pointercancel="onPointerUp"
                    @wheel.prevent="onWheel"
                    @keydown="onKeydown"
                >
                    <canvas
                        ref="workspaceCanvas"
                        class="absolute inset-0 h-full w-full"
                        style="position: absolute; inset: 0; width: 100%; height: 100%"
                    />

                    <template v-if="image">
                        <div
                            class="pointer-events-none absolute bg-black/55"
                            :style="maskStyles.top"
                        />
                        <div
                            class="pointer-events-none absolute bg-black/55"
                            :style="maskStyles.bottom"
                        />
                        <div
                            class="pointer-events-none absolute bg-black/55"
                            :style="maskStyles.left"
                        />
                        <div
                            class="pointer-events-none absolute bg-black/55"
                            :style="maskStyles.right"
                        />

                        <div
                            class="pointer-events-none absolute border border-white/90"
                            data-image-crop-frame
                            :style="frameStyle"
                        >
                            <div class="absolute inset-y-0 left-1/3 w-px bg-white/25" />
                            <div class="absolute inset-y-0 left-2/3 w-px bg-white/25" />
                            <div class="absolute inset-x-0 top-1/3 h-px bg-white/25" />
                            <div class="absolute inset-x-0 top-2/3 h-px bg-white/25" />
                        </div>
                    </template>

                    <div
                        v-if="loadState === 'loading'"
                        class="crop-loading-scrim absolute inset-0 flex items-center justify-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                    >
                        <SpinnerIcon class="size-5 animate-spin" />
                        {{ formatMessage(messages.loading) }}
                    </div>
                    <div
                        v-else-if="loadError"
                        role="alert"
                        class="crop-error-scrim absolute inset-0 flex items-center justify-center p-6 text-center text-sm text-red"
                    >
                        {{ loadError }}
                    </div>
                    <div
                        v-else-if="loadState === 'oversized'"
                        data-image-crop-oversized
                        class="crop-oversized-scrim absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 p-6 text-center"
                    >
                        <p class="m-0 font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.oversizedTitle) }}
                        </p>
                        <p class="m-0 max-w-md text-sm text-[var(--color-text-tertiary)]">
                            {{ oversizedDescription }}
                        </p>
                        <div class="flex flex-wrap justify-center gap-2">
                            <Button
                                type="colored"
                                color="brand"
                                data-image-crop-auto-compress
                                @click="autoCompressOversized"
                            >
                                {{ formatMessage(messages.autoCompress) }}
                            </Button>
                            <Button
                                type="outlined"
                                data-image-crop-keep-original
                                @click="keepOriginalOversized"
                            >
                                {{ formatMessage(messages.keepOriginal) }}
                            </Button>
                        </div>
                        <p class="m-0 max-w-md text-xs text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.oversizedKeepOriginalHint) }}
                        </p>
                    </div>
                </div>
                <p class="m-0 text-xs text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.dragHint) }}
                </p>
            </div>

            <aside class="flex min-w-0 flex-col gap-4">
                <section class="flex flex-col gap-2">
                    <h3 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.preview) }}
                    </h3>
                    <div
                        class="crop-checkerboard relative overflow-hidden rounded-lg border border-solid border-surface-4"
                        :style="
                            animatedPreviewUrl && image
                                ? { aspectRatio: String(selectedAspectRatio) }
                                : undefined
                        "
                    >
                        <img
                            v-if="animatedPreviewUrl && image"
                            :src="animatedPreviewUrl"
                            :alt="formatMessage(messages.animatedPreview)"
                            class="image-crop-animated-preview pointer-events-none select-none"
                            :style="animatedPreviewStyle"
                        />
                        <canvas
                            v-else
                            ref="previewCanvas"
                            class="block h-auto w-full"
                            style="display: block; width: 100%; height: auto"
                            role="img"
                            :aria-label="previewAriaLabel"
                            data-image-crop-preview
                        />
                    </div>
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ dimensionsText }}
                    </p>
                </section>

                <p
                    :id="helpId"
                    class="m-0 text-xs leading-relaxed text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.help) }}
                </p>

                <section v-if="aspectOptions.length > 1" class="flex flex-col gap-2">
                    <h3 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.aspectRatio) }}
                    </h3>
                    <div class="flex flex-wrap gap-2">
                        <Button
                            v-for="option in aspectOptions"
                            :key="option.id"
                            :type="option.id === selectedAspectId ? 'colored' : 'outlined'"
                            :color="option.id === selectedAspectId ? 'brand' : undefined"
                            size="sm"
                            :data-aspect-id="option.id"
                            :disabled="!image || exporting"
                            @click="selectAspect(option.id)"
                        >
                            {{ option.label }}
                        </Button>
                    </div>
                </section>

                <section class="flex flex-col gap-2">
                    <div class="flex items-center justify-between gap-2">
                        <h3 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.zoom) }}
                        </h3>
                        <span
                            class="text-sm tabular-nums text-[var(--color-text-tertiary)]"
                            data-image-crop-zoom
                        >
                            {{ zoomPercent }}%
                        </span>
                    </div>
                    <Slider
                        :model-value="zoomPercent"
                        :min="100"
                        :max="800"
                        :step="5"
                        unit="%"
                        :disabled="!image || exporting"
                        @update:model-value="onZoomInput"
                    />
                    <Button
                        type="outlined"
                        size="sm"
                        :disabled="!image || exporting"
                        @click="resetViewTransform"
                    >
                        <RefreshCwIcon />
                        {{ formatMessage(commonMessages.resetButton) }}
                    </Button>
                </section>

                <p
                    v-if="exportError"
                    role="alert"
                    class="m-0 rounded-lg border border-solid border-red bg-bg-red p-2 text-sm text-red"
                >
                    {{ exportError }}
                </p>
            </aside>
        </div>

        <span class="sr-only" role="status" aria-live="polite">{{ zoomStatus }}</span>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button
                    type="outlined"
                    data-image-crop-cancel
                    :disabled="exporting"
                    @click="cancel"
                >
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    data-image-crop-confirm
                    :disabled="!canConfirm"
                    :loading="exporting"
                    @click="confirm"
                >
                    <CropIcon />
                    {{ resolvedConfirmLabel }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import { CropIcon, RefreshCwIcon, SpinnerIcon } from '@modrinth/assets'
import {
    type ComponentPublicInstance,
    computed,
    type CSSProperties,
    nextTick,
    onBeforeUnmount,
    ref,
    shallowRef,
    watch,
} from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { commonMessages } from '../../utils/common-messages'
import Button from '../base/buttons/Button.vue'
import Slider from '../base/Slider.vue'
import {
    clampCropView,
    coverScale,
    type CropSelection,
    DEFAULT_CROP_ASPECT_RATIO,
    fitCropFrame,
    type ImageCropAspectRatioOption,
    type ImageCropModalExposed,
    type ImageCropOutcome,
    type ImageCropResult,
    type ImageCropSelectionOptions,
    type ImageCropShowOptions,
    MAX_CROP_ZOOM,
    MIN_CROP_ZOOM,
    outputSizeForSource,
    panView,
    resetView,
    sourceRect,
    zoomViewAt,
} from './image-crop'
import NewModal from './NewModal.vue'

const props = withDefaults(
    defineProps<{
        title?: string
        confirmLabel?: string
        outputMimeType?: 'image/png' | 'image/webp'
        outputQuality?: number
        maxOutputDimension?: number
        maxSourceBytes?: number
        maxSourcePixels?: number
    }>(),
    {
        title: undefined,
        confirmLabel: undefined,
        outputMimeType: 'image/png',
        outputQuality: 0.9,
        maxOutputDimension: 4096,
        maxSourceBytes: 64 * 1024 * 1024,
        maxSourcePixels: 64_000_000,
    },
)

const messages = defineMessages({
    title: { id: 'image-crop.title', defaultMessage: 'Crop image' },
    preview: { id: 'image-crop.preview', defaultMessage: 'Result preview' },
    animatedPreview: {
        id: 'image-crop.animated-preview',
        defaultMessage: 'Animated result preview',
    },
    workspace: { id: 'image-crop.workspace', defaultMessage: 'Crop workspace' },
    dragHint: {
        id: 'image-crop.drag-hint',
        defaultMessage: 'Drag the image to reposition it.',
    },
    help: {
        id: 'image-crop.help',
        defaultMessage:
            'Drag the image to reposition it. Scroll over the workspace or use the zoom control to zoom.',
    },
    aspectRatio: { id: 'image-crop.aspect-ratio', defaultMessage: 'Aspect ratio' },
    zoom: { id: 'image-crop.zoom', defaultMessage: 'Zoom' },
    zoomStatus: {
        id: 'image-crop.zoom-status',
        defaultMessage: 'Zoom {percent}%, {width} × {height} pixels',
    },
    loading: { id: 'image-crop.loading', defaultMessage: 'Loading image…' },
    decodeError: {
        id: 'image-crop.decode-error',
        defaultMessage: 'This image could not be opened. Choose a different image.',
    },
    tooLarge: {
        id: 'image-crop.too-large',
        defaultMessage: 'This image is too large to crop.',
    },
    oversizedTitle: {
        id: 'image-crop.oversized-title',
        defaultMessage: 'This file is larger than the usual limit',
    },
    oversizedKeepOriginalHint: {
        id: 'image-crop.oversized-keep-original-hint',
        defaultMessage:
            'Keep original keeps the source resolution and frame rate; the animation may use more memory and CPU.',
    },
    oversizedBytes: {
        id: 'image-crop.oversized-bytes',
        defaultMessage:
            'The source is {size} MB; the usual limit is {limit} MB. Continuing may use a lot of memory.',
    },
    oversizedPixels: {
        id: 'image-crop.oversized-pixels',
        defaultMessage:
            'The image is {width} × {height} pixels; the usual limit is {limit} megapixels. Continuing may use a lot of memory.',
    },
    autoCompress: {
        id: 'image-crop.auto-compress',
        defaultMessage: 'Auto-compress',
    },
    keepOriginal: {
        id: 'image-crop.keep-original',
        defaultMessage: 'Keep original',
    },
    exportError: {
        id: 'image-crop.export-error',
        defaultMessage: 'The cropped image could not be created. Try again.',
    },
    apply: { id: 'image-crop.apply', defaultMessage: 'Use image' },
    dimensions: {
        id: 'image-crop.dimensions',
        defaultMessage: '{width} × {height} px',
    },
    previewLabel: {
        id: 'image-crop.preview-label',
        defaultMessage: 'Cropped result preview, {width} by {height} pixels',
    },
})

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof NewModal>>()

const resolvedTitle = computed(() => props.title ?? formatMessage(messages.title))
const resolvedConfirmLabel = computed(() => props.confirmLabel ?? formatMessage(messages.apply))

const helpId = `image-crop-help-${Math.random().toString(36).slice(2, 8)}`

const image = shallowRef<ImageBitmap | null>(null)
const imageSize = ref({ width: 1, height: 1 })
const workspaceSize = ref({ width: 1, height: 1 })
const view = ref({ zoom: 1, centerX: 1, centerY: 1 })
const loadState = ref<'idle' | 'loading' | 'ready' | 'error' | 'oversized'>('idle')
const loadError = ref('')
const oversizedReason = ref<'bytes' | 'pixels' | null>(null)
const acceptedOversizedSource = ref(false)
const exportError = ref('')
const exporting = ref(false)
const sessionMode = ref<'encode' | 'crop'>('encode')
const sessionMaxOutputDimension = ref(props.maxOutputDimension)
const sessionBaseMaxOutputDimension = ref(props.maxOutputDimension)
const preserveOriginalQuality = ref(false)
const animatedPreviewUrl = ref<string | null>(null)
const aspectOptions = ref<ImageCropAspectRatioOption[]>([
    { id: 'default', label: '16:9', ratio: DEFAULT_CROP_ASPECT_RATIO },
])
const selectedAspectId = ref('default')

const zoomPercent = computed(() => Math.round(view.value.zoom * 100))
const canConfirm = computed(
    () =>
        loadState.value === 'ready' &&
        !!image.value &&
        !exporting.value &&
        workspaceSize.value.width > 2 &&
        workspaceSize.value.height > 2,
)

const selectedAspectRatio = computed(() => {
    const option = aspectOptions.value.find((entry) => entry.id === selectedAspectId.value)
    return option?.ratio ?? DEFAULT_CROP_ASPECT_RATIO
})

const frameRect = computed(() =>
    fitCropFrame(workspaceSize.value.width, workspaceSize.value.height, selectedAspectRatio.value),
)

const frameStyle = computed(() => ({
    left: `${frameRect.value.left}px`,
    top: `${frameRect.value.top}px`,
    width: `${frameRect.value.width}px`,
    height: `${frameRect.value.height}px`,
}))

const maskStyles = computed(() => {
    const frame = frameRect.value
    const workspace = workspaceSize.value
    return {
        top: {
            left: '0px',
            top: '0px',
            width: `${workspace.width}px`,
            height: `${Math.max(0, frame.top)}px`,
        },
        bottom: {
            left: '0px',
            top: `${frame.top + frame.height}px`,
            width: `${workspace.width}px`,
            height: `${Math.max(0, workspace.height - frame.top - frame.height)}px`,
        },
        left: {
            left: '0px',
            top: `${frame.top}px`,
            width: `${Math.max(0, frame.left)}px`,
            height: `${frame.height}px`,
        },
        right: {
            left: `${frame.left + frame.width}px`,
            top: `${frame.top}px`,
            width: `${Math.max(0, workspace.width - frame.left - frame.width)}px`,
            height: `${frame.height}px`,
        },
    }
})

const currentSourceRect = computed(() => sourceRect(view.value, imageSize.value, frameRect.value))

const outputSize = computed(() =>
    outputSizeForSource(currentSourceRect.value, sessionMaxOutputDimension.value),
)

const dimensionsText = computed(() =>
    formatMessage(messages.dimensions, {
        width: outputSize.value.width,
        height: outputSize.value.height,
    }),
)

const previewAriaLabel = computed(() =>
    formatMessage(messages.previewLabel, {
        width: outputSize.value.width,
        height: outputSize.value.height,
    }),
)

const animatedPreviewStyle = computed<CSSProperties>(() => {
    const rect = currentSourceRect.value
    return {
        position: 'absolute',
        left: `${(-rect.left / rect.width) * 100}%`,
        top: `${(-rect.top / rect.height) * 100}%`,
        width: `${(imageSize.value.width / rect.width) * 100}%`,
        height: `${(imageSize.value.height / rect.height) * 100}%`,
        maxWidth: 'none',
    }
})

const oversizedDescription = computed(() => {
    if (oversizedReason.value === 'bytes') {
        const size = ((pendingSource?.size ?? 0) / (1024 * 1024)).toFixed(1)
        const limit = (props.maxSourceBytes / (1024 * 1024)).toFixed(0)
        return formatMessage(messages.oversizedBytes, { size, limit })
    }
    if (oversizedReason.value === 'pixels') {
        return formatMessage(messages.oversizedPixels, {
            width: imageSize.value.width,
            height: imageSize.value.height,
            limit: Math.round(props.maxSourcePixels / 1_000_000),
        })
    }
    return ''
})

const zoomStatus = computed(() =>
    formatMessage(messages.zoomStatus, {
        percent: zoomPercent.value,
        width: outputSize.value.width,
        height: outputSize.value.height,
    }),
)

const workspaceCanvas = ref<HTMLCanvasElement | null>(null)
const previewCanvas = ref<HTMLCanvasElement | null>(null)
let resizeObserver: ResizeObserver | null = null
let drawRequest: number | null = null
let dragState: { pointerId: number; lastX: number; lastY: number } | null = null
let pendingResolve: ((result: ImageCropOutcome | null) => void) | null = null
let pendingResult: ImageCropOutcome | null = null
let pendingSource: Blob | null = null
let sessionGeneration = 0

function setWorkspaceElement(element: Element | ComponentPublicInstance | null) {
    resizeObserver?.disconnect()
    resizeObserver = null
    if (!(element instanceof HTMLElement)) return

    const updateSize = () => {
        const bounds = element.getBoundingClientRect()
        const width = Math.max(1, bounds.width)
        const height = Math.max(1, bounds.height)
        // ResizeObserver fires again when the ref update triggers layout; only
        // publish meaningful changes so the observer cannot loop.
        if (
            Math.abs(width - workspaceSize.value.width) < 0.5 &&
            Math.abs(height - workspaceSize.value.height) < 0.5
        ) {
            return
        }
        workspaceSize.value = { width, height }
    }

    updateSize()
    resizeObserver = new ResizeObserver(updateSize)
    resizeObserver.observe(element)
}

function scheduleDraw() {
    if (drawRequest !== null) return
    drawRequest = requestAnimationFrame(() => {
        drawRequest = null
        drawWorkspace()
        drawPreview()
    })
}

function drawWorkspace() {
    const canvas = workspaceCanvas.value
    const bitmap = image.value
    if (!canvas || !bitmap) return

    const width = workspaceSize.value.width
    const height = workspaceSize.value.height
    const dpr = Math.min(window.devicePixelRatio || 1, 3)
    const pixelWidth = Math.max(1, Math.round(width * dpr))
    const pixelHeight = Math.max(1, Math.round(height * dpr))

    if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
        canvas.width = pixelWidth
        canvas.height = pixelHeight
    }

    const context = canvas.getContext('2d')
    if (!context) return

    context.setTransform(dpr, 0, 0, dpr, 0, 0)
    context.clearRect(0, 0, width, height)
    context.imageSmoothingQuality = 'high'

    const frame = frameRect.value
    const clamped = clampCropView(view.value, imageSize.value, frame)
    const scale = coverScale(imageSize.value, frame) * clamped.zoom
    const left = frame.left + frame.width / 2 - clamped.centerX * scale
    const top = frame.top + frame.height / 2 - clamped.centerY * scale

    context.drawImage(
        bitmap,
        left,
        top,
        imageSize.value.width * scale,
        imageSize.value.height * scale,
    )
}

function drawPreview() {
    const canvas = previewCanvas.value
    const bitmap = image.value
    if (!canvas || !bitmap) return

    const previewWidth = 512
    const previewHeight = Math.max(1, Math.round(previewWidth / selectedAspectRatio.value))
    if (canvas.width !== previewWidth || canvas.height !== previewHeight) {
        canvas.width = previewWidth
        canvas.height = previewHeight
    }

    const context = canvas.getContext('2d')
    if (!context) return

    context.clearRect(0, 0, previewWidth, previewHeight)
    context.imageSmoothingQuality = 'high'
    const rect = currentSourceRect.value
    context.drawImage(
        bitmap,
        rect.left,
        rect.top,
        rect.width,
        rect.height,
        0,
        0,
        previewWidth,
        previewHeight,
    )
}

watch(view, scheduleDraw, { deep: true })
watch(frameRect, scheduleDraw)
watch(image, scheduleDraw)

function onPointerDown(event: PointerEvent) {
    if (!image.value || exporting.value || loadState.value !== 'ready') return
    if (event.button !== 0) return

    const element = event.currentTarget as HTMLElement
    try {
        element.setPointerCapture(event.pointerId)
    } catch {
        // Synthetic pointer events in tests do not create an active pointer.
    }
    element.focus()
    dragState = {
        pointerId: event.pointerId,
        lastX: event.clientX,
        lastY: event.clientY,
    }
}

function onPointerMove(event: PointerEvent) {
    if (!dragState || dragState.pointerId !== event.pointerId || !image.value) return

    const dx = event.clientX - dragState.lastX
    const dy = event.clientY - dragState.lastY
    if (dx === 0 && dy === 0) return

    dragState.lastX = event.clientX
    dragState.lastY = event.clientY
    view.value = panView(view.value, dx, dy, imageSize.value, frameRect.value)
}

function onPointerUp(event: PointerEvent) {
    if (!dragState || dragState.pointerId !== event.pointerId) return

    dragState = null
    const element = event.currentTarget as HTMLElement
    if (element.hasPointerCapture(event.pointerId)) {
        element.releasePointerCapture(event.pointerId)
    }
}

function onWheel(event: WheelEvent) {
    if (!image.value || exporting.value || loadState.value !== 'ready') return

    const element = event.currentTarget as HTMLElement
    const bounds = element.getBoundingClientRect()
    const frame = frameRect.value
    const focalX = event.clientX - bounds.left - (frame.left + frame.width / 2)
    const focalY = event.clientY - bounds.top - (frame.top + frame.height / 2)
    const factor = Math.exp(-event.deltaY * 0.0015)
    const nextZoom = Math.min(MAX_CROP_ZOOM, Math.max(MIN_CROP_ZOOM, view.value.zoom * factor))

    view.value = zoomViewAt(view.value, nextZoom, focalX, focalY, imageSize.value, frame)
}

function onKeydown(event: KeyboardEvent) {
    if (!image.value || exporting.value || loadState.value !== 'ready') return

    const step = event.shiftKey ? 50 : 10
    if (event.key === 'ArrowLeft') {
        event.preventDefault()
        view.value = panView(view.value, -step, 0, imageSize.value, frameRect.value)
    } else if (event.key === 'ArrowRight') {
        event.preventDefault()
        view.value = panView(view.value, step, 0, imageSize.value, frameRect.value)
    } else if (event.key === 'ArrowUp') {
        event.preventDefault()
        view.value = panView(view.value, 0, -step, imageSize.value, frameRect.value)
    } else if (event.key === 'ArrowDown') {
        event.preventDefault()
        view.value = panView(view.value, 0, step, imageSize.value, frameRect.value)
    } else if (event.key === '+' || event.key === '=') {
        event.preventDefault()
        zoomBy(1.15)
    } else if (event.key === '-' || event.key === '_') {
        event.preventDefault()
        zoomBy(1 / 1.15)
    } else if (event.key === '0') {
        event.preventDefault()
        resetViewTransform()
    }
}

function zoomBy(factor: number) {
    view.value = zoomViewAt(
        view.value,
        view.value.zoom * factor,
        0,
        0,
        imageSize.value,
        frameRect.value,
    )
}

function onZoomInput(value: number) {
    if (!image.value) return
    view.value = zoomViewAt(view.value, value / 100, 0, 0, imageSize.value, frameRect.value)
}

function selectAspect(id: string) {
    if (!aspectOptions.value.some((option) => option.id === id)) return
    selectedAspectId.value = id
    if (image.value) {
        view.value = clampCropView(view.value, imageSize.value, frameRect.value)
    }
    scheduleDraw()
}

function resetViewTransform() {
    if (!image.value) return
    view.value = resetView(imageSize.value)
}

function cleanSource() {
    if (animatedPreviewUrl.value) {
        URL.revokeObjectURL(animatedPreviewUrl.value)
        animatedPreviewUrl.value = null
    }
    image.value?.close()
    image.value = null
    imageSize.value = { width: 1, height: 1 }
    view.value = { zoom: 1, centerX: 1, centerY: 1 }
    loadState.value = 'idle'
    loadError.value = ''
    exportError.value = ''
    oversizedReason.value = null
    acceptedOversizedSource.value = false
    preserveOriginalQuality.value = false
    pendingSource = null
    dragState = null
    if (drawRequest !== null) {
        cancelAnimationFrame(drawRequest)
        drawRequest = null
    }
}

function handleAfterHide() {
    const resolve = pendingResolve
    const result = pendingResult
    pendingResolve = null
    pendingResult = null
    sessionGeneration += 1
    cleanSource()
    resolve?.(result)
}

async function showInternal(
    source: Blob,
    options: ImageCropShowOptions | ImageCropSelectionOptions,
    mode: 'encode' | 'crop',
): Promise<ImageCropOutcome | null> {
    if (pendingResolve) return null
    if (!modal.value) return null

    const selectionOptions = options as ImageCropSelectionOptions
    const configuredOptions = (options.aspectRatioOptions ?? []).filter(
        (option) => option.id.length > 0 && Number.isFinite(option.ratio) && option.ratio > 0,
    )
    aspectOptions.value =
        configuredOptions.length > 0
            ? configuredOptions
            : [{ id: 'default', label: '16:9', ratio: DEFAULT_CROP_ASPECT_RATIO }]
    selectedAspectId.value =
        options.defaultAspectRatioId &&
        aspectOptions.value.some((option) => option.id === options.defaultAspectRatioId)
            ? options.defaultAspectRatioId
            : aspectOptions.value[0].id

    const session = ++sessionGeneration
    sessionMode.value = mode
    sessionMaxOutputDimension.value =
        mode === 'crop' && selectionOptions.maxOutputDimension
            ? selectionOptions.maxOutputDimension
            : props.maxOutputDimension
    sessionBaseMaxOutputDimension.value = sessionMaxOutputDimension.value
    preserveOriginalQuality.value = false
    if (animatedPreviewUrl.value) {
        URL.revokeObjectURL(animatedPreviewUrl.value)
    }
    animatedPreviewUrl.value =
        mode === 'crop' && selectionOptions.animated ? URL.createObjectURL(source) : null
    pendingResult = null
    pendingSource = source
    oversizedReason.value = null
    acceptedOversizedSource.value = false
    loadError.value = ''
    exportError.value = ''
    exporting.value = false
    loadState.value = 'loading'
    image.value = null
    imageSize.value = { width: 1, height: 1 }
    view.value = { zoom: 1, centerX: 1, centerY: 1 }

    const promise = new Promise<ImageCropOutcome | null>((resolve) => {
        pendingResolve = resolve
    })

    modal.value.show()
    void loadSource(source, session)
    return promise
}

function show(source: Blob, options: ImageCropShowOptions = {}): Promise<ImageCropResult | null> {
    return showInternal(source, options, 'encode') as Promise<ImageCropResult | null>
}

function showCrop(
    source: Blob,
    options: ImageCropSelectionOptions = {},
): Promise<CropSelection | null> {
    return showInternal(source, options, 'crop') as Promise<CropSelection | null>
}

async function loadSource(source: Blob, session: number, ignoreLimits = false) {
    try {
        if (!ignoreLimits && source.size > props.maxSourceBytes) {
            oversizedReason.value = 'bytes'
            loadState.value = 'oversized'
            return
        }

        const bitmap = await decodeImage(source)
        if (session !== sessionGeneration) {
            bitmap.close()
            return
        }

        if (bitmap.width <= 0 || bitmap.height <= 0) {
            bitmap.close()
            throw new Error('The image has no usable dimensions')
        }

        image.value = bitmap
        imageSize.value = { width: bitmap.width, height: bitmap.height }
        view.value = resetView(imageSize.value)

        if (!ignoreLimits && bitmap.width * bitmap.height > props.maxSourcePixels) {
            oversizedReason.value = 'pixels'
            loadState.value = 'oversized'
            scheduleDraw()
            return
        }

        loadState.value = 'ready'
        scheduleDraw()
    } catch (error) {
        if (session !== sessionGeneration) return
        console.warn('Failed to decode an image for cropping', error)
        loadState.value = 'error'
        loadError.value = formatMessage(messages.decodeError)
    }
}

function autoCompressOversized() {
    if (!pendingSource || !oversizedReason.value) return

    acceptedOversizedSource.value = true
    preserveOriginalQuality.value = false
    sessionMaxOutputDimension.value = sessionBaseMaxOutputDimension.value
    if (oversizedReason.value === 'pixels') {
        oversizedReason.value = null
        loadState.value = 'ready'
        scheduleDraw()
        return
    }

    const session = sessionGeneration
    oversizedReason.value = null
    loadState.value = 'loading'
    void loadSource(pendingSource, session, true)
}

function keepOriginalOversized() {
    if (!pendingSource || !oversizedReason.value) return

    acceptedOversizedSource.value = true
    preserveOriginalQuality.value = true
    sessionMaxOutputDimension.value = Number.MAX_SAFE_INTEGER
    if (oversizedReason.value === 'pixels') {
        oversizedReason.value = null
        loadState.value = 'ready'
        scheduleDraw()
        return
    }

    const session = sessionGeneration
    oversizedReason.value = null
    loadState.value = 'loading'
    void loadSource(pendingSource, session, true)
}

async function decodeImage(source: Blob): Promise<ImageBitmap> {
    try {
        return await createImageBitmap(source, { imageOrientation: 'from-image' })
    } catch {
        return await createImageBitmap(source)
    }
}

function canvasToBlob(
    canvas: HTMLCanvasElement,
    type: string,
    quality?: number,
): Promise<Blob | null> {
    return new Promise((resolve) => canvas.toBlob(resolve, type, quality))
}

async function confirm() {
    const bitmap = image.value
    if (!bitmap || exporting.value || !canConfirm.value) return

    if (sessionMode.value === 'crop') {
        const rect = currentSourceRect.value
        pendingResult = {
            sourceWidth: imageSize.value.width,
            sourceHeight: imageSize.value.height,
            crop: {
                left: rect.left,
                top: rect.top,
                width: rect.width,
                height: rect.height,
            },
            normalizedCrop: {
                left: rect.left / imageSize.value.width,
                top: rect.top / imageSize.value.height,
                width: rect.width / imageSize.value.width,
                height: rect.height / imageSize.value.height,
            },
            aspectRatio: selectedAspectRatio.value,
            outputWidth: outputSize.value.width,
            outputHeight: outputSize.value.height,
            oversizedSource: acceptedOversizedSource.value,
            preserveOriginalQuality: preserveOriginalQuality.value,
        }
        await nextTick()
        await modal.value?.hide()
        return
    }

    exporting.value = true
    exportError.value = ''

    try {
        const rect = currentSourceRect.value
        const output = outputSize.value
        const canvas = document.createElement('canvas')
        canvas.width = output.width
        canvas.height = output.height

        const context = canvas.getContext('2d')
        if (!context) throw new Error('The webview could not create a drawing surface')

        context.imageSmoothingQuality = 'high'
        context.drawImage(
            bitmap,
            rect.left,
            rect.top,
            rect.width,
            rect.height,
            0,
            0,
            output.width,
            output.height,
        )

        let blob = await canvasToBlob(
            canvas,
            props.outputMimeType,
            props.outputMimeType === 'image/webp' ? props.outputQuality : undefined,
        )
        if (!blob) {
            blob = await canvasToBlob(canvas, 'image/png')
        }
        if (!blob) throw new Error('The webview could not encode the cropped image')

        if (blob.type !== 'image/png' && blob.type !== 'image/webp') {
            blob = await canvasToBlob(canvas, 'image/png')
        }
        if (!blob || (blob.type !== 'image/png' && blob.type !== 'image/webp')) {
            throw new Error('The webview could not encode the cropped image')
        }

        const mimeType = blob.type === 'image/webp' ? 'image/webp' : 'image/png'
        pendingResult = {
            blob,
            bytes: new Uint8Array(await blob.arrayBuffer()),
            width: output.width,
            height: output.height,
            mimeType,
            extension: mimeType === 'image/webp' ? 'webp' : 'png',
            preserveOriginalQuality: preserveOriginalQuality.value,
        }

        exporting.value = false
        await nextTick()
        await modal.value?.hide()
    } catch (error) {
        console.error('Failed to export a cropped image', error)
        exportError.value = formatMessage(messages.exportError)
    } finally {
        exporting.value = false
    }
}

function cancel() {
    if (exporting.value) return
    void modal.value?.hide()
}

const exposed: ImageCropModalExposed = {
    show,
    showCrop,
    hide: cancel,
}

defineExpose(exposed)

onBeforeUnmount(() => {
    sessionGeneration += 1
    resizeObserver?.disconnect()
    resizeObserver = null
    if (drawRequest !== null) {
        cancelAnimationFrame(drawRequest)
        drawRequest = null
    }
    if (animatedPreviewUrl.value) {
        URL.revokeObjectURL(animatedPreviewUrl.value)
        animatedPreviewUrl.value = null
    }
    image.value?.close()
    image.value = null
    pendingResult = null
    const resolve = pendingResolve
    pendingResolve = null
    resolve?.(null)
})
</script>

<style scoped>
.crop-checkerboard {
    background-color: var(--surface-2);
    background-image:
        linear-gradient(45deg, var(--surface-4) 25%, transparent 25%),
        linear-gradient(-45deg, var(--surface-4) 25%, transparent 25%),
        linear-gradient(45deg, transparent 75%, var(--surface-4) 75%),
        linear-gradient(-45deg, transparent 75%, var(--surface-4) 75%);
    background-position:
        0 0,
        0 6px,
        6px -6px,
        -6px 0;
    background-size: 12px 12px;
}

.crop-loading-scrim {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-80),
        transparent
    );
}

.crop-error-scrim {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-90),
        transparent
    );
}

.crop-oversized-scrim {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-95),
        transparent
    );
}
</style>
