<script setup lang="ts">
import { EditIcon, MaximizeIcon, ZoomInIcon, ZoomOutIcon } from '@modrinth/assets'
import { Button, defineMessages, useFormatBytes, useVIntl } from '@modrinth/ui'
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'

import { canEditImage } from './image-encoding'
import { clampPreviewScale, fitPreviewScale } from './image-preview-geometry'
import type { MediaEditLocation, MediaEditSave } from './media-edit'
import StudioImageEditor from './StudioImageEditor.vue'
import { studioMediaMimeType } from './useStudioMediaPreview'

const props = defineProps<{
    kind: 'image' | 'video'
    url: string
    name: string
    size: number
    content: Uint8Array | null
    location: MediaEditLocation | null
    loading: boolean
    failed: boolean
}>()

const emit = defineEmits<{
    openInSystem: []
    save: [payload: MediaEditSave]
    editFailed: [error: unknown]
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
    edit: {
        id: 'instance.files.studio.preview-edit',
        defaultMessage: 'Edit',
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
const editing = ref(false)
// The file the open editor belongs to. Encoding takes long enough for the active
// tab or the instance to change, so the destination is captured on open instead
// of being read again when the bytes are ready.
const editLocation = ref<MediaEditLocation | null>(null)
// The bytes the editor drew over, kept so the page can tell whether the file was
// replaced on disk while the edit was in progress.
const editOriginal = shallowRef<Uint8Array | null>(null)
// Whether the canvas holds changes the file does not have yet.
const editDirty = ref(false)

let stageObserver: ResizeObserver | null = null
let stopPointerDrag: (() => void) | null = null

const unavailable = computed(() => props.failed || renderFailed.value)
const interactive = computed(() => props.kind === 'image' && !unavailable.value && !props.loading)
const mimeType = computed(() => studioMediaMimeType(props.name))
// The editor exports a single frame and the result is written back over the
// original file, so only the still images the webview can encode again may be
// opened in it.
const canEdit = computed(() => interactive.value && canEditImage(mimeType.value, props.content))

const fitScale = computed(() => fitPreviewScale(natural.value, viewport.value))
const displayScale = computed(() => (fitting.value ? fitScale.value : zoom.value))
const imageStyle = computed(() => ({
    width: `${Math.max(1, Math.round(natural.value.width * displayScale.value))}px`,
    height: `${Math.max(1, Math.round(natural.value.height * displayScale.value))}px`,
}))

const formatByteSize = useFormatBytes()

/**
 * The stage's padding is not part of the room the image may use. Measuring the
 * client box alone asks the fit scale for a larger image than the content area
 * holds, so a fitted image overflows its own stage and shows scrollbars.
 */
function measureStage() {
    const element = stage.value
    if (!element) return
    const style = getComputedStyle(element)
    viewport.value = {
        width:
            element.clientWidth -
            Number.parseFloat(style.paddingLeft) -
            Number.parseFloat(style.paddingRight),
        height:
            element.clientHeight -
            Number.parseFloat(style.paddingTop) -
            Number.parseFloat(style.paddingBottom),
    }
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
    // Chromium reports a trackpad pinch as ctrl+wheel, so this covers pinch on
    // every platform and the key users reach for on Windows and macOS.
    if (!interactive.value || editing.value || !(event.ctrlKey || event.metaKey)) return
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
    if (editing.value || !interactive.value || event.button !== 0 || !element) return
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

function startEditing() {
    if (!props.location || !props.content) return
    editLocation.value = props.location
    editOriginal.value = props.content
    editing.value = true
}

function stopEditing() {
    editing.value = false
    editDirty.value = false
    editLocation.value = null
    editOriginal.value = null
}

function onSave(bytes: Uint8Array) {
    const location = editLocation.value
    const original = editOriginal.value
    if (!location || !original) return
    // The editor stays open until the preview reloads from the file that was
    // written: closing here would throw the canvas away, and a write that fails
    // leaves the file untouched with no way to retry.
    emit('save', { location, original, bytes })
}

defineExpose({
    /** Whether the editor is on screen, so a reload can wait for it to close. */
    isEditing: computed(() => editing.value),
    /** Whether closing the editor would throw away changes. */
    hasUnsavedEdits: computed(() => editing.value && editDirty.value),
    /** Closes the editor and drops its canvas. */
    discardEdits: stopEditing,
})

watch(
    () => props.url,
    () => {
        renderFailed.value = false
        natural.value = { width: 0, height: 0 }
        zoom.value = 1
        fitting.value = true
        stopEditing()
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
            v-if="interactive && !editing"
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
            <Button
                v-if="canEdit"
                type="quiet"
                size="2xs"
                :aria-label="formatMessage(messages.edit)"
                @click="startEditing"
                ><EditIcon class="size-4" />{{ formatMessage(messages.edit) }}</Button
            >
            <span class="ml-auto shrink-0 tabular-nums">
                {{ natural.width }} × {{ natural.height }} · {{ formatByteSize(size) }}
            </span>
        </div>
        <StudioImageEditor
            v-if="editing && content"
            class="min-h-0 flex-1"
            :content="content"
            :mime-type="mimeType"
            :name="name"
            @save="onSave"
            @cancel="stopEditing"
            @dirty="editDirty = $event"
            @error="emit('editFailed', $event)"
        />
        <div
            v-else
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
    /* The grid has to stay visible on either theme, so its tone follows the text
       palette instead of a fixed grey. */
    --checker-tone: color-mix(in srgb, var(--color-text-tertiary) 24%, transparent);
    background-image:
        linear-gradient(45deg, var(--checker-tone) 25%, transparent 25%),
        linear-gradient(-45deg, var(--checker-tone) 25%, transparent 25%),
        linear-gradient(45deg, transparent 75%, var(--checker-tone) 75%),
        linear-gradient(-45deg, transparent 75%, var(--checker-tone) 75%);
    background-position:
        0 0,
        0 8px,
        8px -8px,
        -8px 0;
    background-size: 16px 16px;
}
</style>
