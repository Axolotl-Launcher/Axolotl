<script setup lang="ts">
import ArrowLeftIcon from '@modrinth/assets/icons/arrow-left.svg?component'
import CircleIcon from '@modrinth/assets/icons/circle.svg?component'
import CropIcon from '@modrinth/assets/icons/crop.svg?component'
import EditIcon from '@modrinth/assets/icons/edit.svg?component'
import EraserIcon from '@modrinth/assets/icons/eraser.svg?component'
import HighlighterIcon from '@modrinth/assets/icons/highlighter.svg?component'
import MousePointer2Icon from '@modrinth/assets/icons/mouse-pointer-2.svg?component'
import MoveUpRightIcon from '@modrinth/assets/icons/move-up-right.svg?component'
import PencilIcon from '@modrinth/assets/icons/pencil.svg?component'
import RedoIcon from '@modrinth/assets/icons/redo.svg?component'
import ArrowRightIcon from '@modrinth/assets/icons/right-arrow.svg?component'
import RotateClockwiseIcon from '@modrinth/assets/icons/rotate-clockwise.svg?component'
import SquareIcon from '@modrinth/assets/icons/square.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import TypeIcon from '@modrinth/assets/icons/type.svg?component'
import UndoIcon from '@modrinth/assets/icons/undo.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import ZoomInIcon from '@modrinth/assets/icons/zoom-in.svg?component'
import ZoomOutIcon from '@modrinth/assets/icons/zoom-out.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import IconButton from '@modrinth/ui/src/components/base/buttons/IconButton.vue'
import { imageViewerEditorMessages as messages } from '@modrinth/ui/src/components/image-viewer-editor/image-viewer-editor-messages.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'

import { injectPreviewWindow } from '../../lib/window'

export type ViewerItem = {
    id: string
    src: string
    title: string
    alt: string
    description: string
    /** Natural size of the capture, as the app's metadata line reports it. */
    width: number
    height: number
    size?: number
}

export type ViewerSavePayload = {
    item: ViewerItem
    pngBytes: Uint8Array
    mode: 'create_copy' | 'replace_edit'
}

type EditorTool =
    'select' | 'crop' | 'pen' | 'highlight' | 'eraser' | 'arrow' | 'rectangle' | 'ellipse' | 'text'

type AnnotationKind = 'pen' | 'highlight' | 'arrow' | 'rectangle' | 'ellipse' | 'text'

type Annotation = {
    id: string
    kind: AnnotationKind
    color: string
    width: number
    /** Canvas-space points; shapes use the first and last point. */
    points: { x: number; y: number }[]
    text?: string
}

type EditorState = {
    annotations: Annotation[]
    crop: { left: number; top: number; width: number; height: number }
}

const props = withDefaults(
    defineProps<{
        items: ViewerItem[]
        saving?: boolean
    }>(),
    { saving: false },
)

const emit = defineEmits<{
    save: [payload: ViewerSavePayload]
    saved: [item: ViewerItem]
}>()

const { layer } = injectPreviewWindow()
const { formatMessage } = useVIntl()

const activeIndex = ref(-1)
const mode = ref<'view' | 'edit'>('view')
const viewerEl = ref<HTMLElement>()
const canvas = ref<HTMLCanvasElement>()
const zoom = ref(1)
const tool = ref<EditorTool>('select')
const color = ref('#ef4444')
const eraserMode = ref<'element' | 'area'>('element')
const strokeWidth = ref(6)
const history = ref<EditorState[]>([])
const historyIndex = ref(0)
const dirty = ref(false)
const base = shallowRef<ImageBitmap | null>(null)
/** Objects drawn on top of the capture, in paint order (the app's fabric objects). */
const annotations = ref<Annotation[]>([])
/** The part of the source the canvas shows, as `sourceRect` in the app's editor. */
const crop = ref({ left: 0, top: 0, width: 0, height: 0 })
const dims = ref({ width: 0, height: 0 })
const pointer = ref<{ x: number; y: number; visible: boolean }>({ x: 0, y: 0, visible: false })
const cropRect = ref<{ left: number; top: number; width: number; height: number } | null>(null)
const drag = ref<{
    startX: number
    startY: number
    lastX: number
    lastY: number
} | null>(null)
const textEditing = ref<{ x: number; y: number; value: string } | null>(null)
const textInput = ref<HTMLTextAreaElement>()
const selectedId = ref<string | null>(null)

const activeItem = computed<ViewerItem | undefined>(() => props.items[activeIndex.value])
const canvasReady = computed(() => dims.value.width > 0)

const toolGroups = [
    [
        { id: 'select' as const, message: messages.select, icon: MousePointer2Icon, shortcut: 'V' },
        { id: 'crop' as const, message: messages.crop, icon: CropIcon, shortcut: 'K' },
    ],
    [
        { id: 'pen' as const, message: messages.pen, icon: PencilIcon, shortcut: 'P' },
        {
            id: 'highlight' as const,
            message: messages.highlight,
            icon: HighlighterIcon,
            shortcut: 'H',
        },
        { id: 'eraser' as const, message: messages.eraser, icon: EraserIcon, shortcut: 'E' },
    ],
    [
        { id: 'arrow' as const, message: messages.arrow, icon: MoveUpRightIcon, shortcut: 'A' },
        { id: 'rectangle' as const, message: messages.rectangle, icon: SquareIcon, shortcut: 'R' },
        { id: 'ellipse' as const, message: messages.ellipse, icon: CircleIcon, shortcut: 'O' },
    ],
    [{ id: 'text' as const, message: messages.text, icon: TypeIcon, shortcut: 'T' }],
]

