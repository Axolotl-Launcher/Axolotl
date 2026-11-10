<template>
    <div
        ref="rail"
        data-tooltip-group="navigation"
        class="nav-rail relative flex flex-col gap-[0.5rem]"
        @pointerdown="onRailPointerDown"
        @click.capture="onRailClickCapture"
        @dragstart.capture="cancelPendingActivation"
        @contextmenu.capture="cancelPendingActivation"
    >
        <slot />
        <div
            class="nav-rail-slider pointer-events-none absolute rounded-full"
            :class="[
                subpageSelected ? 'bg-surface-4' : 'bg-button-bgSelected',
                transitionsEnabled ? 'nav-rail-slider-transition' : '',
            ]"
            :style="sliderStyle"
            aria-hidden="true"
        />
    </div>
</template>

<script setup lang="ts">
import { isModifierKeyDown, navigateSettled } from '@modrinth/ui'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { provideNavRailOptimisticSelection } from './nav-rail-selection'

const route = useRoute()
const router = useRouter()
const selection = provideNavRailOptimisticSelection()
const optimisticHref = selection.href

const rail = ref<HTMLElement | null>(null)

const hasActive = ref(false)
const subpageSelected = ref(false)
const sliderReady = ref(false)
const transitionsEnabled = ref(false)

const top = ref(0)
const bottom = ref(0)
const left = ref(0)
const width = ref(0)

const topDelay = ref('0ms')
const bottomDelay = ref('0ms')

const STAGGER_DELAY = '120ms'

interface ItemGeometry {
    top: number
    height: number
    left: number
    width: number
}

let optimisticElement: HTMLAnchorElement | null = null
let activationToken = 0
let pointerCleanup: (() => void) | null = null
let clickSafetyTimer: number | null = null
let disposed = false

let geometryCache = new WeakMap<HTMLElement, ItemGeometry>()
let railHeight = 0
let railSize = ''
let pendingPosition: { animate: boolean; target: HTMLElement | null } | null = null
let resizeObserver: ResizeObserver | null = null
let itemObserver: MutationObserver | null = null

const sliderStyle = computed(() => ({
    top: `${top.value}px`,
    bottom: `${bottom.value}px`,
    left: `${left.value}px`,
    width: `${width.value}px`,
    opacity: sliderReady.value && hasActive.value ? 1 : 0,
}))

function prepareGeometry(container: HTMLElement) {
    railHeight = container.offsetHeight
    railSize = railHeight === 0 ? '' : `${container.offsetWidth}x${railHeight}`
    geometryCache = new WeakMap()

    for (const child of container.children) {
        if (!(child instanceof HTMLElement) || child.classList.contains('nav-rail-slider')) continue
        geometryCache.set(child, {
            top: child.offsetTop,
            height: child.offsetHeight,
            left: child.offsetLeft,
            width: child.offsetWidth,
        })
    }
}

function measure(el: HTMLElement, container: HTMLElement) {
    if (!railSize) prepareGeometry(container)
    if (!railSize) return null

    let geometry = geometryCache.get(el)
    if (!geometry) {
        if (!el.offsetParent) return null
        geometry = {
            top: el.offsetTop,
            height: el.offsetHeight,
            left: el.offsetLeft,
            width: el.offsetWidth,
        }
        geometryCache.set(el, geometry)
    }

    return {
        top: geometry.top,
        bottom: railHeight - geometry.top - geometry.height,
        left: geometry.left,
        width: geometry.width,
        subpage: el.classList.contains('subpage-active'),
    }
}

function applyPosition(
    position: NonNullable<ReturnType<typeof measure>>,
    animate: boolean,
    optimistic: boolean,
) {
    subpageSelected.value = optimistic ? false : position.subpage

    const movingDown = position.top > top.value
    topDelay.value = movingDown ? STAGGER_DELAY : '0ms'
    bottomDelay.value = movingDown ? '0ms' : STAGGER_DELAY

    if (!animate || !sliderReady.value) {
        transitionsEnabled.value = false
        top.value = position.top
        bottom.value = position.bottom
        left.value = position.left
        width.value = position.width
        sliderReady.value = true
        requestAnimationFrame(() => {
            transitionsEnabled.value = true
        })
    } else {
        top.value = position.top
        bottom.value = position.bottom
        left.value = position.left
        width.value = position.width
    }

    hasActive.value = true
}

function positionSlider(animate: boolean, target?: HTMLElement | null) {
    const container = rail.value
    if (!container) return

    const el =
        target ??
        optimisticElement ??
        container.querySelector<HTMLElement>('.router-link-active, .subpage-active')
    if (!el) {
        hasActive.value = false
        return
    }

    const position = measure(el, container)
    if (!position) {
        hasActive.value = false
        return
    }

    applyPosition(position, animate, target != null || optimisticElement != null)
}

function flushPosition() {
    const pending = pendingPosition
    pendingPosition = null
    if (!pending || disposed) return
    positionSlider(pending.animate, pending.target)
}

function requestPosition(animate: boolean, target: HTMLElement | null = null) {
    if (pendingPosition) {
        pendingPosition.animate ||= animate
        // A no-target refresh must not discard an optimistic target queued by
        // pointerdown/click before this tick flushes.
        if (target) pendingPosition.target = target
        return
    }
    pendingPosition = { animate, target }
    void nextTick(flushPosition)
}

function updateSlider() {
    requestPosition(true)
}

function isPlainLeftMouse(event: MouseEvent) {
    return event.button === 0 && !isModifierKeyDown(event) && !event.defaultPrevented
}

