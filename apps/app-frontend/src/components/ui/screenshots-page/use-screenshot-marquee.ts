import { onScopeDispose, type Ref, ref } from 'vue'

import {
    isSelectionModifier,
    rectangleSelection,
    type SelectionRect,
    type SelectionTarget,
} from './screenshot-selection'

/** Blank-area or modified-card drags select; ordinary card drags remain owned by dnd-kit. */
export function useScreenshotMarquee(options: {
    container: Ref<HTMLElement | null | undefined>
    disabled: () => boolean
    targets: () => readonly SelectionTarget[]
    snapshot: () => ReadonlySet<string>
    apply: (keys: readonly string[], base: ReadonlySet<string>) => void
}) {
    const rectangle = ref<SelectionRect>()
    let gesture:
        | {
              id: number
              x: number
              y: number
              base: ReadonlySet<string>
              original: ReadonlySet<string>
              moved: boolean
              cancelled?: boolean
          }
        | undefined
    let suppressClick = false
    let clickTimer: ReturnType<typeof setTimeout> | undefined

    function removeListeners() {
        document.removeEventListener('pointermove', move, true)
        document.removeEventListener('pointerup', end, true)
        document.removeEventListener('pointercancel', release)
        document.removeEventListener('keydown', keydown, true)
        window.removeEventListener('blur', release)
        document.removeEventListener('scroll', refresh, true)
    }

    let pointer = { x: 0, y: 0 }
    function refresh() {
        const container = options.container.value
        if (!gesture?.moved || gesture.cancelled || !container) return
        const box = container.getBoundingClientRect()
        const x = Math.max(0, Math.min(box.width, pointer.x - box.left))
        const y = Math.max(0, Math.min(box.height, pointer.y - box.top))
        const rect = {
            left: Math.min(gesture.x, x),
            top: Math.min(gesture.y, y),
            width: Math.abs(x - gesture.x),
            height: Math.abs(y - gesture.y),
        }
        rectangle.value = { ...rect, left: rect.left + box.left, top: rect.top + box.top }
        options.apply(rectangleSelection(rect, options.targets()), gesture.base)
    }

    function move(event: PointerEvent) {
        if (!gesture || gesture.cancelled || event.pointerId !== gesture.id) return
        if (options.disabled()) {
            cancel()
            return
        }
        pointer = { x: event.clientX, y: event.clientY }
        const box = options.container.value?.getBoundingClientRect()
        if (!box) {
            cancel()
            return
        }
        if (
            !gesture.moved &&
            Math.hypot(pointer.x - box.left - gesture.x, pointer.y - box.top - gesture.y) < 4
        )
            return
        gesture.moved = true
        event.preventDefault()
        refresh()
    }

    function end(event: PointerEvent) {
        if (!gesture || event.pointerId !== gesture.id) return
        move(event)
        if (gesture?.moved) {
            suppressClick = true
            clearTimeout(clickTimer)
            clickTimer = setTimeout(() => {
                suppressClick = false
            }, 0)
        }
        gesture = undefined
        rectangle.value = undefined
        removeListeners()
    }

    function cancel() {
        if (gesture?.moved && !gesture.cancelled) options.apply([], gesture.original)
        rectangle.value = undefined
        if (gesture) gesture.cancelled = true
        else removeListeners()
    }

    function release() {
        cancel()
        gesture = undefined
        removeListeners()
    }

    function keydown(event: KeyboardEvent) {
        if (event.key === 'Escape') {
            event.preventDefault()
            event.stopPropagation()
            cancel()
        }
    }

    function start(event: PointerEvent) {
        if (
            event.button !== 0 ||
            event.pointerType !== 'mouse' ||
            options.disabled() ||
            !options.container.value
        )
            return
        const target = event.target instanceof Element ? event.target : null
        if (target?.closest('[data-screenshot-group-header]')) return
        const card = target?.closest('[data-screenshot-card]')
        if (target?.closest('button, input, textarea, select, a, [contenteditable="true"]')) return
        if (card && !isSelectionModifier(event)) return
        if (!card && target?.closest('[role="button"]')) return
        removeListeners()
        const box = options.container.value.getBoundingClientRect()
        const original = new Set(options.snapshot())
        pointer = { x: event.clientX, y: event.clientY }
        gesture = {
            id: event.pointerId,
            x: event.clientX - box.left,
            y: event.clientY - box.top,
            original,
            base: isSelectionModifier(event) ? original : new Set(),
            moved: false,
        }
        document.addEventListener('pointermove', move, true)
        document.addEventListener('pointerup', end, true)
        document.addEventListener('pointercancel', release)
        document.addEventListener('keydown', keydown, true)
        window.addEventListener('blur', release)
        document.addEventListener('scroll', refresh, true)
    }

    function click(event: MouseEvent) {
        if (!suppressClick) return false
        suppressClick = false
        event.preventDefault()
        event.stopPropagation()
        return true
    }

    onScopeDispose(() => {
        removeListeners()
        clearTimeout(clickTimer)
        gesture = undefined
    })
    return { rectangle, start, click, cancel }
}
