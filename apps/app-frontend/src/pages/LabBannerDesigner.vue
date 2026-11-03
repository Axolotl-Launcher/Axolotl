<script setup lang="ts">
import {
    ChevronDownIcon,
    DownloadIcon,
    HomeIcon,
    PaintbrushIcon,
    PaletteIcon,
    PlusIcon,
    RedoIcon,
    TrashIcon,
    UndoIcon,
    UploadIcon,
} from '@modrinth/assets'
import { Button, defineMessages, EmptyState, TagItem, useVIntl } from '@modrinth/ui'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useTemplateRef } from 'vue'

import BannerBaseColorModal from '@/components/lab/banner-designer/BannerBaseColorModal.vue'
import BannerCreateModal from '@/components/lab/banner-designer/BannerCreateModal.vue'
import BannerExportModal from '@/components/lab/banner-designer/BannerExportModal.vue'
import BannerImportModal from '@/components/lab/banner-designer/BannerImportModal.vue'
import BannerLayerCard from '@/components/lab/banner-designer/BannerLayerCard.vue'
import BannerLayerEditorModal from '@/components/lab/banner-designer/BannerLayerEditorModal.vue'
import BannerPreview from '@/components/lab/banner-designer/BannerPreview.vue'
import BannerRecentCard from '@/components/lab/banner-designer/BannerRecentCard.vue'
import BannerReplaceColorModal from '@/components/lab/banner-designer/BannerReplaceColorModal.vue'
import type { DyeColor } from '@/lab/banner-designer/colors.ts'
import {
    type DesignHistory,
    emptyHistory,
    recordHistory,
    redoHistory,
    undoHistory,
} from '@/lab/banner-designer/history.ts'
import { BANNER_FACE_HEIGHT, BANNER_FACE_WIDTH } from '@/lab/banner-designer/masks.ts'
import { colorNameMessages } from '@/lab/banner-designer/messages.ts'
import { type BannerPatternId, MAX_BANNER_LAYERS } from '@/lab/banner-designer/patterns.ts'
import {
    type BannerColorReplaceOptions,
    replaceBannerColors,
} from '@/lab/banner-designer/recolor.ts'
import {
    clearRecentBanners,
    loadRecentBanners,
    type RecentBanner,
    recordRecentBanner,
    removeRecentBanner,
} from '@/lab/banner-designer/storage.ts'
import {
    type BannerDesign,
    type BannerLayer,
    cloneBannerDesign,
    createBannerLayer,
} from '@/lab/banner-designer/types.ts'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    emptyHeading: {
        id: 'app.lab.banner-designer.empty.heading',
        defaultMessage: 'Design a banner',
    },
    emptyDescription: {
        id: 'app.lab.banner-designer.empty.description',
        defaultMessage:
            'Start from a base color or a preset, or import a banner from a give command or item data.',
    },
    create: { id: 'app.lab.banner-designer.create', defaultMessage: 'Create banner' },
    import: { id: 'app.lab.banner-designer.import', defaultMessage: 'Import banner' },
    toolImport: { id: 'app.lab.banner-designer.tools.import', defaultMessage: 'Import' },
    tools: { id: 'app.lab.banner-designer.tools', defaultMessage: 'Tools' },
    toolExport: { id: 'app.lab.banner-designer.tools.export', defaultMessage: 'Export' },
    toolBaseColor: {
        id: 'app.lab.banner-designer.tools.base-color',
        defaultMessage: 'Base color',
    },
    toolReplaceColors: {
        id: 'app.lab.banner-designer.tools.replace-colors',
        defaultMessage: 'Replace colors',
    },
    layers: { id: 'app.lab.banner-designer.layers', defaultMessage: 'Layers' },
    layerCount: {
        id: 'app.lab.banner-designer.layers.count',
        defaultMessage: '{count} / {max}',
    },
    addLayer: { id: 'app.lab.banner-designer.layers.add', defaultMessage: 'Add layer' },
    layersHint: {
        id: 'app.lab.banner-designer.layers.hint',
        defaultMessage:
            'The top of the list is the top layer: later patterns paint over earlier ones.',
    },
    layersEmpty: {
        id: 'app.lab.banner-designer.layers.empty',
        defaultMessage: 'No patterns yet. Add a layer to paint a pattern onto the banner.',
    },
    layerLimit: {
        id: 'app.lab.banner-designer.layers.limit',
        defaultMessage: 'A banner holds at most {max} patterns.',
    },
    layerMoved: {
        id: 'app.lab.banner-designer.layers.moved',
        defaultMessage: 'Layer moved to position {position}',
    },
    exit: { id: 'app.lab.banner-designer.exit', defaultMessage: 'Exit' },
    undo: { id: 'app.lab.banner-designer.undo', defaultMessage: 'Undo' },
    redo: { id: 'app.lab.banner-designer.redo', defaultMessage: 'Redo' },
    recent: { id: 'app.lab.banner-designer.recent', defaultMessage: 'Recently edited' },
    expandRecent: {
        id: 'app.lab.banner-designer.recent.expand',
        defaultMessage: 'Expand',
    },
    collapseRecent: {
        id: 'app.lab.banner-designer.recent.collapse',
        defaultMessage: 'Collapse',
    },
    clearRecent: {
        id: 'app.lab.banner-designer.clear-recent',
        defaultMessage: 'Clear recent',
    },
    preview: { id: 'app.lab.banner-designer.preview', defaultMessage: 'Banner preview' },
    patternCount: {
        id: 'app.lab.banner-designer.pattern-count',
        defaultMessage: '{count, plural, one {# pattern} other {# patterns}}',
    },
})