function resolveAnchor(target: EventTarget | null): HTMLAnchorElement | null {
    if (!(target instanceof Element)) return null
    const anchor = target.closest<HTMLAnchorElement>('a[href]')
    if (!anchor || !rail.value?.contains(anchor)) return null
    if (anchor.target === '_blank' || anchor.hasAttribute('download')) return null
    if (anchor.getAttribute('aria-disabled') === 'true') return null

    const href = anchor.getAttribute('href')
    if (!href || !href.startsWith('/')) return null

    return anchor
}

function clearPointerListeners(clearTimer = true) {
    pointerCleanup?.()
    pointerCleanup = null
    if (clearTimer && clickSafetyTimer !== null) {
        window.clearTimeout(clickSafetyTimer)
        clickSafetyTimer = null
    }
}

function clearOptimistic() {
    optimisticHref.value = null
    optimisticElement = null
}

function rollbackActivation(token: number) {
    if (token !== activationToken) return
    clearPointerListeners()
    clearOptimistic()
    void updateSlider()
}

function armPointerRelease(token: number, anchor: HTMLAnchorElement) {
    const controller = new AbortController()
    const cleanup = () => {
        controller.abort()
        if (pointerCleanup === cleanup) pointerCleanup = null
    }
    pointerCleanup = cleanup

    window.addEventListener(
        'pointerup',
        (event) => {
            if (token !== activationToken) {
                cleanup()
                return
            }
            if (event.target instanceof Node && anchor.contains(event.target)) {
                if (clickSafetyTimer !== null) window.clearTimeout(clickSafetyTimer)
                clickSafetyTimer = window.setTimeout(() => {
                    clickSafetyTimer = null
                    rollbackActivation(token)
                    cleanup()
                }, 700)
                return
            }
            rollbackActivation(token)
            cleanup()
        },
        { capture: true, signal: controller.signal },
    )
    window.addEventListener(
        'pointercancel',
        () => {
            rollbackActivation(token)
            cleanup()
        },
        { signal: controller.signal },
    )
    window.addEventListener(
        'dragstart',
        () => {
            rollbackActivation(token)
            cleanup()
        },
        { capture: true, signal: controller.signal },
    )
    window.addEventListener(
        'contextmenu',
        () => {
            rollbackActivation(token)
            cleanup()
        },
        { capture: true, signal: controller.signal },
    )
}

function onRailPointerDown(event: PointerEvent) {
    if (event.button !== 0 || !event.isPrimary || event.pointerType !== 'mouse') return
    if (isModifierKeyDown(event) || event.defaultPrevented) return

    const anchor = resolveAnchor(event.target)
    if (!anchor) return

    const href = anchor.getAttribute('href')
    if (!href) return

    const token = ++activationToken
    clearPointerListeners()
    optimisticHref.value = router.resolve(href).path
    optimisticElement = anchor
    requestPosition(true, anchor)
    armPointerRelease(token, anchor)
}

async function onRailClickCapture(event: MouseEvent) {
    if (!isPlainLeftMouse(event)) return

    const anchor = resolveAnchor(event.target)
    if (!anchor) return

    const href = anchor.getAttribute('href')
    if (!href) return

    event.preventDefault()
    clearPointerListeners()

    if (optimisticElement !== anchor) {
        activationToken += 1
        optimisticHref.value = router.resolve(href).path
        optimisticElement = anchor
        requestPosition(true, anchor)
    }

    const token = activationToken
    await navigateSettled(router, href)

    if (disposed || token !== activationToken) return

    clearOptimistic()
    void updateSlider()
}

function cancelPendingActivation() {
    rollbackActivation(activationToken)
}

function invalidateGeometry() {
    railSize = ''
    void updateSlider()
}

function onRailResize() {
    const container = rail.value
    if (!container) return

    const size = `${container.offsetWidth}x${container.offsetHeight}`
    if (size === railSize) return

    invalidateGeometry()
}

onMounted(() => {
    if (rail.value) {
        prepareGeometry(rail.value)
        resizeObserver = new ResizeObserver(onRailResize)
        resizeObserver.observe(rail.value)
        // A child-list change that keeps the rail's box size unchanged would
        // not fire the resize observer, but it still moves item offsets.
        itemObserver = new MutationObserver(invalidateGeometry)
        itemObserver.observe(rail.value, { childList: true })
    }
    void updateSlider()
})

onBeforeUnmount(() => {
    disposed = true
    pendingPosition = null
    resizeObserver?.disconnect()
    resizeObserver = null
    itemObserver?.disconnect()
    itemObserver = null
    clearPointerListeners()
    clearOptimistic()
})

watch(
    () => [route.path, route.query],
    () => {
        // While a click is pending, the router still reports the previous
        // location; measuring it would yank the pill back to the old item.
        if (optimisticHref.value !== null) return
        void updateSlider()
    },
)
</script>

<style scoped>
.nav-rail :deep(a),
.nav-rail :deep(button) {
    position: relative;
    z-index: 1;
}

.nav-rail :deep(a.router-link-active),
.nav-rail :deep(a.subpage-active),
.nav-rail :deep(button.router-link-active),
.nav-rail :deep(button.subpage-active) {
    background-color: transparent;
}

.nav-rail-slider {
    z-index: 0;
}

.nav-rail-slider-transition {
    transition:
        top 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(topDelay),
        bottom 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(bottomDelay),
        left 150ms cubic-bezier(0.4, 0, 0.2, 1),
        width 150ms cubic-bezier(0.4, 0, 0.2, 1),
        opacity 250ms cubic-bezier(0.5, 0, 0.2, 1) 50ms;
}
</style>