const zoomPercent = computed(() => Math.round(zoom.value * 100))
const canUndo = computed(() => historyIndex.value > 0)
const canRedo = computed(() => historyIndex.value < history.value.length - 1)
const hasCrop = computed(() => Boolean(cropRect.value))

function formatBytes(bytes?: number) {
    if (!bytes) return `${Math.round((dims.value.width * dims.value.height * 4) / 3 / 1024)} KB`
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
    return `${Math.round(bytes / 1024)} KB`
}

const metadata = computed(() => {
    const item = activeItem.value
    if (!item) return ''
    return formatMessage(messages.metadata, {
        size: formatBytes(item.size),
        width: dims.value.width || item.width,
        height: dims.value.height || item.height,
    })
})

function show(index: number) {
    if (index < 0 || index >= props.items.length) return
    activeIndex.value = index
    mode.value = 'view'
    zoom.value = 1
    void nextTick(loadCanvas)
}

async function edit(index: number) {
    show(index)
    await nextTick()
    beginEditing()
}

function navigate(offset: number) {
    if (mode.value !== 'view' || props.items.length < 2) return
    const index = (activeIndex.value + offset + props.items.length) % props.items.length
    activeIndex.value = index
    zoom.value = 1
    void nextTick(loadCanvas)
}

function next() {
    navigate(1)
}

function previous() {
    navigate(-1)
}

function hide() {
    if (props.saving) return
    activeIndex.value = -1
    mode.value = 'view'
    cropRect.value = null
    drag.value = null
    textEditing.value = null
    history.value = []
    historyIndex.value = 0
    annotations.value = []
    selectedId.value = null
    dims.value = { width: 0, height: 0 }
    dirty.value = false
    base.value = null
}

let loadGeneration = 0

async function loadCanvas() {
    const item = activeItem.value
    const element = canvas.value
    if (!item || !element) return
    const generation = ++loadGeneration
    try {
        const response = await fetch(item.src)
        const blob = await response.blob()
        const bitmap = await createImageBitmap(blob)
        if (generation !== loadGeneration) {
            bitmap.close()
            return
        }
        base.value = bitmap
        annotations.value = []
        selectedId.value = null
        crop.value = { left: 0, top: 0, width: bitmap.width, height: bitmap.height }
        redraw()
        history.value = [{ annotations: [], crop: { ...crop.value } }]
        historyIndex.value = 0
        dirty.value = false
        cropRect.value = null
    } catch {
        // An unreadable capture keeps the card's still visible behind the
        // canvas, which is the app's own path when the file cannot be read.
    }
}

function pushHistory() {
    history.value = [
        ...history.value.slice(0, historyIndex.value + 1),
        {
            annotations: annotations.value.map((annotation) => ({
                ...annotation,
                points: annotation.points.map((point) => ({ ...point })),
            })),
            crop: { ...crop.value },
        },
    ]
    historyIndex.value = history.value.length - 1
    dirty.value = true
}

function restore(state: EditorState) {
    annotations.value = state.annotations.map((annotation) => ({
        ...annotation,
        points: annotation.points.map((point) => ({ ...point })),
    }))
    crop.value = { ...state.crop }
    redraw()
}

function deleteSelected() {
    if (!selectedId.value) return
    annotations.value = annotations.value.filter((annotation) => annotation.id !== selectedId.value)
    selectedId.value = null
    redraw()
    pushHistory()
}

function undo() {
    if (!canUndo.value) return
    historyIndex.value -= 1
    restore(history.value[historyIndex.value])
}

function redo() {
    if (!canRedo.value) return
    historyIndex.value += 1
    restore(history.value[historyIndex.value])
}

function beginEditing() {
    if (!activeItem.value || props.saving) return
    mode.value = 'edit'
    tool.value = 'select'
    void nextTick(loadCanvas)
}

function cancelEditing() {
    if (props.saving) return
    mode.value = 'view'
    cropRect.value = null
    drag.value = null
    textEditing.value = null
    history.value = []
    historyIndex.value = 0
    annotations.value = []
    selectedId.value = null
    void nextTick(loadCanvas)
}

/** Repaints the capture and every annotation, as the fabric canvas renders. */
function redraw(preview?: Annotation) {
    const element = canvas.value
    const context = element?.getContext('2d')
    if (!element || !context) return
    const source = crop.value
    const width = Math.max(1, Math.round(source.width))
    const height = Math.max(1, Math.round(source.height))
    if (element.width !== width) element.width = width
    if (element.height !== height) element.height = height
    dims.value = { width: element.width, height: element.height }
    if (base.value) {
        context.drawImage(
            base.value,
            source.left,
            source.top,
            source.width,
            source.height,
            0,
            0,
            source.width,
            source.height,
        )
    }
    for (const annotation of annotations.value) drawAnnotation(context, annotation)
    if (preview) drawAnnotation(context, preview)
    if (selectedId.value) drawSelection(context, selectedId.value)
}