const design = ref<BannerDesign | null>(null)
const editingLayer = ref<BannerLayer | null>(null)
const recent = ref<RecentBanner[]>(loadRecentBanners())
const recentExpanded = ref(false)
/** Spoken after a keyboard reorder, since the list moves without the focus. */
const layerAnnouncement = ref('')

const history = ref<DesignHistory>(emptyHistory())
const canUndo = computed(() => history.value.past.length > 0)
const canRedo = computed(() => history.value.future.length > 0)

/** Number of cards drawn behind the top of the recent deck. */
const deckDepth = computed(() => Math.min(2, Math.max(0, recent.value.length - 1)))
/**
 * The deck always shows its most recent banner. The older ones live in a clipped
 * region below it, so the deck can unfold without reflowing the start screen.
 */
const headRecent = computed<RecentBanner | null>(() => recent.value[0] ?? null)
const tailRecent = computed(() => recent.value.slice(1))

/**
 * Live drag state. `to` is an insertion index into the list with the dragged
 * card taken out, so it runs from 0 to `layers.length - 1`.
 */
const drag = ref<{
    from: number
    to: number
    /** The pointer that started the drag; every other pointer is ignored. */
    pointerId: number
    pointer: { x: number; y: number }
    offset: { x: number; y: number }
    width: number
    height: number
} | null>(null)

const createModal = useTemplateRef<InstanceType<typeof BannerCreateModal>>('createModal')
const importModal = useTemplateRef<InstanceType<typeof BannerImportModal>>('importModal')
const exportModal = useTemplateRef<InstanceType<typeof BannerExportModal>>('exportModal')
const baseColorModal = useTemplateRef<InstanceType<typeof BannerBaseColorModal>>('baseColorModal')
const replaceColorModal =
    useTemplateRef<InstanceType<typeof BannerReplaceColorModal>>('replaceColorModal')
const layerEditor = useTemplateRef<InstanceType<typeof BannerLayerEditorModal>>('layerEditor')

const previewArea = useTemplateRef<HTMLElement>('previewArea')
const layerList = useTemplateRef<HTMLElement>('layerList')
const areaSize = ref({ width: 0, height: 0 })
let resizeObserver: ResizeObserver | undefined

const previewScale = computed(() => {
    const { width, height } = areaSize.value
    const limitY = Math.floor(((height || 480) - 96) / BANNER_FACE_HEIGHT)
    const limitX = Math.floor(((width || 480) - 96) / BANNER_FACE_WIDTH)
    // Never force a floor above the space available: a short window would push
    // the preview past its container. One pixel per texel is the smallest
    // faithful render, so that is the only hard floor.
    return Math.max(1, Math.min(12, limitY, limitX))
})

