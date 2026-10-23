import {
    computed,
    type ComputedRef,
    inject,
    type InjectionKey,
    onScopeDispose,
    provide,
    type Ref,
    ref,
    shallowRef,
    watch,
} from 'vue'

import type { PreviewClock } from '../../lib/clock'
import type { PreviewWindowContext } from '../../lib/window'

/**
 * The library's `@dnd-kit/vue` setup (`DragDropProvider` in GridDisplay,
 * `useDraggable` in DraggableInstanceCard, `useDroppable` in InstanceGroup)
 * with the defaults it relies on:
 *
 * - PointerSensor: a mouse drag starts after a 5 px move or a 200 ms hold
 *   (a hold is abandoned after 10 px), a touch drag after 250 ms; the app turns
 *   off `preventActivation`, so a drag can start on the card's buttons too.
 * - Feedback: the card follows the pointer above everything while its slot in
 *   the grid stays reserved; a drop that does not move the card animates it
 *   back to its slot (250 ms, `ease`).
 * - Collision: the group under the pointer wins (closest centre), otherwise
 *   the group the card overlaps most.
 * - AutoScroller: dragging within 20 % of the scroll container's edge scrolls
 *   it, faster closer to the edge (up to 25 px per frame).
 * - Escape stops the drag over the current target; the app's drag-end handler
 *   does not look at `canceled`, so it applies like a drop.
 */
export interface InstanceDragData {
    instanceId: string
    fromGroup: string
}

export interface InstanceGroupDropData {
    groupId: string
}

interface DroppableEntry {
    element: Ref<HTMLElement | undefined>
    disabled: () => boolean
    data: () => InstanceGroupDropData
}

interface Point {
    x: number
    y: number
}

interface ActiveDrag {
    draggableId: string
    data: InstanceDragData
    element: HTMLElement
    /** Card rectangle in the window's own (unscaled) coordinates. */
    origin: { left: number; top: number; width: number; height: number }
    /** Card rectangle in viewport coordinates when the drag started. */
    clientOrigin: { left: number; top: number; width: number; height: number }
    start: Point
    pointer: Point
}

export interface DragFeedback {
    left: number
    top: number
    width: number
    height: number
    x: number
    y: number
    dropping: boolean
}

interface DragDropContext {
    activeId: Ref<string | null>
    targetId: Ref<string | null>
    feedback: Ref<DragFeedback | null>
    registerDroppable: (id: string, entry: DroppableEntry) => () => void
    pointerDown: (
        event: PointerEvent,
        draggable: {
            id: string
            element: HTMLElement
            data: InstanceDragData
            onStart?: () => void
        },
    ) => void
}

const key: InjectionKey<DragDropContext> = Symbol('library-drag-drop')

const DROP_ANIMATION_MS = 250
const CURSOR_STYLE_ID = 'launcher-preview-library-dragging'

