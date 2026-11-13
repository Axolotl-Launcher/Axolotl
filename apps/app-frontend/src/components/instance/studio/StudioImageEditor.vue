<script setup lang="ts">
import {
    CircleIcon,
    CropIcon,
    EraserIcon,
    HighlighterIcon,
    MousePointer2Icon,
    MoveUpRightIcon,
    PencilIcon,
    RedoIcon,
    SaveIcon,
    SquareIcon,
    TrashIcon,
    TypeIcon,
    UndoIcon,
    XIcon,
    ZoomInIcon,
    ZoomOutIcon,
} from '@modrinth/assets'
import {
    Button,
    Chips,
    commonMessages,
    defineMessages,
    IconButton,
    imageViewerEditorMessages as messages,
    type ScreenshotEditorTool,
    type ScreenshotEraserMode,
    useImageEditor,
    useVIntl,
} from '@modrinth/ui'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { reencodeImageBytes } from './image-encoding'

const props = defineProps<{
    content: Uint8Array
    mimeType: string
    name: string
}>()

const emit = defineEmits<{
    cancel: []
    save: [bytes: Uint8Array]
    error: [error: unknown]
    dirty: [dirty: boolean]
}>()

const studioMessages = defineMessages({
    save: { id: 'instance.files.studio.preview-save', defaultMessage: 'Save' },
    tools: {
        id: 'instance.files.studio.preview-tools',
        defaultMessage: 'Image editing tools',
    },
})

const { formatMessage } = useVIntl()
const editor = useImageEditor()
const {
    loading,
    tool,
    color,
    strokeWidth,
    fontSize,
    eraserMode,
    zoom,
    isFit,
    originalWidth,
    originalHeight,
    canUndo,
    canRedo,
    canDelete,
    canZoomOut,
    canZoomIn,
    hasColorProperty,
    propertyValueKind,
    showEraserMode,
    showCropControls,
    cropWidth,
    cropHeight,
    canResetCrop,
    initialize,
    dispose,
    setTool,
    updateColor,
    updateStrokeWidth,
    updateFontSize,
    beginPropertyEdit,
    commitPropertyEdit,
    deleteSelection,
    resetCrop,
    undo,
    redo,
    setInteractionEnabled,
    fitToViewport,
    setZoom,
    setFit,
    exportPng,
    handleKeyboardShortcut,
    isTextEditing,
} = editor

/**
 * The screenshot viewer offers the whole annotation set; Studio edits a file that
 * stays on disk, so the sharing tools are left out. The censor brush exists to
 * hide player names and addresses before a screenshot is posted publicly, which
 * has no meaning for a texture or an icon.
 */
const toolGroups = [
    [
        { id: 'select', message: messages.select, icon: MousePointer2Icon, shortcut: 'V' },
        { id: 'crop', message: messages.crop, icon: CropIcon, shortcut: 'K' },
    ],
    [
        { id: 'pen', message: messages.pen, icon: PencilIcon, shortcut: 'P' },
        { id: 'highlight', message: messages.highlight, icon: HighlighterIcon, shortcut: 'H' },
        { id: 'eraser', message: messages.eraser, icon: EraserIcon, shortcut: 'E' },
    ],
    [
        { id: 'arrow', message: messages.arrow, icon: MoveUpRightIcon, shortcut: 'A' },
        { id: 'rectangle', message: messages.rectangle, icon: SquareIcon, shortcut: 'R' },
        { id: 'ellipse', message: messages.ellipse, icon: CircleIcon, shortcut: 'O' },
    ],
    [{ id: 'text', message: messages.text, icon: TypeIcon, shortcut: 'T' }],
] satisfies Array<
    Array<{
        id: ScreenshotEditorTool
        message: (typeof messages)[keyof typeof messages]
        icon: typeof MousePointer2Icon
        shortcut: string
    }>
>

const eraserModes: ScreenshotEraserMode[] = ['element', 'area']

// The history starts on a snapshot of the untouched image, so anything that can
// be undone is a change that has not been written to the file yet.
watch(canUndo, (dirty) => emit('dirty', dirty), { immediate: true })