const layerCount = computed(() => design.value?.layers.length ?? 0)
const atLayerLimit = computed(() => layerCount.value >= MAX_BANNER_LAYERS)
const baseColorName = computed(() =>
    design.value ? formatMessage(colorNameMessages[design.value.baseColor]) : '',
)

/**
 * Layers paint over each other, so the last entry of `design.layers` is the
 * topmost pattern. The list shows the topmost layer first, which means it walks
 * the array backwards.
 */
const orderedLayers = computed(() => (design.value ? [...design.value.layers].reverse() : []))

function openDesign(next: BannerDesign) {
    design.value = cloneBannerDesign(next)
    editingLayer.value = null
    history.value = emptyHistory()
}

/** Keeps the banner that is being edited so it can be reopened from the start screen. */
function rememberDesign() {
    if (design.value) recent.value = recordRecentBanner(design.value)
}

function closeDesign() {
    rememberDesign()
    design.value = null
    editingLayer.value = null
    history.value = emptyHistory()
    cancelDrag()
}

function openRecent(record: RecentBanner) {
    openDesign(record.design)
}

function clearRecent() {
    recent.value = clearRecentBanners()
    recentExpanded.value = false
}

function showLayerEditor(layer: BannerLayer | null) {
    editingLayer.value = layer
    // Pass the layer along: the prop reaches the dialog a tick later, and its
    // `show` would seed the editor from the layer it still has.
    layerEditor.value?.show(layer)
}

/** Replaces the design and remembers the state it replaced, so it can be undone. */
function commit(next: BannerDesign) {
    if (!design.value) return
    history.value = recordHistory(history.value, cloneBannerDesign(design.value))
    design.value = next
}

function undo() {
    if (!design.value) return
    const step = undoHistory(history.value, cloneBannerDesign(design.value))
    if (!step) return
    history.value = step.history
    design.value = step.design
    editingLayer.value = null
}

function redo() {
    if (!design.value) return
    const step = redoHistory(history.value, cloneBannerDesign(design.value))
    if (!step) return
    history.value = step.history
    design.value = step.design
    editingLayer.value = null
}

function applyLayer(pattern: BannerPatternId, color: DyeColor) {
    if (!design.value) return
    const editing = editingLayer.value
    if (editing) {
        const index = design.value.layers.findIndex((layer) => layer.id === editing.id)
        if (index >= 0) {
            const layers = [...design.value.layers]
            layers[index] = { ...editing, pattern, color }
            commit({ ...design.value, layers })
        }
    } else if (!atLayerLimit.value) {
        commit({
            ...design.value,
            layers: [...design.value.layers, createBannerLayer(pattern, color)],
        })
    }
    editingLayer.value = null
}

function removeLayer(id: string) {
    if (!design.value) return
    commit({
        ...design.value,
        layers: design.value.layers.filter((layer) => layer.id !== id),
    })
}

function moveLayer(displayIndex: number, offset: number) {
    const total = design.value?.layers.length ?? 0
    const target = displayIndex + offset
    if (!design.value || target < 0 || target >= total) return
    reorder(displayIndex, target)
    // Only a keyboard move lands here, so this is the one path that has to say
    // out loud what happened: the list reorders without the focus moving.
    layerAnnouncement.value = formatMessage(messages.layerMoved, { position: total - target })
}

/** Moves the layer at `from` so that it ends up at `to`, both top-first indexes. */
function reorder(from: number, to: number) {
    if (!design.value) return
    const ordered = [...design.value.layers].reverse()
    if (from === to || from < 0 || to < 0 || from >= ordered.length || to >= ordered.length) return
    const [moved] = ordered.splice(from, 1)
    ordered.splice(to, 0, moved)
    commit({ ...design.value, layers: ordered.reverse() })
}

function setBaseColor(color: DyeColor) {
    if (!design.value || design.value.baseColor === color) return
    commit({ ...design.value, baseColor: color })
}

function replaceColors(options: BannerColorReplaceOptions) {
    if (!design.value) return
    const next = replaceBannerColors(design.value, options)
    // A replacement that matches nothing returns the design itself, so an empty
    // action does not fill the undo history.
    if (next === design.value) return
    commit(next)
}

