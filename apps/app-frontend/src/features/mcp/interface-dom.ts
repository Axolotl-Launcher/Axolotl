import { nextTick } from 'vue'
import type { Router } from 'vue-router'

import { captureMcpUi } from './interface-capture'

export type UiAction =
    | { kind: 'context' | 'screenshot' }
    | { kind: 'navigate'; path: string }
    | { kind: 'click'; view_id: string; element_id: string }
    | { kind: 'type'; view_id: string; element_id: string; text: string }
    | { kind: 'key'; view_id: string; key: string; shift: boolean; control: boolean }
    | {
          kind: 'scroll'
          view_id: string
          element_id: string
          direction: 'up' | 'down' | 'left' | 'right'
          pixels: number
      }

const selector =
    'button,a[href],input:not([type=password]):not([type=hidden]),textarea,select,[role=button],[role=tab],[role=checkbox],[contenteditable=true],[tabindex]'
type Target = {
    element: HTMLElement
    label: string
    bounds: { x: number; y: number; width: number; height: number }
    state: string
}

function interactionState(element: HTMLElement) {
    return JSON.stringify([
        element.getAttribute('role'),
        element.getAttribute('aria-checked'),
        element.getAttribute('aria-expanded'),
        element.getAttribute('aria-selected'),
        element.getAttribute('aria-disabled'),
        element.getAttribute('readonly'),
        element.getAttribute('contenteditable'),
        element instanceof HTMLInputElement ? [element.type, element.checked] : null,
    ])
}

function bounds(element: HTMLElement) {
    const { x, y, width, height } = element.getBoundingClientRect()
    return { x, y, width, height }
}

function visible(element: HTMLElement) {
    const rect = element.getBoundingClientRect()
    const style = getComputedStyle(element)
    return (
        element.isConnected &&
        !element.closest('[inert],[aria-hidden=true]') &&
        style.visibility !== 'hidden' &&
        style.display !== 'none' &&
        Number(style.opacity) !== 0 &&
        element.checkVisibility({ opacityProperty: true, visibilityProperty: true }) &&
        rect.width > 0 &&
        rect.height > 0 &&
        rect.right > 0 &&
        rect.bottom > 0 &&
        rect.left < innerWidth &&
        rect.top < innerHeight &&
        hitPoint(element) !== null
    )
}

function hitPoint(element: HTMLElement) {
    const rect = element.getBoundingClientRect()
    let left = Math.max(0, rect.left)
    let top = Math.max(0, rect.top)
    let right = Math.min(innerWidth, rect.right)
    let bottom = Math.min(innerHeight, rect.bottom)
    // Fixed overlays escape ordinary ancestor clipping.
    if (getComputedStyle(element).position !== 'fixed') {
        for (let parent = element.parentElement; parent; parent = parent.parentElement) {
            const style = getComputedStyle(parent)
            const bounds = parent.getBoundingClientRect()
            if (style.overflowX !== 'visible') {
                left = Math.max(left, bounds.left)
                right = Math.min(right, bounds.right)
            }
            if (style.overflowY !== 'visible') {
                top = Math.max(top, bounds.top)
                bottom = Math.min(bottom, bounds.bottom)
            }
            if (style.position === 'fixed') break
        }
    }
    if (right <= left || bottom <= top) return null
    const x = (left + right) / 2
    const y = (top + bottom) / 2
    const hit = document.elementFromPoint(x, y)
    return hit && (hit === element || element.contains(hit)) ? { x, y } : null
}

function scrollable(element: HTMLElement, horizontal: boolean) {
    const style = getComputedStyle(element)
    return (
        /^(auto|scroll|overlay)$/.test(horizontal ? style.overflowX : style.overflowY) &&
        (horizontal
            ? element.scrollWidth > element.clientWidth
            : element.scrollHeight > element.clientHeight)
    )
}

function sensitive(element: HTMLElement) {
    return (
        element.matches('input[type=password],input[type=hidden]') ||
        !!element.closest('[data-mcp-private]')
    )
}