const canvasElement = ref<HTMLCanvasElement>()
const viewport = ref<HTMLElement>()
const exporting = ref(false)
const busy = computed(() => exporting.value || loading.value)
const zoomPercent = computed(() => Math.round(zoom.value * 100))
const propertyValue = computed(() =>
    propertyValueKind.value === 'size' ? fontSize.value : strokeWidth.value,
)
const propertyMin = computed(() => (propertyValueKind.value === 'size' ? 12 : 1))
const propertyMax = computed(() => (propertyValueKind.value === 'size' ? 120 : 40))
const propertyProgress = computed(
    () =>
        ((propertyValue.value - propertyMin.value) / (propertyMax.value - propertyMin.value)) * 100,
)
const hasPropertyControls = computed(
    () =>
        showCropControls.value ||
        showEraserMode.value ||
        hasColorProperty.value ||
        Boolean(propertyValueKind.value),
)

let resizeObserver: ResizeObserver | undefined
let unmounted = false

function tooltip(option: (typeof toolGroups)[number][number]) {
    return `${formatMessage(option.message)} (${option.shortcut})`
}

function formatEraserMode(value: ScreenshotEraserMode) {
    return formatMessage(value === 'element' ? messages.element : messages.area)
}

function updatePropertyValue(nextValue: number) {
    if (!Number.isFinite(nextValue)) return
    const normalized = Math.min(
        propertyMax.value,
        Math.max(propertyMin.value, Math.round(nextValue)),
    )
    if (propertyValueKind.value === 'size') updateFontSize(normalized)
    else updateStrokeWidth(normalized)
}

function handlePropertyInput(event: Event) {
    updatePropertyValue(Number((event.target as HTMLInputElement).value))
}

function handleColorInput(event: Event) {
    updateColor((event.target as HTMLInputElement).value)
}

/**
 * The viewport's padding is not part of the room the image may use; measuring the
 * client box alone would ask the engine to fit an image wider than the space it
 * has and produce scrollbars on a freshly opened file.
 */