/**
 * Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z (or Ctrl+Y). Text fields and open dialogs keep
 * their own editing history, so the page stays out of the way there.
 */
function handleShortcut(event: KeyboardEvent) {
    if (!design.value || drag.value || event.altKey) return
    if (!event.ctrlKey && !event.metaKey) return
    const target = event.target as HTMLElement | null
    if (target?.closest('input, textarea, select, [contenteditable="true"], [role="dialog"]'))
        return
    const key = event.key.toLowerCase()
    if (key === 'z') {
        event.preventDefault()
        if (event.shiftKey) redo()
        else undo()
    } else if (key === 'y') {
        event.preventDefault()
        redo()
    }
}

/**
 * Pointer-driven reordering. HTML5 drag and drop never reported usable drop
 * targets inside the webview, so the list tracks the pointer itself: the dragged
 * card leaves the list, a floating copy follows the pointer, and the dashed
 * placeholder is inserted at the slot the pointer is currently over.
 */
function startDrag(displayIndex: number, event: PointerEvent) {
    if (!design.value || design.value.layers.length < 2) return
    // A second finger must not take over a drag that is already running.
    if (drag.value) return
    event.preventDefault()
    // Settle any running move animation first: the pointer offset must come from
    // the resting position of the card, not from a frame halfway through a slide.
    const snapshot = captureLayout()
    const card = layerList.value?.querySelector<HTMLElement>(`[data-layer-slot="${displayIndex}"]`)
    const bounds = card?.getBoundingClientRect()
    if (!bounds) return
    drag.value = {
        from: displayIndex,
        to: displayIndex,
        pointerId: event.pointerId,
        pointer: { x: event.clientX, y: event.clientY },
        offset: { x: event.clientX - bounds.left, y: event.clientY - bounds.top },
        width: bounds.width,
        height: bounds.height,
    }
    document.body.style.userSelect = 'none'
    window.addEventListener('pointermove', trackPointer)
    window.addEventListener('pointerup', endDrag)
    window.addEventListener('pointercancel', cancelDrag)
    // The dragged card only leaves the list on the next frame; measure after that.
    void nextTick(() => updateTarget(event, snapshot))
}

/** Index the pointer would insert at, among the cards currently rendered. */
function computeTarget(event: PointerEvent): number {
    const list = layerList.value
    if (!list) return 0
    const cards = [...list.querySelectorAll<HTMLElement>('[data-layer-slot]')]
    // offsetTop ignores the FLIP transforms, so a running animation cannot move the drop target.
    const pointerY = event.clientY - list.getBoundingClientRect().top
    for (let index = 0; index < cards.length; index++) {
        if (pointerY < cards[index].offsetTop + cards[index].offsetHeight / 2) return index
    }
    return cards.length
}

function trackPointer(event: PointerEvent) {
    if (event.pointerId !== drag.value?.pointerId) return
    updateTarget(event)
}

function updateTarget(event: PointerEvent, snapshot?: FlipSnapshot) {
    const state = drag.value
    if (!state) return
    state.pointer = { x: event.clientX, y: event.clientY }
    const target = computeTarget(event)
    if (target === state.to) {
        if (snapshot) void nextTick(() => playLayout(snapshot))
        return
    }
    const before = snapshot ?? captureLayout()
    state.to = target
    void nextTick(() => playLayout(before))
}

/** Drops the drag state along with the listeners that follow it. */
function resetDrag() {
    drag.value = null
    document.body.style.userSelect = ''
    window.removeEventListener('pointermove', trackPointer)
    window.removeEventListener('pointerup', endDrag)
    window.removeEventListener('pointercancel', cancelDrag)
}

function endDrag(event?: PointerEvent) {
    const state = drag.value
    if (!state) return
    if (event && event.pointerId !== state.pointerId) return
    const snapshot = state.from !== state.to ? captureLayout() : null
    resetDrag()
    if (state.from === state.to) return
    reorder(state.from, state.to)
    if (snapshot) void nextTick(() => playLayout(snapshot))
}