function label(element: HTMLElement) {
    const editable = element.matches('input,textarea,select') || element.isContentEditable
    const labels =
        element instanceof HTMLInputElement ||
        element instanceof HTMLTextAreaElement ||
        element instanceof HTMLSelectElement
            ? Array.from(element.labels ?? [])
                  .map((node) => node.textContent)
                  .join(' ')
            : ''
    return (
        element.getAttribute('aria-label') ||
        labels ||
        element.getAttribute('title') ||
        (editable ? '' : element.textContent) ||
        ''
    )
        .replace(/\s+/g, ' ')
        .trim()
        .slice(0, 160)
}

function editable(element: HTMLElement): element is HTMLInputElement | HTMLTextAreaElement {
    return (
        (element instanceof HTMLInputElement &&
            ['text', 'search', 'url', 'email', 'tel', 'number'].includes(element.type)) ||
        element instanceof HTMLTextAreaElement
    )
}

function setText(element: HTMLElement, text: string, inputType = 'insertText') {
    if (
        !element.dispatchEvent(
            new InputEvent('beforeinput', {
                bubbles: true,
                cancelable: true,
                inputType,
                data: text,
            }),
        )
    )
        throw new Error('Text edit was cancelled by the control')
    if (editable(element)) {
        const prototype =
            element instanceof HTMLTextAreaElement
                ? HTMLTextAreaElement.prototype
                : HTMLInputElement.prototype
        Object.getOwnPropertyDescriptor(prototype, 'value')?.set?.call(element, text)
    } else element.textContent = text
    element.dispatchEvent(new InputEvent('input', { bubbles: true, inputType, data: text }))
    element.dispatchEvent(new Event('change', { bubbles: true }))
}

function activate(element: HTMLElement) {
    const point = hitPoint(element)
    if (!point) throw new Error('Element is covered; observe the page again')
    const options = {
        bubbles: true,
        cancelable: true,
        clientX: point.x,
        clientY: point.y,
        button: 0,
        detail: 1,
    }
    element.dispatchEvent(
        new PointerEvent('pointerdown', {
            ...options,
            pointerId: 1,
            pointerType: 'mouse',
            isPrimary: true,
            buttons: 1,
        }),
    )
    element.dispatchEvent(new MouseEvent('mousedown', { ...options, buttons: 1 }))
    element.focus()
    element.dispatchEvent(
        new PointerEvent('pointerup', {
            ...options,
            pointerId: 1,
            pointerType: 'mouse',
            isPrimary: true,
        }),
    )
    element.dispatchEvent(new MouseEvent('mouseup', options))
    if (element.isConnected) element.dispatchEvent(new MouseEvent('click', options))
}