function drawAnnotation(context: CanvasRenderingContext2D, annotation: Annotation) {
    const first = annotation.points[0]
    const last = annotation.points[annotation.points.length - 1]
    if (!first || !last) return
    context.strokeStyle = annotation.color
    context.fillStyle = annotation.color
    context.lineWidth = annotation.width
    context.lineCap = 'round'
    context.lineJoin = 'round'
    if (annotation.kind === 'pen' || annotation.kind === 'highlight') {
        context.globalAlpha = annotation.kind === 'highlight' ? 0.35 : 1
        context.beginPath()
        context.moveTo(first.x, first.y)
        for (const point of annotation.points.slice(1)) context.lineTo(point.x, point.y)
        context.stroke()
        context.globalAlpha = 1
        return
    }
    if (annotation.kind === 'text') {
        context.font = `600 ${annotation.width * 4}px sans-serif`
        context.textBaseline = 'top'
        context.fillText(annotation.text ?? '', first.x, first.y)
        return
    }
    if (annotation.kind === 'arrow') {
        const angle = Math.atan2(last.y - first.y, last.x - first.x)
        const head = Math.max(12, annotation.width * 3)
        context.beginPath()
        context.moveTo(first.x, first.y)
        context.lineTo(last.x, last.y)
        context.stroke()
        context.beginPath()
        context.moveTo(last.x, last.y)
        context.lineTo(
            last.x - head * Math.cos(angle - Math.PI / 7),
            last.y - head * Math.sin(angle - Math.PI / 7),
        )
        context.lineTo(
            last.x - head * Math.cos(angle + Math.PI / 7),
            last.y - head * Math.sin(angle + Math.PI / 7),
        )
        context.closePath()
        context.fill()
        return
    }
    if (annotation.kind === 'rectangle') {
        context.strokeRect(first.x, first.y, last.x - first.x, last.y - first.y)
        return
    }
    context.beginPath()
    context.ellipse(
        first.x + (last.x - first.x) / 2,
        first.y + (last.y - first.y) / 2,
        Math.abs(last.x - first.x) / 2,
        Math.abs(last.y - first.y) / 2,
        0,
        0,
        Math.PI * 2,
    )
    context.stroke()
}

function drawSelection(context: CanvasRenderingContext2D, id: string) {
    const annotation = annotations.value.find((candidate) => candidate.id === id)
    if (!annotation) return
    const bounds = annotationBounds(annotation)
    context.save()
    context.strokeStyle = '#3b82f6'
    context.lineWidth = 1.5
    context.strokeRect(bounds.left, bounds.top, bounds.width, bounds.height)
    context.fillStyle = '#ffffff'
    for (const point of annotation.points) {
        context.beginPath()
        context.arc(point.x, point.y, 4, 0, Math.PI * 2)
        context.fill()
        context.stroke()
    }
    context.restore()
}

function annotationBounds(annotation: Annotation) {
    const xs = annotation.points.map((point) => point.x)
    const ys = annotation.points.map((point) => point.y)
    if (annotation.kind === 'text') {
        const size = annotation.width * 4
        return {
            left: Math.min(...xs),
            top: Math.min(...ys),
            width: Math.max(size, (annotation.text?.length ?? 0) * size * 0.55),
            height: size * 1.3,
        }
    }
    return {
        left: Math.min(...xs) - annotation.width / 2,
        top: Math.min(...ys) - annotation.width / 2,
        width: Math.max(...xs) - Math.min(...xs) + annotation.width,
        height: Math.max(...ys) - Math.min(...ys) + annotation.width,
    }
}

/** Distance from a point to the annotation; the app hit-tests its fabric paths. */
function annotationAt(point: { x: number; y: number }, annotation: Annotation) {
    if (annotation.kind === 'text') {
        const bounds = annotationBounds(annotation)
        return (
            point.x >= bounds.left &&
            point.x <= bounds.left + bounds.width &&
            point.y >= bounds.top &&
            point.y <= bounds.top + bounds.height
        )
    }
    const reach = Math.max(annotation.width / 2 + 6, 10)
    const points = annotation.points
    for (let index = 1; index < points.length; index++) {
        const start = points[index - 1]
        const end = points[index]
        const length = Math.hypot(end.x - start.x, end.y - start.y)
        const t =
            length === 0
                ? 0
                : Math.max(
                      0,
                      Math.min(
                          1,
                          ((point.x - start.x) * (end.x - start.x) +
                              (point.y - start.y) * (end.y - start.y)) /
                              (length * length),
                      ),
                  )
        const nearest = { x: start.x + t * (end.x - start.x), y: start.y + t * (end.y - start.y) }
        if (Math.hypot(point.x - nearest.x, point.y - nearest.y) <= reach) return true
    }
    if (annotation.kind === 'rectangle' || annotation.kind === 'ellipse') {
        const bounds = annotationBounds(annotation)
        const inside =
            point.x >= bounds.left &&
            point.x <= bounds.left + bounds.width &&
            point.y >= bounds.top &&
            point.y <= bounds.top + bounds.height
        if (inside && annotation.kind === 'rectangle') return true
        if (inside && annotation.kind === 'ellipse') {
            const rx = bounds.width / 2
            const ry = bounds.height / 2
            const nx = (point.x - (bounds.left + rx)) / (rx || 1)
            const ny = (point.y - (bounds.top + ry)) / (ry || 1)
            if (nx * nx + ny * ny <= 1.15) return true
        }
    }
    return points.length === 1 && Math.hypot(point.x - points[0].x, point.y - points[0].y) <= reach
}