export function provideInstanceDragDrop(options: {
    window: PreviewWindowContext
    clock: PreviewClock
    /** Returns whether the drop changed where the card is shown. */
    onDragEnd: (source: InstanceDragData, target: InstanceGroupDropData | null) => boolean
}) {
    const { window: previewWindow, clock } = options
    const droppables = new Map<string, DroppableEntry>()
    const active = shallowRef<ActiveDrag | null>(null)
    const activeId = ref<string | null>(null)
    const targetId = ref<string | null>(null)
    const feedback = ref<DragFeedback | null>(null)

    let pending: {
        event: PointerEvent
        draggable: {
            id: string
            element: HTMLElement
            data: InstanceDragData
            onStart?: () => void
        }
        start: Point
        holdTimer: () => void
        touch: boolean
    } | null = null
    let dropping = false
    let finishTimer: (() => void) | null = null
    let frame = 0
    let disposed = false

    function scaleOf() {
        return previewWindow.scale.value || 1
    }

    function registerDroppable(id: string, entry: DroppableEntry) {
        droppables.set(id, entry)
        return () => {
            if (droppables.get(id) === entry) droppables.delete(id)
            if (targetId.value === id) targetId.value = null
        }
    }

    function detectTarget(pointer: Point, shape: DOMRectLike): string | null {
        let best: { id: string; priority: number; value: number } | null = null
        for (const [id, entry] of droppables) {
            const element = entry.element.value
            if (!element || entry.disabled()) continue
            const rect = element.getBoundingClientRect()
            if (rect.width === 0 && rect.height === 0) continue
            const center = { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }
            const distance = Math.max(1, Math.hypot(center.x - pointer.x, center.y - pointer.y))
            let candidate: { id: string; priority: number; value: number } | null = null
            if (
                pointer.x >= rect.left &&
                pointer.x <= rect.right &&
                pointer.y >= rect.top &&
                pointer.y <= rect.bottom
            ) {
                candidate = { id, priority: 2, value: 1 / distance }
            } else {
                const width = Math.min(shape.right, rect.right) - Math.max(shape.left, rect.left)
                const height = Math.min(shape.bottom, rect.bottom) - Math.max(shape.top, rect.top)
                if (width > 0 && height > 0) {
                    const area = width * height
                    const ratio =
                        area / (shape.width * shape.height + rect.width * rect.height - area)
                    candidate = { id, priority: 1, value: ratio / distance }
                }
            }
            if (
                candidate &&
                (!best ||
                    candidate.priority > best.priority ||
                    (candidate.priority === best.priority && candidate.value > best.value))
            ) {
                best = candidate
            }
        }
        return best?.id ?? null
    }

    function scrollContainerOf(element: HTMLElement): HTMLElement | null {
        let node: HTMLElement | null = element.parentElement
        while (node) {
            const style = getComputedStyle(node)
            if (/(auto|scroll)/.test(style.overflowY) && node.scrollHeight > node.clientHeight) {
                return node
            }
            node = node.parentElement
        }
        return null
    }

    function update() {
        const drag = active.value
        if (!drag || dropping) return
        const scale = scaleOf()
        const dx = (drag.pointer.x - drag.start.x) / scale
        const dy = (drag.pointer.y - drag.start.y) / scale
        feedback.value = { ...drag.origin, x: dx, y: dy, dropping: false }
        const shape = {
            left: drag.clientOrigin.left + (drag.pointer.x - drag.start.x),
            top: drag.clientOrigin.top + (drag.pointer.y - drag.start.y),
            width: drag.clientOrigin.width,
            height: drag.clientOrigin.height,
            right: 0,
            bottom: 0,
        }
        shape.right = shape.left + shape.width
        shape.bottom = shape.top + shape.height
        targetId.value = detectTarget(drag.pointer, shape)
    }

    function autoScroll() {
        frame = 0
        const drag = active.value
        if (!drag || dropping || disposed) return
        const container = scrollContainerOf(drag.element)
        if (container) {
            const rect = container.getBoundingClientRect()
            const threshold = rect.height * 0.2
            const y = drag.pointer.y
            let speed = 0
            if (y <= rect.top + threshold && container.scrollTop > 0) {
                speed = -25 * Math.abs((rect.top + threshold - y) / threshold)
            } else if (
                y >= rect.bottom - threshold &&
                container.scrollTop + container.clientHeight < container.scrollHeight
            ) {
                speed = 25 * Math.abs((rect.bottom - threshold - y) / threshold)
            }
            if (speed) {
                container.scrollTop += speed
                update()
            }
        }
        frame = requestAnimationFrame(autoScroll)
    }

    function setDraggingStyles(on: boolean) {
        const existing = document.getElementById(CURSOR_STYLE_ID)
        if (!on) {
            existing?.remove()
            return
        }
        if (existing) return
        const style = document.createElement('style')
        style.id = CURSOR_STYLE_ID
        style.textContent =
            '.launcher-preview-window, .launcher-preview-window * { cursor: grabbing !important; user-select: none !important; }'
        document.head.appendChild(style)
    }

    function swallowNextClick() {
        const swallow = (event: Event) => {
            event.preventDefault()
            event.stopPropagation()
        }
        window.addEventListener('click', swallow, { capture: true, once: true })
        clock.after(0, () => window.removeEventListener('click', swallow, { capture: true }))
    }

    function activate(event: PointerEvent) {
        const request = pending
        if (!request || active.value || dropping) return
        clearPending()
        const element = request.draggable.element
        if (!element.isConnected) return
        const rect = element.getBoundingClientRect()
        const scale = scaleOf()
        const topLeft = previewWindow.toLocal(rect.left, rect.top)
        active.value = {
            draggableId: request.draggable.id,
            data: request.draggable.data,
            element,
            origin: {
                left: topLeft.x,
                top: topLeft.y,
                width: rect.width / scale,
                height: rect.height / scale,
            },
            clientOrigin: {
                left: rect.left,
                top: rect.top,
                width: rect.width,
                height: rect.height,
            },
            start: request.start,
            pointer: { x: event.clientX, y: event.clientY },
        }
        activeId.value = request.draggable.id
        request.draggable.onStart?.()
        setDraggingStyles(true)
        update()
        window.addEventListener('keydown', onKeyDown, true)
        if (!frame) frame = requestAnimationFrame(autoScroll)
    }

    function clearPending() {
        if (!pending) return
        pending.holdTimer()
        pending = null
    }

    function onPointerMove(event: PointerEvent) {
        if (pending && !active.value) {
            const dx = event.clientX - pending.start.x
            const dy = event.clientY - pending.start.y
            const distance = Math.hypot(dx, dy)
            if (pending.touch) {
                if (distance > 5) {
                    clearPending()
                    removeListeners()
                }
                return
            }
            // Delay constraint: a hold is abandoned beyond 10 px...
            if (distance > 10) pending.holdTimer()
            // ...and the distance constraint activates beyond 5 px.
            if (distance > 5) activate(event)
            return
        }
        const drag = active.value
        if (!drag || dropping) return
        event.preventDefault()
        drag.pointer = { x: event.clientX, y: event.clientY }
        update()
    }

    function onPointerUp(event: PointerEvent) {
        if (active.value && !dropping) {
            event.preventDefault()
            event.stopPropagation()
            swallowNextClick()
            stop()
            return
        }
        clearPending()
        removeListeners()
    }

    function onPointerCancel() {
        if (active.value && !dropping) {
            stop()
            return
        }
        clearPending()
        removeListeners()
    }

    function onKeyDown(event: KeyboardEvent) {
        if (event.key !== 'Escape' || !active.value || dropping) return
        event.preventDefault()
        event.stopPropagation()
        stop()
    }

    function removeListeners() {
        window.removeEventListener('pointermove', onPointerMove, true)
        window.removeEventListener('pointerup', onPointerUp, true)
        window.removeEventListener('pointercancel', onPointerCancel, true)
        window.removeEventListener('keydown', onKeyDown, true)
    }

    function stop() {
        const drag = active.value
        if (!drag) return
        removeListeners()
        if (frame) cancelAnimationFrame(frame)
        frame = 0
        const target = targetId.value ? (droppables.get(targetId.value)?.data() ?? null) : null
        const moved = options.onDragEnd(drag.data, target)
        targetId.value = null
        setDraggingStyles(false)
        if (moved || !drag.element.isConnected) {
            finish()
            return
        }
        // Drop animation back to the reserved slot.
        dropping = true
        const rect = drag.element.getBoundingClientRect()
        const slot = previewWindow.toLocal(rect.left, rect.top)
        feedback.value = {
            ...drag.origin,
            x: slot.x - drag.origin.left,
            y: slot.y - drag.origin.top,
            dropping: true,
        }
        finishTimer = clock.after(DROP_ANIMATION_MS, finish)
    }

    function finish() {
        finishTimer?.()
        finishTimer = null
        dropping = false
        active.value = null
        activeId.value = null
        feedback.value = null
    }

    function pointerDown(
        event: PointerEvent,
        draggable: {
            id: string
            element: HTMLElement
            data: InstanceDragData
            onStart?: () => void
        },
    ) {
        if (disposed || !event.isPrimary || event.button !== 0) return
        if (active.value || dropping || pending) return
        const start = { x: event.clientX, y: event.clientY }
        const touch = event.pointerType === 'touch'
        pending = {
            event,
            draggable,
            start,
            touch,
            holdTimer: () => {},
        }
        const request = pending
        request.holdTimer = clock.after(touch ? 250 : 200, () => {
            if (pending === request) activate(request.event)
        })
        window.addEventListener('pointermove', onPointerMove, true)
        window.addEventListener('pointerup', onPointerUp, true)
        window.addEventListener('pointercancel', onPointerCancel, true)
    }

    onScopeDispose(() => {
        disposed = true
        clearPending()
        removeListeners()
        if (frame) cancelAnimationFrame(frame)
        setDraggingStyles(false)
        finish()
    })

    const context: DragDropContext = {
        activeId,
        targetId,
        feedback,
        registerDroppable,
        pointerDown,
    }
    provide(key, context)
    return context
}