export function createMcpUiController(router: Router) {
    let scope = ''
    let viewId: string | undefined
    let routePath = ''
    let focusedElement: Element | null = null
    let busy = false
    let stopped = false
    let capture: AbortController | undefined
    const targets = new Map<string, Target>()
    const invalidate = () => {
        viewId = undefined
        targets.clear()
    }
    const onHuman = (event: Event) => {
        if (event.isTrusted) invalidate()
    }
    const removeRoute = router.afterEach(invalidate)
    for (const name of ['pointerdown', 'keydown', 'input'])
        document.addEventListener(name, onHuman, true)
    document.addEventListener('scroll', invalidate, true)
    window.addEventListener('resize', invalidate)

    function observe() {
        invalidate()
        viewId = crypto.randomUUID()
        routePath = router.currentRoute.value.path
        focusedElement = document.activeElement
        const all = Array.from(document.querySelectorAll<HTMLElement>(selector)).filter(
            (element) => visible(element) && !sensitive(element),
        )
        const elements = all.slice(0, 100).map((element, index) => {
            const id = `element-${index + 1}`
            const rect = bounds(element)
            targets.set(id, {
                element,
                label: label(element),
                bounds: rect,
                state: interactionState(element),
            })
            return {
                element_id: id,
                role: element.getAttribute('role') || element.tagName.toLowerCase(),
                label: label(element),
                bounds: rect,
                disabled: element.matches(':disabled,[aria-disabled=true]'),
                focused: document.activeElement === element,
                editable: editable(element) || element.isContentEditable,
                selected: element.getAttribute('aria-selected'),
                checked:
                    element.getAttribute('aria-checked') ??
                    (element instanceof HTMLInputElement &&
                    ['checkbox', 'radio'].includes(element.type)
                        ? element.checked
                        : null),
                expanded: element.getAttribute('aria-expanded'),
            }
        })
        return {
            view_id: viewId,
            route: routePath,
            route_name: router.currentRoute.value.name?.toString() ?? null,
            instance_id:
                typeof router.currentRoute.value.params.id === 'string' &&
                routePath.startsWith('/instance/')
                    ? router.currentRoute.value.params.id
                    : null,
            viewport: {
                width: innerWidth,
                height: innerHeight,
                device_pixel_ratio: devicePixelRatio,
            },
            elements,
            elements_truncated: all.length > 100,
        }
    }

    function target(id: string) {
        const saved = targets.get(id)
        if (!saved || !visible(saved.element) || sensitive(saved.element))
            throw new Error('Element is unavailable; observe the page again')
        if (label(saved.element) !== saved.label || interactionState(saved.element) !== saved.state)
            throw new Error('Element changed; observe the page again')
        const rect = bounds(saved.element)
        if (
            Object.keys(rect).some(
                (key) =>
                    Math.abs(
                        rect[key as keyof typeof rect] - saved.bounds[key as keyof typeof rect],
                    ) > 2,
            )
        )
            throw new Error('Element moved; observe the page again')
        if (!hitPoint(saved.element)) throw new Error('Element is covered; observe the page again')
        if (saved.element.matches(':disabled,[aria-disabled=true]'))
            throw new Error('Element is disabled')
        return saved.element
    }

    async function perform(action: UiAction) {
        if (action.kind === 'context') return observe()
        if (action.kind === 'screenshot') {
            const context = observe()
            capture = new AbortController()
            try {
                const result = await captureMcpUi(capture.signal)
                if (viewId !== context.view_id)
                    throw new Error('View changed during capture; retry screenshot')
                return {
                    context: { ...context, warnings: result.warnings },
                    image: result.image,
                }
            } finally {
                capture = undefined
            }
        }
        if (action.kind === 'navigate') {
            if (
                !action.path.startsWith('/') ||
                action.path.startsWith('//') ||
                /[?#\\]/.test(action.path) ||
                Array.from(action.path).some((char) => char.charCodeAt(0) < 32) ||
                action.path.startsWith('/__')
            )
                throw new Error('Use a recognized internal route path without query or hash')
            const route = router.resolve(action.path)
            if (
                !route.matched.length ||
                route.matched.some((record) => record.path.includes(':pathMatch'))
            )
                throw new Error('Unknown launcher route')
            invalidate()
            await router.push(action.path)
            await nextTick()
            return {
                route: router.currentRoute.value.path,
                reached: router.currentRoute.value.path === route.path,
            }
        }
        if (
            !('view_id' in action) ||
            action.view_id !== viewId ||
            router.currentRoute.value.path !== routePath
        )
            throw new Error('View expired; observe the page again')
        if (action.kind === 'click') {
            const element = target(action.element_id)
            invalidate()
            activate(element)
        } else if (action.kind === 'type') {
            const element = target(action.element_id)
            if (element.matches('[readonly]') || (!editable(element) && !element.isContentEditable))
                throw new Error('Element is not editable')
            invalidate()
            element.focus()
            setText(element, action.text)
        } else if (action.kind === 'scroll') {
            const horizontal = action.direction === 'left' || action.direction === 'right'
            let element: HTMLElement | null =
                action.element_id === 'viewport'
                    ? document.querySelector('.app-viewport') ||
                      (document.scrollingElement as HTMLElement)
                    : target(action.element_id)
            if (action.element_id === 'viewport' && element && !scrollable(element, horizontal)) {
                const rect = element.getBoundingClientRect()
                const center = document.elementFromPoint(
                    (Math.max(0, rect.left) + Math.min(innerWidth, rect.right)) / 2,
                    (Math.max(0, rect.top) + Math.min(innerHeight, rect.bottom)) / 2,
                )
                if (center instanceof HTMLElement && element.contains(center)) element = center
            }
            while (element && !scrollable(element, horizontal)) element = element.parentElement
            if (!element) throw new Error('No scrollable container at this element')
            invalidate()
            const delta = action.pixels * (['up', 'left'].includes(action.direction) ? -1 : 1)
            element.scrollBy({
                left: horizontal ? delta : 0,
                top: horizontal ? 0 : delta,
                behavior: 'instant',
            })
        } else if (action.kind === 'key') {
            const element = document.activeElement
            if (element !== focusedElement) throw new Error('Focus changed; observe the page again')
            if (!(element instanceof HTMLElement) || !visible(element) || sensitive(element))
                throw new Error('Focus a visible launcher control first')
            if (element.matches(':disabled,[aria-disabled=true]'))
                throw new Error('Element is disabled')
            invalidate()
            const options = {
                key: action.key,
                shiftKey: action.shift,
                ctrlKey: action.control,
                bubbles: true,
                cancelable: true,
            }
            const allowed = element.dispatchEvent(new KeyboardEvent('keydown', options))
            if (allowed && action.control && action.key === 'a' && editable(element))
                element.select()
            else if (
                allowed &&
                !action.control &&
                editable(element) &&
                !element.readOnly &&
                ['Backspace', 'Delete'].includes(action.key) &&
                element.selectionStart !== null &&
                element.selectionEnd !== null
            ) {
                const start = element.selectionStart
                const end = element.selectionEnd
                const from =
                    start === end && action.key === 'Backspace' ? Math.max(0, start - 1) : start
                const to =
                    start === end && action.key === 'Delete'
                        ? Math.min(element.value.length, end + 1)
                        : end
                setText(
                    element,
                    element.value.slice(0, from) + element.value.slice(to),
                    action.key === 'Delete' ? 'deleteContentForward' : 'deleteContentBackward',
                )
                element.setSelectionRange(from, from)
            } else if (allowed && !action.control && action.key === 'Enter') {
                if (element.matches('button,a[href],[role=button]')) activate(element)
                else if (element instanceof HTMLInputElement) element.form?.requestSubmit()
            } else if (allowed && !action.control && action.key === 'Tab') {
                const all = Array.from(document.querySelectorAll<HTMLElement>(selector)).filter(
                    (candidate) =>
                        visible(candidate) &&
                        !sensitive(candidate) &&
                        !candidate.matches(':disabled,[aria-disabled=true]') &&
                        candidate.tabIndex >= 0,
                )
                const index = all.indexOf(element)
                all[(index + (action.shift ? -1 : 1) + all.length) % all.length]?.focus()
            }
            element.dispatchEvent(new KeyboardEvent('keyup', options))
        }
        await nextTick()
        return { performed: true, route: router.currentRoute.value.path, observe_again: true }
    }

    return {
        async execute(action: UiAction, controlScope: string) {
            if (stopped) throw new Error('Launcher UI controller stopped')
            if (busy)
                throw new Error('Launcher UI is busy; retry after the current request finishes')
            busy = true
            try {
                if (scope !== controlScope) {
                    invalidate()
                    scope = controlScope
                }
                return await perform(action)
            } catch (error) {
                invalidate()
                throw error
            } finally {
                busy = false
            }
        },
        stop() {
            stopped = true
            capture?.abort(new Error('Launcher UI controller stopped'))
            invalidate()
            removeRoute()
            for (const name of ['pointerdown', 'keydown', 'input'])
                document.removeEventListener(name, onHuman, true)
            document.removeEventListener('scroll', invalidate, true)
            window.removeEventListener('resize', invalidate)
        },
    }
}