/**
 * The eraser removes annotations above the capture, as the app's fabric
 * objects are erased: `element` lifts the annotation under the pointer, while
 * `area` clears every annotation the stroke crosses.
 */
function eraseAt(point: { x: number; y: number }) {
    const hits = annotations.value.filter((annotation) => annotationAt(point, annotation))
    if (hits.length === 0) return
    const removed = eraserMode.value === 'area' ? hits : [[...hits].reverse()[0]]
    annotations.value = annotations.value.filter(
        (annotation) => !removed.some((hit) => hit.id === annotation.id),
    )
    if (removed.some((hit) => hit.id === selectedId.value)) selectedId.value = null
    redraw()
    dirty.value = true
}

function pointerDown(event: PointerEvent) {
    if (mode.value !== 'edit' || !canvasReady.value) return
    if (textEditing.value) commitText()
    if (tool.value === 'select') {
        const point = canvasPoint(event)
        const hit = [...annotations.value]
            .reverse()
            .find((annotation) => annotationAt(point, annotation))
        selectedId.value = hit?.id ?? null
        redraw()
        return
    }
    event.preventDefault()
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    const point = canvasPoint(event)
    if (tool.value === 'text') {
        textEditing.value = { x: point.x, y: point.y, value: '' }
        void nextTick(() => {
            textInput.value?.focus()
            pointer.value = { ...pointer.value, visible: false }
        })
        return
    }
    drag.value = { startX: point.x, startY: point.y, lastX: point.x, lastY: point.y }
    draftPoints = [{ x: point.x, y: point.y }]
    if (tool.value === 'crop') {
        cropRect.value = { left: point.x, top: point.y, width: 0, height: 0 }
    } else if (tool.value === 'eraser') {
        eraseAt(point)
    }
}

let draftPoints: { x: number; y: number }[] = []

function pointerMove(event: PointerEvent) {
    const element = canvas.value
    if (!element) return
    const bounds = element.getBoundingClientRect()
    pointer.value = {
        x: event.clientX - bounds.left,
        y: event.clientY - bounds.top,
        visible: mode.value === 'edit' && tool.value !== 'select' && tool.value !== 'crop',
    }
    const gesture = drag.value
    if (!gesture) return
    const point = canvasPoint(event)
    if (tool.value === 'crop') {
        gesture.lastX = point.x
        gesture.lastY = point.y
        cropRect.value = {
            left: Math.min(gesture.startX, point.x),
            top: Math.min(gesture.startY, point.y),
            width: Math.abs(point.x - gesture.startX),
            height: Math.abs(point.y - gesture.startY),
        }
        return
    }
    if (tool.value === 'eraser') {
        eraseAt(point)
        return
    }
    gesture.lastX = point.x
    gesture.lastY = point.y
    if (!draftPoints.length) draftPoints = [{ x: gesture.startX, y: gesture.startY }]
    if (tool.value === 'pen' || tool.value === 'highlight') {
        draftPoints = [...draftPoints, { x: point.x, y: point.y }]
        redraw({
            id: 'draft',
            kind: tool.value,
            color: color.value,
            width: tool.value === 'highlight' ? strokeWidth.value * 4 : strokeWidth.value,
            points: draftPoints,
        })
        dirty.value = true
        return
    }
    redraw({
        id: 'draft',
        kind: tool.value as AnnotationKind,
        color: color.value,
        width: strokeWidth.value,
        points: [
            { x: gesture.startX, y: gesture.startY },
            { x: point.x, y: point.y },
        ],
    })
    dirty.value = true
}

function pointerUp() {
    const gesture = drag.value
    if (!gesture) return
    drag.value = null
    if (mode.value !== 'edit') return
    if (tool.value === 'crop') {
        const rect = cropRect.value
        if (rect && (rect.width < 6 || rect.height < 6)) cropRect.value = null
        return
    }
    if (tool.value === 'eraser') return
    if (tool.value === 'select') return
    const last = { x: gesture.lastX, y: gesture.lastY }
    const start = { x: gesture.startX, y: gesture.startY }
    if (Math.hypot(last.x - start.x, last.y - start.y) < 3) {
        draftPoints = []
        redraw()
        return
    }
    const kind = tool.value as AnnotationKind
    annotations.value = [
        ...annotations.value,
        {
            id: `annotation-${Date.now()}`,
            kind,
            color: color.value,
            width: kind === 'highlight' ? strokeWidth.value * 4 : strokeWidth.value,
            points:
                kind === 'pen' || kind === 'highlight'
                    ? [...draftPoints, last]
                    : [start, { x: last.x, y: last.y }],
        },
    ]
    draftPoints = []
    redraw()
    pushHistory()
}

