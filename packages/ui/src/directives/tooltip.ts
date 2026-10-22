import '../styles/overlays.css'

import type { Placement } from '@floating-ui/vue'
import type { ObjectDirective } from 'vue'

import { isElementTruncated } from '../utils/truncate'
import { acquireTooltipLayer } from './tooltip-layer'
import { resolveTooltipContent, type TooltipValue } from './tooltip-value'

/**
 * The project's tooltip directive, replacing `floating-vue`'s.
 *
 * It deliberately keeps that directive's call signature: several hundred call
 * sites use `v-tooltip`, and the shapes it accepts are the ones they rely on.
 * Falsy means "no tooltip" -- see `tooltip-value.ts`, which holds that
 * coercion on its own so it can be tested without loading this file's CSS.
 *
 * `html` writes raw markup for the two call sites that pass an
 * already-sanitised fragment; everything else is written as text, so untrusted
 * strings can never become markup.
 *
 * It lives in `packages/ui` rather than the app because the website compiles
 * these components too and needs the same directive; the stylesheet travels
 * with it for the same reason.
 */

/** Matches floating-vue's tooltip theme, so the feel is unchanged. */
const SHOW_DELAY_MS = 200

const PLACEMENTS = [
    'top',
    'top-start',
    'top-end',
    'right',
    'right-start',
    'right-end',
    'bottom',
    'bottom-start',
    'bottom-end',
    'left',
    'left-start',
    'left-end',
] as const

interface TooltipHandle {
    /** Called from `updated` with the fresh value; the stored one goes stale. */
    setValue: (value: TooltipValue) => void
    destroy: () => void
}

function createTooltip(trigger: HTMLElement, modifier: Placement | null): TooltipHandle {
    const { layer, release } = acquireTooltipLayer(trigger)
    let value: TooltipValue
    let showTimer: ReturnType<typeof setTimeout> | null = null
    let hovered = false
    let focused = false
    let disposed = false
    let resizeObserver: ResizeObserver | undefined
    let mutationObserver: MutationObserver | undefined
    let observedTarget: HTMLElement | undefined
    let measureFrame: number | undefined

    function resolvedContent() {
        const resolved = resolveTooltipContent(value)
        if (
            resolved?.options.onlyWhenTruncated &&
            !isElementTruncated(resolved.options.overflowTarget ?? trigger)
        )
            return null
        return resolved
    }

    function refreshTruncation() {
        if (disposed || measureFrame !== undefined) return
        measureFrame = requestAnimationFrame(() => {
            measureFrame = undefined
            measureTruncation()
        })
    }

    function measureTruncation() {
        if (disposed || !observedTarget) return
        const next = isElementTruncated(observedTarget)
        if (
            showTimer === null &&
            hasActiveTrigger() &&
            (layer.owns(trigger) || !layer.hasOwner()) &&
            next !== layer.owns(trigger)
        )
            show()
    }

    function stopObserving() {
        if (measureFrame !== undefined) cancelAnimationFrame(measureFrame)
        measureFrame = undefined
        resizeObserver?.disconnect()
        mutationObserver?.disconnect()
        resizeObserver = undefined
        mutationObserver = undefined
        observedTarget = undefined
        document.fonts?.removeEventListener('loadingdone', refreshTruncation)
    }

    function observeTruncation() {
        const options = resolveTooltipContent(value)?.options
        const target = options?.onlyWhenTruncated ? (options.overflowTarget ?? trigger) : undefined
        if (target === observedTarget) return
        stopObserving()
        if (!target) return
        observedTarget = target
        resizeObserver = new ResizeObserver(refreshTruncation)
        resizeObserver.observe(target)
        mutationObserver = new MutationObserver(refreshTruncation)
        mutationObserver.observe(target, {
            subtree: true,
            childList: true,
            characterData: true,
            attributes: true,
            attributeFilter: ['style', 'class'],
        })
        for (let parent = target.parentElement; parent; parent = parent.parentElement) {
            mutationObserver.observe(parent, {
                attributes: true,
                attributeFilter: ['style', 'class'],
            })
        }
        document.fonts?.addEventListener('loadingdone', refreshTruncation)
        void document.fonts?.ready.then(refreshTruncation)
    }

    function triggerEnabled(name: 'hover' | 'focus') {
        const triggers = resolveTooltipContent(value)?.options.triggers
        return triggers === undefined || triggers.includes(name)
    }

    function hasActiveTrigger() {
        return (hovered && triggerEnabled('hover')) || (focused && triggerEnabled('focus'))
    }

    function cancelPendingShow() {
        if (showTimer === null) return
        clearTimeout(showTimer)
        showTimer = null
    }

    function show() {
        cancelPendingShow()
        if (disposed) return
        const resolved = resolvedContent()
        if (resolved) layer.show(trigger, resolved, modifier ?? resolved.options.placement ?? 'top')
        else layer.leave(trigger, true)
    }

    function hide() {
        cancelPendingShow()
        layer.leave(trigger)
    }

    function onEnter() {
        hovered = true
        if (!triggerEnabled('hover')) return
        cancelPendingShow()
        const request = layer.beginActivation()
        if (layer.isVisible()) show()
        else
            showTimer = setTimeout(() => {
                showTimer = null
                if (layer.isCurrentActivation(request) && hasActiveTrigger()) show()
            }, SHOW_DELAY_MS)
    }

    function onLeave() {
        hovered = false
        if (!triggerEnabled('hover')) return
        if (!hasActiveTrigger()) hide()
    }

    function onFocusIn() {
        focused = true
        if (triggerEnabled('focus')) {
            layer.beginActivation()
            show()
        }
    }

    function onFocusOut(event: FocusEvent) {
        if (event.relatedTarget instanceof Node && trigger.contains(event.relatedTarget)) return
        focused = false
        if (triggerEnabled('focus') && !hasActiveTrigger()) hide()
    }

    trigger.addEventListener('mouseenter', onEnter)
    trigger.addEventListener('mouseleave', onLeave)
    trigger.addEventListener('focusin', onFocusIn)
    trigger.addEventListener('focusout', onFocusOut)

    return {
        setValue(next) {
            value = next
            observeTruncation()
            // Only re-render when something is actually on screen, so an update to
            // a closed tooltip costs nothing.
            if (
                showTimer === null &&
                hasActiveTrigger() &&
                (layer.owns(trigger) || !layer.hasOwner())
            )
                show()
            else if (!hasActiveTrigger()) hide()
        },
        destroy() {
            disposed = true
            stopObserving()
            cancelPendingShow()
            release()
            trigger.removeEventListener('mouseenter', onEnter)
            trigger.removeEventListener('mouseleave', onLeave)
            trigger.removeEventListener('focusin', onFocusIn)
            trigger.removeEventListener('focusout', onFocusOut)
        },
    }
}

const handles = new WeakMap<HTMLElement, TooltipHandle>()

function placementModifierOf(modifiers: Record<string, boolean>): Placement | null {
    return (PLACEMENTS.find((name) => modifiers[name]) as Placement | undefined) ?? null
}

export const tooltipDirective: ObjectDirective<HTMLElement, TooltipValue> = {
    mounted(el, binding) {
        const handle = createTooltip(el, placementModifierOf(binding.modifiers))
        handles.set(el, handle)
        handle.setValue(binding.value)
    },

    updated(el, binding) {
        const handle = handles.get(el)
        if (!handle) return
        if (binding.value === binding.oldValue) return

        handle.setValue(binding.value)
    },

    unmounted(el) {
        handles.get(el)?.destroy()
        handles.delete(el)
    },
}