/**
 * `pointercancel` means the browser took the pointer away mid-move, so the
 * gesture never completed: the list goes back to the order it started in
 * instead of applying the slot the pointer happened to be over.
 */
function cancelDrag(event?: PointerEvent) {
    const state = drag.value
    if (!state) return
    if (event && event.pointerId !== state.pointerId) return
    const snapshot = state.from !== state.to ? captureLayout() : null
    resetDrag()
    if (snapshot) void nextTick(() => playLayout(snapshot))
}

type FlipSnapshot = Map<string, number>

const MOVE_ANIMATION = 'layer-move'
const MOVE_DURATION = 130

function prefersReducedMotion() {
    return (
        typeof window !== 'undefined' &&
        window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true
    )
}

/**
 * Records where every card sits before the list re-renders. Running move
 * animations are cancelled first so the measurement is the resting position.
 */
function captureLayout(): FlipSnapshot {
    const snapshot: FlipSnapshot = new Map()
    for (const element of layerList.value?.querySelectorAll<HTMLElement>('[data-flip-key]') ?? []) {
        for (const animation of element.getAnimations()) {
            if (animation.id === MOVE_ANIMATION) animation.cancel()
        }
        snapshot.set(element.dataset.flipKey ?? '', element.getBoundingClientRect().top)
    }
    return snapshot
}

/** Slides every card from its recorded position to the one it just moved to. */
function playLayout(snapshot: FlipSnapshot) {
    if (prefersReducedMotion()) return
    for (const element of layerList.value?.querySelectorAll<HTMLElement>('[data-flip-key]') ?? []) {
        const before = snapshot.get(element.dataset.flipKey ?? '')
        if (before === undefined) continue
        const delta = before - element.getBoundingClientRect().top
        if (Math.abs(delta) < 1) continue
        element.animate([{ transform: `translateY(${delta}px)` }, { transform: 'translateY(0)' }], {
            id: MOVE_ANIMATION,
            duration: MOVE_DURATION,
            easing: 'cubic-bezier(0.2, 0, 0, 1)',
        })
    }
}

/** The card lifted out of the list, drawn as a floating copy under the pointer. */
const draggedLayer = computed(() =>
    drag.value ? (orderedLayers.value[drag.value.from] ?? null) : null,
)

/**
 * Index in the rendered list where the dashed placeholder goes. The dragged card
 * is skipped while dragging, so every slot after it shifts up by one.
 */
const ghostSlot = computed(() => {
    const state = drag.value
    if (!state) return null
    return state.to < state.from ? state.to : state.to + 1
})

/** One row of the layer list: a layer card, or the dashed drop placeholder. */
interface LayerSlot {
    key: string
    layer: BannerLayer | null
    index: number
}

/**
 * What the list renders: every card plus the placeholder. The placeholder keeps
 * a constant key so it is the same element across moves and can be animated.
 */
const layerSlots = computed<LayerSlot[]>(() => {
    const ghost = ghostSlot.value
    const slots: LayerSlot[] = []
    orderedLayers.value.forEach((layer, index) => {
        if (ghost === index) slots.push({ key: 'ghost', layer: null, index })
        if (drag.value?.from === index) return
        slots.push({ key: layer.id, layer, index })
    })
    if (ghost === orderedLayers.value.length) {
        slots.push({ key: 'ghost', layer: null, index: orderedLayers.value.length })
    }
    return slots
})

const ghostStyle = computed(() => ({
    height: `${drag.value?.height ?? 0}px`,
    backgroundColor: 'var(--color-brand-highlight)',
}))

const dragPreviewStyle = computed(() => {
    const state = drag.value
    if (!state) return {}
    return {
        left: `${state.pointer.x - state.offset.x}px`,
        top: `${state.pointer.y - state.offset.y}px`,
        width: `${state.width}px`,
    }
})

onMounted(() => {
    window.addEventListener('keydown', handleShortcut)
    if (!previewArea.value || typeof ResizeObserver === 'undefined') return
    resizeObserver = new ResizeObserver(([entry]) => {
        areaSize.value = {
            width: entry.contentRect.width,
            height: entry.contentRect.height,
        }
    })
    resizeObserver.observe(previewArea.value)
})