interface DOMRectLike {
    left: number
    top: number
    right: number
    bottom: number
    width: number
    height: number
}

export function useInstanceDraggable(options: {
    id: ComputedRef<string>
    element: Ref<HTMLElement | undefined>
    disabled: ComputedRef<boolean>
    data: ComputedRef<InstanceDragData>
    onStart?: () => void
}) {
    const context = inject(key, null)
    const isDragging = computed(() => context?.activeId.value === options.id.value)

    function onPointerDown(event: PointerEvent) {
        const element = options.element.value
        if (!context || !element || options.disabled.value) return
        context.pointerDown(event, {
            id: options.id.value,
            element,
            data: options.data.value,
            onStart: options.onStart,
        })
    }

    return { isDragging, feedback: context?.feedback ?? ref(null), onPointerDown }
}

export function useInstanceDroppable(options: {
    id: ComputedRef<string>
    element: Ref<HTMLElement | undefined>
    disabled: ComputedRef<boolean>
    data: ComputedRef<InstanceGroupDropData>
}) {
    const context = inject(key, null)
    let unregister: (() => void) | null = null
    watch(
        options.id,
        (id) => {
            unregister?.()
            unregister =
                context?.registerDroppable(id, {
                    element: options.element,
                    disabled: () => options.disabled.value,
                    data: () => options.data.value,
                }) ?? null
        },
        { immediate: true },
    )
    onScopeDispose(() => unregister?.())
    const isDropTarget = computed(
        () => !options.disabled.value && context?.targetId.value === options.id.value,
    )
    return { isDropTarget }
}
