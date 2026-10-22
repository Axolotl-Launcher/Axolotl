import {
    arrow,
    autoUpdate,
    computePosition,
    flip,
    offset,
    type Placement,
    shift,
} from '@floating-ui/vue'

import type { resolveTooltipContent } from './tooltip-value'

type Content = NonNullable<ReturnType<typeof resolveTooltipContent>>

const GROUP_GAP_MS = 80
const EXIT_MS = 140
let tooltipCounter = 0

/** A group owns one movable surface; each trigger still owns its input and observers. */
export function createTooltipLayer() {
    const positioner = document.createElement('div')
    positioner.className = 'tooltip-positioner'
    positioner.dataset.state = 'closed'
    const popper = document.createElement('div')
    popper.className = 'tooltip-popper tooltip-surface'
    popper.id = `tooltip-${++tooltipCounter}`
    popper.setAttribute('role', 'tooltip')
    const arrowElement = document.createElement('span')
    arrowElement.className = 'tooltip-popper-arrow'
    arrowElement.setAttribute('aria-hidden', 'true')
    positioner.append(popper)

    let owner: HTMLElement | null = null
    let stopAutoUpdate: (() => void) | undefined
    let hideTimer: ReturnType<typeof setTimeout> | undefined
    let removeTimer: ReturnType<typeof setTimeout> | undefined
    let revealFrame: number | undefined
    let generation = 0
    let activation = 0
    let positioned = false

    function clearDescription() {
        if (!owner) return
        const ids = (owner.getAttribute('aria-describedby') ?? '')
            .split(/\s+/)
            .filter((id) => id && id !== popper.id)
        if (ids.length) owner.setAttribute('aria-describedby', ids.join(' '))
        else owner.removeAttribute('aria-describedby')
    }

    function cancelExit() {
        clearTimeout(hideTimer)
        clearTimeout(removeTimer)
        hideTimer = undefined
        removeTimer = undefined
    }

    function remove() {
        cancelExit()
        if (revealFrame !== undefined) cancelAnimationFrame(revealFrame)
        revealFrame = undefined
        generation++
        stopAutoUpdate?.()
        stopAutoUpdate = undefined
        clearDescription()
        owner = null
        positioner.remove()
        positioner.dataset.state = 'closed'
        positioner.dataset.positioned = 'false'
        positioned = false
    }

    function close() {
        generation++
        if (revealFrame !== undefined) cancelAnimationFrame(revealFrame)
        revealFrame = undefined
        stopAutoUpdate?.()
        stopAutoUpdate = undefined
        clearDescription()
        owner = null
        positioner.dataset.state = 'closed'
        popper.setAttribute('aria-hidden', 'true')
        if (matchMedia('(prefers-reduced-motion: reduce)').matches) remove()
        else removeTimer = setTimeout(remove, EXIT_MS)
    }

    async function updatePosition(trigger: HTMLElement, placement: Placement) {
        if (owner !== trigger) return
        const request = ++generation
        const result = await computePosition(trigger, positioner, {
            placement,
            strategy: 'fixed',
            middleware: [
                offset(5),
                flip({ padding: 8 }),
                shift({ padding: 8 }),
                arrow({ element: arrowElement, padding: 7 }),
            ],
        })
        if (
            request !== generation ||
            owner !== trigger ||
            !trigger.isConnected ||
            !positioner.isConnected
        )
            return

        if (positioned) {
            const box = positioner.getBoundingClientRect()
            const safeX = Math.max(8, Math.min(box.left, innerWidth - box.width - 8))
            const safeY = Math.max(8, Math.min(box.top, innerHeight - box.height - 8))
            if (Math.abs(safeX - box.left) > 1 || Math.abs(safeY - box.top) > 1) {
                positioner.dataset.positioned = 'false'
                positioner.style.transform = `translate(${safeX}px, ${safeY}px)`
                positioner.getBoundingClientRect()
                positioner.dataset.positioned = 'true'
            }
        }
        positioner.style.transform = `translate(${Math.round(result.x)}px, ${Math.round(result.y)}px)`
        const side = result.placement.split('-')[0] as 'top' | 'bottom' | 'left' | 'right'
        arrowElement.dataset.side = side
        const horizontal = side === 'top' || side === 'bottom'
        arrowElement.style.left =
            horizontal && result.middlewareData.arrow?.x !== undefined
                ? `${result.middlewareData.arrow.x}px`
                : ''
        arrowElement.style.top =
            !horizontal && result.middlewareData.arrow?.y !== undefined
                ? `${result.middlewareData.arrow.y}px`
                : ''
        popper.style.transformOrigin =
            {
                top: 'center bottom',
                bottom: 'center top',
                left: 'right center',
                right: 'left center',
            }[side] ?? 'center'

        if (!positioned) {
            positioned = true
            // Flush the initial position before enabling movement and entrance transitions.
            popper.getBoundingClientRect()
            positioner.dataset.positioned = 'true'
        }
        if (revealFrame === undefined) {
            revealFrame = requestAnimationFrame(() => {
                revealFrame = undefined
                if (owner && positioner.isConnected) positioner.dataset.state = 'open'
            })
        }
    }

    return {
        beginActivation: () => ++activation,
        isCurrentActivation: (request: number) => request === activation,
        hasOwner: () => owner !== null,
        owns: (trigger: HTMLElement) => owner === trigger,
        isVisible: () => positioner.isConnected,
        show(trigger: HTMLElement, resolved: Content, placement: Placement) {
            cancelExit()
            stopAutoUpdate?.()
            clearDescription()
            owner = trigger
            popper.removeAttribute('aria-hidden')
            popper.className = resolved.options.popperClass
                ? `tooltip-popper tooltip-surface ${resolved.options.popperClass}`
                : 'tooltip-popper tooltip-surface'
            if (resolved.options.html) popper.innerHTML = resolved.text
            else popper.textContent = resolved.text
            popper.append(arrowElement)
            if (!positioner.isConnected)
                (document.getElementById('teleports') ?? document.body).append(positioner)
            const ids = new Set(
                (trigger.getAttribute('aria-describedby') ?? '').split(/\s+/).filter(Boolean),
            )
            ids.add(popper.id)
            trigger.setAttribute('aria-describedby', [...ids].join(' '))
            stopAutoUpdate = autoUpdate(
                trigger,
                positioner,
                () => void updatePosition(trigger, placement),
            )
        },
        leave(trigger: HTMLElement, immediate = false) {
            if (owner !== trigger) return
            clearTimeout(hideTimer)
            if (immediate) close()
            else hideTimer = setTimeout(close, GROUP_GAP_MS)
        },
        release(trigger: HTMLElement) {
            if (owner === trigger) remove()
        },
        destroy: remove,
    }
}

type Layer = ReturnType<typeof createTooltipLayer>
const groups = new WeakMap<Element, { layer: Layer; users: number }>()

/** Groups are DOM-local: nested toolbars and dialogs cannot borrow an outer group's surface. */
export function acquireTooltipLayer(trigger: HTMLElement) {
    const boundary = trigger.closest(
        '[data-tooltip-group], [data-tooltip-boundary], [role="dialog"]',
    )
    const group = boundary?.hasAttribute('data-tooltip-group') ? boundary : trigger
    let entry = groups.get(group)
    if (!entry) {
        entry = { layer: createTooltipLayer(), users: 0 }
        groups.set(group, entry)
    }
    entry.users++
    const acquired = entry
    return {
        layer: acquired.layer,
        release() {
            acquired.layer.release(trigger)
            if (--acquired.users === 0) {
                acquired.layer.destroy()
                groups.delete(group)
            }
        },
    }
}