function measureViewport() {
    const element = viewport.value
    if (!element) return undefined
    const style = getComputedStyle(element)
    return {
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

function fitEditorToViewport() {
    const size = measureViewport()
    if (!size) return
    fitToViewport(size.width, size.height)
}

function observeViewport() {
    resizeObserver?.disconnect()
    if (!viewport.value) return
    resizeObserver = new ResizeObserver(fitEditorToViewport)
    resizeObserver.observe(viewport.value)
    fitEditorToViewport()
}

function onWheel(event: WheelEvent) {
    // Chromium reports a trackpad pinch as ctrl+wheel; a plain wheel keeps
    // scrolling the stage, which is how the preview behaves.
    if (!(event.ctrlKey || event.metaKey) || busy.value) return
    event.preventDefault()
    setZoom(zoom.value * (event.deltaY < 0 ? 1.1 : 1 / 1.1))
}

async function save() {
    if (busy.value) return
    exporting.value = true
    try {
        emit('save', await reencodeImageBytes(await exportPng(), props.mimeType))
    } catch (error) {
        emit('error', error)
    } finally {
        exporting.value = false
    }
}

function cancel() {
    if (busy.value) return
    emit('cancel')
}

/**
 * The property controls below the canvas are ordinary inputs. The shortcut
 * listener sits on the document, so without this a key pressed while one of them
 * has focus would change the active tool instead of reaching the control.
 */
const FOCUSABLE_CONTROL_TAGS = new Set(['INPUT', 'TEXTAREA', 'SELECT'])

function isControlFocused(event: KeyboardEvent) {
    const target = event.target
    if (!(target instanceof HTMLElement)) return false
    return target.isContentEditable || FOCUSABLE_CONTROL_TAGS.has(target.tagName)
}

function onKeydown(event: KeyboardEvent) {
    if (document.querySelector('.modal-root')) return
    // Saving stays available from anywhere in the editor, including a focused
    // control, because the file is what the whole surface is about.
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
        event.preventDefault()
        void save()
        return
    }
    if (isControlFocused(event)) return
    if (isTextEditing()) return
    // The engine binds `C` to the censor tool, which Studio does not offer; the
    // shortcut would otherwise select a tool with no button and no way back.
    if (!event.metaKey && !event.ctrlKey && !event.altKey && event.key.toLowerCase() === 'c') return
    if (handleKeyboardShortcut(event)) return
    if (event.key === 'Escape') {
        event.preventDefault()
        cancel()
    }
}

onMounted(async () => {
    document.addEventListener('keydown', onKeydown)
    await nextTick()
    const element = canvasElement.value
    if (!element) return
    try {
        await initialize(
            element,
            { source: new Blob([props.content as BlobPart], { type: props.mimeType }) },
            measureViewport(),
            // A texture's cut-out pixels have to stay cut out: an opaque canvas
            // would bake the workspace colour into them on export, and smoothing
            // would blur the pixel art the editor exists to touch up.
            { backgroundColor: '', imageSmoothing: false },
        )
        // Closing the editor while the fabric import and the image decode are
        // still running leaves this initialiser behind: the canvas and the
        // object URL it just created would outlive the component for good.
        if (unmounted) {
            void dispose()
            return
        }
        setInteractionEnabled(true)
        observeViewport()
    } catch (error) {
        if (!unmounted) emit('error', error)
    }
})

onBeforeUnmount(() => {
    unmounted = true
    document.removeEventListener('keydown', onKeydown)
    resizeObserver?.disconnect()
    void dispose()
})
</script>

<template>
    <div class="flex size-full min-h-0 flex-col bg-surface-1">
        <div
            class="flex h-10 shrink-0 items-center gap-1 overflow-x-auto border-0 border-b border-solid border-surface-4 px-2"
            role="toolbar"
            :aria-label="formatMessage(studioMessages.tools)"
        >
            <template v-for="(group, groupIndex) in toolGroups" :key="groupIndex">
                <span v-if="groupIndex > 0" class="mx-0.5 h-4 w-px shrink-0 bg-surface-4" />
                <IconButton
                    v-for="option in group"
                    :key="option.id"
                    v-tooltip="tooltip(option)"
                    :label="formatMessage(option.message)"
                    type="quiet"
                    size="2xs"
                    :color="tool === option.id ? 'brand' : undefined"
                    :class="{
                        '!bg-brand-highlight shadow-[inset_0_0_0_1px_var(--color-brand)]':
                            tool === option.id,
                    }"
                    :aria-pressed="tool === option.id"
                    @click="setTool(option.id)"
                >
                    <component :is="option.icon" class="size-4" />
                </IconButton>
            </template>
            <span class="ml-auto shrink-0 pl-2 tabular-nums text-[var(--color-text-tertiary)]">
                {{ originalWidth }} × {{ originalHeight }}
            </span>
        </div>

        <div
            ref="viewport"
            class="relative min-h-0 flex-1 overflow-auto p-4"
            :class="busy ? 'pointer-events-none opacity-60' : ''"
            @wheel="onWheel"
        >
            <div class="editor-transparency m-auto w-fit shrink-0">
                <canvas ref="canvasElement" :aria-label="name" role="img" />
            </div>
        </div>

        <div
            class="flex shrink-0 flex-wrap items-center gap-x-2 gap-y-1.5 border-0 border-t border-solid border-surface-4 px-2 py-1.5 text-xs text-[var(--color-text-tertiary)]"
        >
            <template v-if="showCropControls">
                <span class="shrink-0 tabular-nums">
                    {{
                        formatMessage(messages.cropDimensions, {
                            width: cropWidth,
                            height: cropHeight,
                        })
                    }}
                </span>
                <Button type="quiet" size="2xs" :disabled="!canResetCrop" @click="resetCrop">
                    {{ formatMessage(messages.resetCrop) }}
                </Button>
                <span class="h-4 w-px shrink-0 bg-surface-4" />
            </template>
            <Chips
                v-if="showEraserMode"
                v-model="eraserMode"
                :items="eraserModes"
                :format-label="formatEraserMode"
                :aria-label="formatMessage(messages.eraserMode)"
                size="small"
                hide-checkmark-icon
            />
            <input
                v-if="hasColorProperty"
                type="color"
                :value="color"
                :aria-label="formatMessage(messages.colour)"
                class="size-7 shrink-0 cursor-pointer rounded-md border border-solid border-surface-4 bg-surface-3 p-0.5"
                @focus="beginPropertyEdit"
                @pointerdown="beginPropertyEdit"
                @input="handleColorInput"
                @change="commitPropertyEdit"
            />
            <div
                v-if="propertyValueKind"
                class="flex shrink-0 items-center gap-2"
                @focusin="beginPropertyEdit"
                @pointerdown.capture="beginPropertyEdit"
                @change.capture="commitPropertyEdit"
                @focusout="commitPropertyEdit"
            >
                <span class="shrink-0">
                    {{
                        formatMessage(propertyValueKind === 'size' ? messages.size : messages.width)
                    }}
                </span>
                <input
                    type="range"
                    :value="propertyValue"
                    :min="propertyMin"
                    :max="propertyMax"
                    step="1"
                    :aria-label="
                        formatMessage(propertyValueKind === 'size' ? messages.size : messages.width)
                    "
                    class="editor-property-range h-1 w-24 shrink-0"
                    :style="{ '--property-progress': `${propertyProgress}%` }"
                    @input="handlePropertyInput"
                />
                <input
                    type="number"
                    :value="propertyValue"
                    :min="propertyMin"
                    :max="propertyMax"
                    step="1"
                    :aria-label="
                        formatMessage(propertyValueKind === 'size' ? messages.size : messages.width)
                    "
                    class="h-6 w-11 shrink-0 rounded-md border border-solid border-surface-4 bg-surface-3 px-1 text-center tabular-nums text-[var(--color-text-primary)] outline-none focus:ring-2 focus:ring-brand-shadow"
                    @input="handlePropertyInput"
                />
            </div>

            <span v-if="hasPropertyControls" class="h-4 w-px shrink-0 bg-surface-4" />

            <IconButton
                v-if="canDelete"
                v-tooltip="formatMessage(messages.deleteSelection)"
                :label="formatMessage(messages.deleteSelection)"
                type="quiet"
                size="2xs"
                @click="deleteSelection"
            >
                <TrashIcon class="size-4" />
            </IconButton>
            <IconButton
                v-tooltip="formatMessage(messages.undo)"
                :label="formatMessage(messages.undo)"
                type="quiet"
                size="2xs"
                :disabled="!canUndo"
                @click="undo"
            >
                <UndoIcon class="size-4" />
            </IconButton>
            <IconButton
                v-tooltip="formatMessage(messages.redo)"
                :label="formatMessage(messages.redo)"
                type="quiet"
                size="2xs"
                :disabled="!canRedo"
                @click="redo"
            >
                <RedoIcon class="size-4" />
            </IconButton>

            <span class="h-4 w-px shrink-0 bg-surface-4" />

            <IconButton
                v-tooltip="formatMessage(messages.zoomOut)"
                :label="formatMessage(messages.zoomOut)"
                type="quiet"
                size="2xs"
                :disabled="!canZoomOut"
                @click="setZoom(zoom - 0.1)"
            >
                <ZoomOutIcon class="size-4" />
            </IconButton>
            <Button
                v-tooltip="formatMessage(messages.fitToWorkspace)"
                type="quiet"
                size="2xs"
                class="min-w-12 tabular-nums"
                @click="setFit"
            >
                {{ isFit ? formatMessage(messages.fit) : `${zoomPercent}%` }}
            </Button>
            <IconButton
                v-tooltip="formatMessage(messages.zoomIn)"
                :label="formatMessage(messages.zoomIn)"
                type="quiet"
                size="2xs"
                :disabled="!canZoomIn"
                @click="setZoom(zoom + 0.1)"
            >
                <ZoomInIcon class="size-4" />
            </IconButton>

            <div class="ml-auto flex shrink-0 items-center gap-1.5">
                <Button type="quiet" size="2xs" :disabled="busy" @click="cancel">
                    <XIcon class="size-4" aria-hidden="true" />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="brand" size="2xs" :disabled="busy" @click="save">
                    <SaveIcon class="size-4" aria-hidden="true" />
                    {{ formatMessage(studioMessages.save) }}
                </Button>
            </div>
        </div>
    </div>