function applyCrop() {
    const rect = cropRect.value
    const element = canvas.value
    if (!rect || !element) return
    const current = crop.value
    const left = current.left + rect.left
    const top = current.top + rect.top
    crop.value = {
        left,
        top,
        width: Math.max(1, Math.min(rect.width, current.left + current.width - left)),
        height: Math.max(1, Math.min(rect.height, current.top + current.height - top)),
    }
    annotations.value = annotations.value
        .map((annotation) => ({
            ...annotation,
            points: annotation.points.map((point) => ({
                x: point.x - rect.left,
                y: point.y - rect.top,
            })),
        }))
        .filter((annotation) =>
            annotation.points.some(
                (point) =>
                    point.x >= -annotation.width &&
                    point.y >= -annotation.width &&
                    point.x <= crop.value.width + annotation.width &&
                    point.y <= crop.value.height + annotation.width,
            ),
        )
    cropRect.value = null
    redraw()
    pushHistory()
}

function resetCrop() {
    cropRect.value = null
}

/** Rotates the capture a quarter turn, as the editor's rotate control does. */
function rotate() {
    const element = canvas.value
    if (!element) return
    const source = document.createElement('canvas')
    source.width = element.width
    source.height = element.height
    source.getContext('2d')?.drawImage(element, 0, 0)
    const width = crop.value.width
    const height = crop.value.height
    crop.value = { left: 0, top: 0, width: height, height: width }
    annotations.value = annotations.value.map((annotation) => ({
        ...annotation,
        points: annotation.points.map((point) => ({
            x: element.height - point.y,
            y: point.x,
        })),
    }))
    redraw()
    pushHistory()
}

function commitText() {
    const editing = textEditing.value
    if (!editing) return
    textEditing.value = null
    if (!editing.value.trim()) return
    annotations.value = [
        ...annotations.value,
        {
            id: `annotation-${Date.now()}`,
            kind: 'text',
            color: color.value,
            width: strokeWidth.value,
            points: [{ x: editing.x, y: editing.y }],
            text: editing.value,
        },
    ]
    redraw()
    pushHistory()
}

function cancelText() {
    textEditing.value = null
}

function zoomIn() {
    zoom.value = Math.min(5, zoom.value * 1.25)
}

function zoomOut() {
    zoom.value = Math.max(1, zoom.value / 1.25)
}

function resetZoom() {
    zoom.value = 1
}

function closeOnBackdrop(event: MouseEvent) {
    if (mode.value === 'view' && event.target === viewerEl.value) hide()
}

async function requestSave(saveMode: 'create_copy' | 'replace_edit') {
    const item = activeItem.value
    const element = canvas.value
    if (!item || !element || props.saving) return
    const blob = await new Promise<Blob | null>((resolve) => element.toBlob(resolve, 'image/png'))
    if (!blob) return
    const bytes = new Uint8Array(await blob.arrayBuffer())
    emit('save', { item, pngBytes: bytes, mode: saveMode })
}

function markSavedAndView(itemId?: string) {
    mode.value = 'view'
    dirty.value = false
    history.value = []
    historyIndex.value = 0
    if (itemId) {
        const index = props.items.findIndex((item) => item.id === itemId)
        if (index >= 0) {
            activeIndex.value = index
            void nextTick(loadCanvas)
        }
    }
    if (activeItem.value) emit('saved', activeItem.value)
}

function handleKeydown(event: KeyboardEvent) {
    if (activeIndex.value < 0) return
    if (event.key === 'Escape') {
        if (textEditing.value) {
            event.preventDefault()
            commitText()
            return
        }
        event.preventDefault()
        if (mode.value === 'edit') cancelEditing()
        else hide()
        return
    }
    if (mode.value !== 'view') return
    if (event.key === 'ArrowLeft') {
        event.preventDefault()
        previous()
    } else if (event.key === 'ArrowRight') {
        event.preventDefault()
        next()
    }
}

watch(
    () => [props.items.length, activeIndex.value] as const,
    ([length, index]) => {
        if (index >= length && length > 0) activeIndex.value = length - 1
    },
)

onMounted(() => document.addEventListener('keydown', handleKeydown))
onBeforeUnmount(() => {
    document.removeEventListener('keydown', handleKeydown)
    loadGeneration += 1
})