onBeforeUnmount(() => {
    rememberDesign()
    window.removeEventListener('keydown', handleShortcut)
    resizeObserver?.disconnect()
    document.body.style.userSelect = ''
    window.removeEventListener('pointermove', trackPointer)
    window.removeEventListener('pointerup', endDrag)
    window.removeEventListener('pointercancel', cancelDrag)
})
</script>

<template>
    <main class="flex h-[calc(100vh-4rem)] min-h-[36rem] w-full flex-col overflow-hidden">
        <template v-if="!design">
            <div class="relative flex flex-1 flex-col overflow-y-auto">
                <section
                    class="flex flex-1 flex-col items-center justify-center gap-6 p-8"
                    :class="recent.length ? 'pb-32' : ''"
                >
                    <EmptyState
                        type="no-documents"
                        :heading="formatMessage(messages.emptyHeading)"
                        :description="formatMessage(messages.emptyDescription)"
                    >
                        <template #actions>
                            <Button type="colored" color="brand" @click="createModal?.show()"
                                ><PlusIcon />{{ formatMessage(messages.create) }}
                            </Button>
                            <Button type="outlined" @click="importModal?.show()"
                                ><DownloadIcon />{{ formatMessage(messages.import) }}
                            </Button>
                        </template>
                    </EmptyState>
                </section>

                <!-- The deck floats over the start screen, so unfolding it covers the
                     content above instead of pushing it around. -->
                <section v-if="recent.length" class="absolute inset-x-0 bottom-0">
                    <div class="mx-auto w-[min(40rem,calc(100%-3rem))] pb-8">
                        <header class="mb-3 flex items-center justify-between gap-3">
                            <h2 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.recent) }}
                            </h2>
                            <div class="flex items-center gap-1">
                                <Button
                                    v-if="recent.length > 1"
                                    type="quiet"
                                    size="2xs"
                                    @click="recentExpanded = !recentExpanded"
                                >
                                    <ChevronDownIcon
                                        class="transition-transform duration-200"
                                        :class="recentExpanded ? 'rotate-180' : ''"
                                    />{{
                                        formatMessage(
                                            recentExpanded
                                                ? messages.collapseRecent
                                                : messages.expandRecent,
                                        )
                                    }}
                                </Button>
                                <Button type="quiet" size="2xs" @click="clearRecent">
                                    <TrashIcon />{{ formatMessage(messages.clearRecent) }}
                                </Button>
                            </div>
                        </header>

                        <div class="relative">
                            <!-- The edges of the deck, shown while the stack is collapsed. -->
                            <div
                                v-for="depth in deckDepth"
                                :key="`deck-${depth}`"
                                class="pointer-events-none absolute inset-x-0 top-0 h-full rounded-[var(--radius-md)] border border-surface-5 bg-surface-2 transition-opacity duration-200 motion-reduce:transition-none"
                                :class="recentExpanded ? 'opacity-0' : 'opacity-100'"
                                :style="{
                                    transform: `translateY(${depth * 6}px) scale(${1 - depth * 0.02})`,
                                }"
                                aria-hidden="true"
                            ></div>

                            <ul v-if="headRecent" class="relative m-0 list-none p-0">
                                <BannerRecentCard
                                    :record="headRecent"
                                    @open="openRecent(headRecent)"
                                    @remove="recent = removeRecentBanner(headRecent.id)"
                                />
                            </ul>
                        </div>

                        <!-- Older banners unfold out of a clipped row under the top card. -->
                        <div
                            class="relative grid transition-[grid-template-rows] duration-200 ease-out motion-reduce:transition-none"
                            :style="{ gridTemplateRows: recentExpanded ? '1fr' : '0fr' }"
                        >
                            <div class="min-h-0 overflow-hidden">
                                <ul class="m-0 flex list-none flex-col gap-2 pt-2 pl-0">
                                    <BannerRecentCard
                                        v-for="record in tailRecent"
                                        :key="record.id"
                                        :record="record"
                                        @open="openRecent(record)"
                                        @remove="recent = removeRecentBanner(record.id)"
                                    />
                                </ul>
                            </div>
                        </div>
                    </div>
                </section>
            </div>
        </template>

        <template v-else>
            <div class="flex min-h-0 flex-1">
                <section
                    ref="previewArea"
                    class="flex min-w-0 flex-1 items-center justify-center overflow-hidden p-6"
                >
                    <BannerPreview
                        :design="design"
                        :scale="previewScale"
                        :label="formatMessage(messages.preview)"
                        class="rounded-[2px] shadow-[0_18px_48px_-12px_rgb(0_0_0_/_45%)]"
                    />
                </section>

                <aside
                    class="flex w-[22rem] shrink-0 flex-col border-l border-surface-5 bg-surface-2"
                >
                    <div
                        class="flex shrink-0 items-center gap-2 border-b border-surface-5 px-3 py-2"
                    >
                        <Button type="quiet" size="sm" @click="closeDesign"
                            ><HomeIcon />{{ formatMessage(messages.exit) }}
                        </Button>
                        <Button
                            v-tooltip="formatMessage(messages.undo)"
                            type="quiet"
                            size="sm"
                            circular
                            icon-only
                            :disabled="!canUndo"
                            :aria-label="formatMessage(messages.undo)"
                            @click="undo"
                            ><UndoIcon />
                        </Button>
                        <Button
                            v-tooltip="formatMessage(messages.redo)"
                            type="quiet"
                            size="sm"
                            circular
                            icon-only
                            :disabled="!canRedo"
                            :aria-label="formatMessage(messages.redo)"
                            @click="redo"
                            ><RedoIcon />
                        </Button>
                        <div class="ml-auto flex min-w-0 items-center gap-2">
                            <TagItem class="shrink-0">{{ baseColorName }}</TagItem>
                            <span class="shrink-0 text-xs text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.patternCount, { count: layerCount }) }}
                            </span>
                        </div>
                    </div>

                    <section class="shrink-0 border-b border-surface-5 p-3">
                        <h2
                            class="m-0 mb-2 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.tools) }}
                        </h2>
                        <div class="grid grid-cols-2 gap-2">
                            <button
                                type="button"
                                class="flex cursor-pointer flex-col items-center gap-1.5 rounded-[var(--radius-md)] border border-surface-5 bg-surface-1 px-2 py-2.5 text-xs font-medium text-[var(--color-text-primary)] transition-[border-color,background-color] hover:border-brand hover:bg-surface-3 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
                                @click="importModal?.show()"
                            >
                                <DownloadIcon class="size-4 text-[var(--color-text-tertiary)]" />
                                {{ formatMessage(messages.toolImport) }}
                            </button>
                            <button
                                type="button"
                                class="flex cursor-pointer flex-col items-center gap-1.5 rounded-[var(--radius-md)] border border-surface-5 bg-surface-1 px-2 py-2.5 text-xs font-medium text-[var(--color-text-primary)] transition-[border-color,background-color] hover:border-brand hover:bg-surface-3 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
                                @click="exportModal?.show()"
                            >
                                <UploadIcon class="size-4 text-[var(--color-text-tertiary)]" />
                                {{ formatMessage(messages.toolExport) }}
                            </button>
                            <button
                                type="button"
                                class="flex cursor-pointer flex-col items-center gap-1.5 rounded-[var(--radius-md)] border border-surface-5 bg-surface-1 px-2 py-2.5 text-xs font-medium text-[var(--color-text-primary)] transition-[border-color,background-color] hover:border-brand hover:bg-surface-3 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
                                @click="baseColorModal?.show()"
                            >
                                <PaletteIcon class="size-4 text-[var(--color-text-tertiary)]" />
                                {{ formatMessage(messages.toolBaseColor) }}
                            </button>
                            <button
                                type="button"
                                class="flex cursor-pointer flex-col items-center gap-1.5 rounded-[var(--radius-md)] border border-surface-5 bg-surface-1 px-2 py-2.5 text-xs font-medium text-[var(--color-text-primary)] transition-[border-color,background-color] hover:border-brand hover:bg-surface-3 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
                                @click="replaceColorModal?.show()"
                            >
                                <PaintbrushIcon class="size-4 text-[var(--color-text-tertiary)]" />
                                {{ formatMessage(messages.toolReplaceColors) }}
                            </button>
                        </div>
                    </section>

                    <section class="flex min-h-0 flex-1 flex-col">
                        <header
                            class="flex shrink-0 items-center justify-between gap-3 px-3 pb-2 pt-3"
                        >
                            <div class="min-w-0">
                                <div class="flex items-baseline gap-2">
                                    <h2
                                        class="m-0 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                                    >
                                        {{ formatMessage(messages.layers) }}
                                    </h2>
                                    <span
                                        class="text-xs tabular-nums text-[var(--color-text-tertiary)]"
                                    >
                                        {{
                                            formatMessage(messages.layerCount, {
                                                count: layerCount,
                                                max: MAX_BANNER_LAYERS,
                                            })
                                        }}
                                    </span>
                                </div>
                                <p
                                    class="m-0 mt-1 text-[11px] leading-snug text-[var(--color-text-tertiary)]"
                                >
                                    {{ formatMessage(messages.layersHint) }}
                                </p>
                            </div>
                            <Button
                                v-tooltip="
                                    atLayerLimit
                                        ? formatMessage(messages.layerLimit, {
                                              max: MAX_BANNER_LAYERS,
                                          })
                                        : undefined
                                "
                                type="colored"
                                color="brand"
                                size="2xs"
                                class="shrink-0"
                                :disabled="atLayerLimit"
                                @click="showLayerEditor(null)"
                                ><PlusIcon />{{ formatMessage(messages.addLayer) }}
                            </Button>
                        </header>

                        <div class="min-h-0 flex-1 overflow-y-auto px-3 pb-3">
                            <p class="sr-only" role="status" aria-live="polite">
                                {{ layerAnnouncement }}
                            </p>
                            <p
                                v-if="design.layers.length === 0"
                                class="m-0 rounded-[var(--radius-md)] border border-dashed border-surface-5 p-4 text-center text-xs text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.layersEmpty) }}
                            </p>
                            <ul
                                v-else
                                ref="layerList"
                                class="relative m-0 flex list-none flex-col gap-2 p-0"
                            >
                                <template v-for="slot in layerSlots" :key="slot.key">
                                    <li
                                        v-if="!slot.layer"
                                        data-flip-key="ghost"
                                        class="shrink-0 rounded-[var(--radius-md)] border-2 border-dashed border-brand"
                                        :style="ghostStyle"
                                        aria-hidden="true"
                                    ></li>
                                    <BannerLayerCard
                                        v-else
                                        :data-flip-key="slot.layer.id"
                                        :data-layer-slot="slot.index"
                                        :design="design"
                                        :layer="slot.layer"
                                        :order="layerCount - slot.index"
                                        @edit="showLayerEditor(slot.layer)"
                                        @remove="removeLayer(slot.layer.id)"
                                        @move="(offset) => moveLayer(slot.index, offset)"
                                        @dragstart="(event) => startDrag(slot.index, event)"
                                    />
                                </template>
                            </ul>
                        </div>
                    </section>
                </aside>
            </div>
        </template>

        <BannerCreateModal ref="createModal" @create="openDesign" />
        <BannerImportModal ref="importModal" @import="openDesign" />
        <template v-if="design">
            <BannerLayerEditorModal
                ref="layerEditor"
                :design="design"
                :layer="editingLayer"
                @submit="applyLayer"
            />
            <BannerBaseColorModal
                ref="baseColorModal"
                :base-color="design.baseColor"
                @change="setBaseColor"
            />
            <BannerReplaceColorModal
                ref="replaceColorModal"
                :design="design"
                @replace="replaceColors"
            />
            <BannerExportModal ref="exportModal" :design="design" />
        </template>

        <Teleport to="body">
            <div
                v-if="design && drag && draggedLayer"
                class="pointer-events-none fixed z-50 opacity-95"
                :style="dragPreviewStyle"
            >
                <BannerLayerCard
                    as="div"
                    :design="design"
                    :layer="draggedLayer"
                    :order="layerCount - drag.from"
                    class="shadow-[0_18px_44px_-10px_rgb(0_0_0_/_60%)] ring-1 ring-brand"
                />
            </div>
        </Teleport>
    </main>
</template>