</template>

<style scoped>
/* Transparency needs a backdrop, or a texture's cut-out pixels read as black
   against the pane. Utilities cannot express a two-tone tile pattern. */
.editor-transparency {
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

.editor-property-range {
    padding: 0;
    margin: 0;
    cursor: pointer;
    background: transparent;
    outline: none;
    appearance: none;

    &::-webkit-slider-runnable-track {
        height: 0.25rem;
        border-radius: 999px;
        background: linear-gradient(
            to right,
            var(--color-brand) 0%,
            var(--color-brand) var(--property-progress),
            var(--surface-4) var(--property-progress),
            var(--surface-4) 100%
        );
    }

    &::-webkit-slider-thumb {
        width: 0.75rem;
        height: 0.75rem;
        margin-top: -0.25rem;
        border: 2px solid var(--surface-1);
        border-radius: 999px;
        background: var(--color-brand);
        appearance: none;
    }

    &::-moz-range-track {
        height: 0.25rem;
        border-radius: 999px;
        background: var(--surface-4);
    }

    &::-moz-range-progress {
        height: 0.25rem;
        border-radius: 999px;
        background: var(--color-brand);
    }

    &::-moz-range-thumb {
        width: 0.75rem;
        height: 0.75rem;
        border: 2px solid var(--surface-1);
        border-radius: 999px;
        background: var(--color-brand);
    }
}
</style>