defineExpose({ show, edit, hide, next, previous, markSavedAndView })
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <Transition name="image-viewer">
            <div
                v-if="activeItem"
                ref="viewerEl"
                class="fixed inset-0 z-[110] overflow-hidden bg-black/95 text-white"
                role="dialog"
                aria-modal="true"
                :aria-label="activeItem.title || activeItem.alt"
                @click.self="closeOnBackdrop"
            >
                <header
                    v-if="activeItem.title || activeItem.description"
                    class="absolute inset-x-6 top-[calc(var(--top-bar-height,3rem)_+_1.5rem)] z-10 min-w-0"
                    @click.stop
                >
                    <div class="viewer-heading-text w-fit min-w-0 max-w-full">
                        <h2
                            v-if="activeItem.title"
                            class="m-0 max-w-[min(42rem,896px)] truncate text-base font-semibold leading-snug text-[var(--color-text-primary)]"
                        >
                            {{ activeItem.title }}
                        </h2>
                        <p
                            v-if="activeItem.description"
                            class="mb-0 mt-1 max-w-[min(42rem,896px)] truncate text-xs leading-snug text-white"
                        >
                            {{ activeItem.description }}
                        </p>
                        <p
                            class="mb-0 mt-1 max-w-[min(42rem,896px)] truncate text-xs leading-snug text-white"
                        >
                            {{ metadata }}
                        </p>
                    </div>
                </header>

                <div
                    class="editor-viewport absolute inset-0 flex min-h-0 min-w-0 select-none overflow-auto px-6 pb-[5.75rem] pt-[4.75rem]"
                    :class="{
                        'is-view': mode === 'view',
                        'is-view-zoomed': mode === 'view' && zoom > 1,
                    }"
                >
                    <div
                        class="editor-canvas relative z-[2] m-auto shrink-0"
                        :style="{ transform: `scale(${zoom})`, transformOrigin: 'center center' }"
                        @pointerenter="pointer.visible = mode === 'edit' && tool !== 'select'"
                        @pointerleave="pointer.visible = false"
                        @pointermove="pointerMove"
                        @pointerdown="pointerDown"
                        @pointerup="pointerUp"
                        @pointercancel="pointerUp"
                    >
                        <canvas
                            ref="canvas"
                            :aria-label="activeItem.alt"
                            role="img"
                            class="block max-h-[calc(800px_-_11rem)] max-w-full rounded-md shadow-[0_1.5rem_4rem_rgb(0_0_0_/_24%)]"
                            :class="{ 'cursor-crosshair': mode === 'edit' && tool !== 'select' }"
                        />
                        <div
                            v-if="cropRect && canvasReady"
                            class="crop-selection pointer-events-none absolute border-2 border-dashed"
                            :style="{
                                left: `${(cropRect.left / dims.width) * 100}%`,
                                top: `${(cropRect.top / dims.height) * 100}%`,
                                width: `${(cropRect.width / dims.width) * 100}%`,
                                height: `${(cropRect.height / dims.height) * 100}%`,
                            }"
                        />
                        <textarea
                            v-if="textEditing && canvasReady"
                            ref="textInput"
                            v-model="textEditing.value"
                            class="both absolute z-[3] min-h-8 w-56 resize rounded border border-solid border-white/40 bg-black/50 p-1 text-sm text-white outline-none"
                            :style="{
                                left: `${(textEditing.x / dims.width) * 100}%`,
                                top: `${(textEditing.y / dims.height) * 100}%`,
                            }"
                            @blur="commitText"
                            @keydown.enter.exact.prevent="commitText"
                            @keydown.esc.prevent="cancelText"
                        />
                        <div
                            v-if="pointer.visible && tool !== 'text'"
                            class="pointer-events-none absolute z-10 -translate-x-1/2 -translate-y-1/2 rounded-full border border-solid shadow-[0_0_0_1px_rgb(0_0_0_/_80%),inset_0_0_0_1px_rgb(255_255_255_/_35%)]"
                            :style="{
                                left: `${pointer.x}px`,
                                top: `${pointer.y}px`,
                                width: `${strokeWidth * zoom}px`,
                                height: `${strokeWidth * zoom}px`,
                                borderColor: color,
                            }"
                        >
                            <div
                                class="absolute inset-0 rounded-full opacity-[0.15]"
                                :style="{ backgroundColor: color }"
                            />
                        </div>
                        <img
                            v-if="!canvasReady"
                            :src="activeItem.src"
                            :alt="activeItem.alt"
                            class="pointer-events-none absolute inset-0 m-auto max-h-[calc(800px_-_11rem)] max-w-full rounded-md opacity-100"
                        />
                    </div>
                </div>

                <template v-if="mode === 'edit'">
                    <div
                        class="absolute left-6 top-1/2 z-10 flex -translate-y-1/2 flex-col items-center gap-2 rounded-[20px] border border-solid border-white/10 bg-surface-3 px-3 py-2.5 shadow-[0_1rem_3rem_rgb(0_0_0_/_32%)]"
                        role="toolbar"
                        :aria-label="formatMessage(messages.annotationTools)"
                        @click.stop
                    >
                        <template v-for="(group, groupIndex) in toolGroups" :key="group[0]?.id">
                            <div v-if="groupIndex > 0" class="h-px w-6 bg-white/10" />
                            <IconButton
                                v-for="option in group"
                                :key="option.id"
                                v-tooltip="`${formatMessage(option.message)} (${option.shortcut})`"
                                :label="formatMessage(option.message)"
                                type="quiet"
                                :color="tool === option.id ? 'brand' : undefined"
                                :class="{
                                    '!bg-brand-highlight shadow-[inset_0_0_0_1px_var(--color-brand)]':
                                        tool === option.id,
                                }"
                                :aria-pressed="tool === option.id"
                                @click="tool = option.id"
                            >
                                <component :is="option.icon" />
                            </IconButton>
                        </template>
                        <div class="h-px w-6 bg-white/10" />
                        <IconButton
                            v-tooltip="formatMessage(messages.rotateClockwise)"
                            :label="formatMessage(messages.rotateClockwise)"
                            type="quiet"
                            @click="rotate"
                        >
                            <RotateClockwiseIcon />
                        </IconButton>
                    </div>

                    <div
                        class="absolute bottom-6 left-1/2 z-10 flex w-max max-w-[calc(100%_-_3rem)] -translate-x-1/2 items-center gap-2 rounded-[20px] border border-solid border-white/10 bg-surface-3 p-2 shadow-[0_1rem_3rem_rgb(0_0_0_/_32%)]"
                        @click.stop
                    >
                        <div v-if="tool === 'crop' && hasCrop" class="flex items-center gap-2">
                            <span
                                class="shrink-0 text-base font-semibold tabular-nums leading-5 text-white/60"
                            >
                                {{
                                    formatMessage(messages.cropDimensions, {
                                        width: Math.round(cropRect?.width ?? 0),
                                        height: Math.round(cropRect?.height ?? 0),
                                    })
                                }}
                            </span>
                            <Button type="quiet" @click="resetCrop">
                                {{ formatMessage(messages.resetCrop) }}
                            </Button>
                            <Button type="colored" color="brand" @click="applyCrop">
                                {{ formatMessage(messages.saveImage) }}
                            </Button>
                            <div class="h-6 w-px bg-white/10" />
                        </div>
                        <div v-if="tool === 'eraser'" class="flex items-center gap-2">
                            <span class="shrink-0 text-base font-semibold leading-5 text-white/60">
                                {{ formatMessage(messages.eraserMode) }}
                            </span>
                            <div
                                class="flex items-center gap-1"
                                role="group"
                                :aria-label="formatMessage(messages.eraserMode)"
                            >
                                <Button
                                    type="quiet"
                                    :color="eraserMode === 'element' ? 'brand' : undefined"
                                    :class="{ '!bg-brand-highlight': eraserMode === 'element' }"
                                    :aria-pressed="eraserMode === 'element'"
                                    @click="eraserMode = 'element'"
                                >
                                    {{ formatMessage(messages.element) }}
                                </Button>
                                <Button
                                    type="quiet"
                                    :color="eraserMode === 'area' ? 'brand' : undefined"
                                    :class="{ '!bg-brand-highlight': eraserMode === 'area' }"
                                    :aria-pressed="eraserMode === 'area'"
                                    @click="eraserMode = 'area'"
                                >
                                    {{ formatMessage(messages.area) }}
                                </Button>
                            </div>
                            <div class="h-6 w-px bg-white/10" />
                        </div>
                        <div
                            v-if="tool !== 'select' && tool !== 'crop' && tool !== 'eraser'"
                            class="flex min-w-0 items-center gap-2"
                        >
                            <input
                                v-model="color"
                                type="color"
                                :aria-label="formatMessage(messages.colour)"
                                class="size-9 shrink-0 cursor-pointer rounded-xl border border-solid border-surface-5 bg-surface-4 p-1"
                            />
                            <div class="flex w-52 min-w-0 items-center gap-2">
                                <span
                                    class="shrink-0 text-base font-semibold leading-5 text-white/60"
                                >
                                    {{ formatMessage(messages.width) }}
                                </span>
                                <input
                                    v-model.number="strokeWidth"
                                    type="range"
                                    min="1"
                                    max="24"
                                    step="1"
                                    :aria-label="formatMessage(messages.width)"
                                    class="min-w-0 flex-1 accent-brand"
                                />
                                <input
                                    v-model.number="strokeWidth"
                                    type="number"
                                    min="1"
                                    max="24"
                                    step="1"
                                    :aria-label="formatMessage(messages.width)"
                                    class="h-9 w-12 shrink-0 rounded-xl border border-solid border-surface-5 bg-surface-4 px-1 text-center text-base font-semibold tabular-nums leading-5 text-white outline-none focus:ring-4 focus:ring-brand-shadow"
                                />
                            </div>
                            <div class="h-6 w-px bg-white/10" />
                        </div>
                        <div class="flex items-center gap-2">
                            <IconButton
                                v-if="tool === 'select' && selectedId"
                                v-tooltip="formatMessage(messages.deleteSelection)"
                                :label="formatMessage(messages.deleteSelection)"
                                type="quiet"
                                @click="deleteSelected"
                            >
                                <TrashIcon />
                            </IconButton>
                            <IconButton
                                v-tooltip="formatMessage(messages.undo)"
                                :label="formatMessage(messages.undo)"
                                type="quiet"
                                :disabled="!canUndo"
                                @click="undo"
                            >
                                <UndoIcon />
                            </IconButton>
                            <IconButton
                                v-tooltip="formatMessage(messages.redo)"
                                :label="formatMessage(messages.redo)"
                                type="quiet"
                                :disabled="!canRedo"
                                @click="redo"
                            >
                                <RedoIcon />
                            </IconButton>
                        </div>
                        <div class="h-6 w-px bg-white/10" />
                        <Button type="quiet" :disabled="saving" @click="cancelEditing">
                            {{ formatMessage(commonMessages.cancelButton) }}
                        </Button>
                        <Button
                            v-tooltip="formatMessage(messages.saveAsCopy)"
                            type="colored"
                            color="brand"
                            :disabled="saving || !dirty"
                            :loading="saving"
                            @click="requestSave('create_copy')"
                        >
                            {{ formatMessage(messages.saveAsCopy) }}
                        </Button>
                        <Button
                            v-tooltip="formatMessage(messages.overwrite)"
                            type="colored"
                            color="brand"
                            :disabled="saving || !dirty"
                            @click="requestSave('replace_edit')"
                        >
                            {{ formatMessage(messages.overwrite) }}
                        </Button>
                    </div>
                </template>

                <div
                    v-else
                    class="absolute bottom-6 left-1/2 z-10 flex max-w-[calc(100%_-_3rem)] -translate-x-1/2 items-center gap-2 rounded-[20px] border border-solid border-white/10 bg-surface-3 p-2 shadow-[0_1rem_3rem_rgb(0_0_0_/_32%)]"
                    @click.stop
                >
                    <div v-if="items.length > 1" class="flex items-center gap-2">
                        <IconButton
                            :label="formatMessage(messages.previous)"
                            type="quiet"
                            @click="previous"
                        >
                            <ArrowLeftIcon aria-hidden="true" />
                        </IconButton>
                        <span
                            class="flex min-w-14 justify-center gap-1 text-base font-semibold tabular-nums leading-5 text-[var(--color-text-tertiary)]"
                            aria-live="polite"
                        >
                            <strong class="font-semibold text-[var(--color-text-primary)]">{{
                                activeIndex + 1
                            }}</strong>
                            <span>/ {{ items.length }}</span>
                        </span>
                        <IconButton
                            :label="formatMessage(messages.next)"
                            type="quiet"
                            @click="next"
                        >
                            <ArrowRightIcon aria-hidden="true" />
                        </IconButton>
                    </div>
                    <div v-if="items.length > 1" class="h-6 w-px bg-white/10" />
                    <div class="flex items-center gap-2">
                        <IconButton
                            v-tooltip="formatMessage(messages.zoomOut)"
                            :label="formatMessage(messages.zoomOut)"
                            type="quiet"
                            :disabled="zoom <= 1"
                            @click="zoomOut"
                        >
                            <ZoomOutIcon aria-hidden="true" />
                        </IconButton>
                        <Button
                            v-tooltip="formatMessage(messages.fitToWorkspace)"
                            type="quiet"
                            class="w-16 px-2 tabular-nums text-[var(--color-text-tertiary)]"
                            @click="resetZoom"
                        >
                            {{ zoomPercent }}%
                        </Button>
                        <IconButton
                            v-tooltip="formatMessage(messages.zoomIn)"
                            :label="formatMessage(messages.zoomIn)"
                            type="quiet"
                            :disabled="zoom >= 5"
                            @click="zoomIn"
                        >
                            <ZoomInIcon aria-hidden="true" />
                        </IconButton>
                    </div>
                    <div class="h-6 w-px bg-white/10" />
                    <IconButton
                        v-tooltip="formatMessage(messages.edit)"
                        :label="formatMessage(messages.edit)"
                        type="quiet"
                        @click="beginEditing"
                    >
                        <EditIcon aria-hidden="true" />
                    </IconButton>
                    <div class="flex items-center gap-2">
                        <slot name="actions" :item="activeItem" />
                    </div>
                    <div class="h-6 w-px bg-white/10" />
                    <IconButton :label="formatMessage(messages.close)" type="quiet" @click="hide">
                        <XIcon aria-hidden="true" />
                    </IconButton>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
.image-viewer-enter-active {
    transition: opacity 180ms ease-out;
}

.image-viewer-leave-active {
    transition: opacity 120ms ease-in;
}

.image-viewer-enter-from,
.image-viewer-leave-to {
    opacity: 0;
}

.viewer-heading-text {
    filter: drop-shadow(0 2px 3px rgb(0 0 0 / 90%));
}

/* White and black are literal because the crop marquee sits on the screenshot
   rather than a themed surface; the rungs keep both layers on the shared opacity
   scale instead of the /NN alpha utilities, which emit no CSS. */
.crop-selection {
    border-color: color-mix(in srgb, white var(--opacity-ratio-keep-90, 90%), transparent);
    background-color: color-mix(in srgb, black var(--opacity-ratio-keep-25, 25%), transparent);
}

.editor-viewport {
    scrollbar-width: none;
}

.editor-viewport::-webkit-scrollbar {
    display: none;
}

@media (prefers-reduced-motion: reduce) {
    .image-viewer-enter-active,
    .image-viewer-leave-active {
        transition: none;
    }
}
</style>
