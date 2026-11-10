<template>
    <nav
        v-if="filteredLinks.length > 1"
        ref="scrollContainer"
        :role="mode === 'local' ? 'tablist' : undefined"
        class="relative flex w-fit select-none overflow-x-auto rounded-full bg-surface-3 p-1 text-sm font-bold"
        :class="{ 'shadow-xl border border-solid border-surface-4': mode === 'navigation' }"
        @click.capture="onTabClickCapture"
    >
        <template v-if="mode === 'navigation'">
            <RouterLink
                v-for="(link, index) in filteredLinks"
                v-show="link.shown ?? true"
                :key="link.href"
                :replace="replace"
                :to="query ? (link.href ? `?${query}=${link.href}` : '?') : link.href"
                class="button-animation z-[1] flex flex-row items-center gap-2 px-4 py-2 focus:rounded-full"
                :class="getSSRFallbackClasses(index)"
                @pointerdown="onTabPointerDown(index, $event)"
                @dragstart.capture="cancelPendingActivation"
                @contextmenu.capture="cancelPendingActivation"
                @mouseenter="link.onHover?.()"
                @focus="link.onHover?.()"
            >
                <component
                    :is="link.icon"
                    v-if="link.icon"
                    class="size-5"
                    :class="getIconClasses(index)"
                />
                <span class="text-nowrap" :class="getLabelClasses(index)">
                    {{ link.label }}
                </span>
            </RouterLink>
        </template>

        <template v-else>
            <!--
                A local tab carries an id derived from its href so a consumer can point
                its panel's aria-labelledby at `nav-tab-<href>`. The href is also the
                v-for key, so the id is unique without a prefix prop to thread through.
            -->
            <button
                v-for="(link, index) in filteredLinks"
                v-show="link.shown ?? true"
                :id="link.href ? `nav-tab-${link.href}` : undefined"
                :key="link.href"
                type="button"
                role="tab"
                :aria-selected="index === currentActiveIndex"
                class="button-animation z-[1] flex flex-row items-center gap-2 border-0 bg-transparent px-4 py-2 text-inherit hover:cursor-pointer focus:rounded-full"
                :class="getSSRFallbackClasses(index)"
                @click="onLocalTabClick(index, link)"
            >
                <component
                    :is="link.icon"
                    v-if="link.icon"
                    class="size-5"
                    :class="getIconClasses(index)"
                />
                <span class="text-nowrap" :class="getLabelClasses(index)">{{ link.label }}</span>
            </button>
        </template>

        <!-- Animated slider background -->
        <div
            v-if="sliderReady && currentActiveIndex !== -1"
            ref="slider"
            class="navtabs-slider pointer-events-none absolute h-[calc(100%-0.5rem)] overflow-hidden rounded-full p-1"
            :class="[
                subpageSelected ? 'bg-surface-4' : 'bg-button-bgSelected',
                { 'navtabs-transition': transitionsEnabled },
            ]"
            :style="sliderStyle"
            aria-hidden="true"
            @transitionend="onSliderTransitionEnd"
            @transitioncancel="finishSliderAnimation"
        />
    </nav>
</template>

<script setup lang="ts">
import type { Component } from 'vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'

import { isModifierKeyDown } from '../../utils/events'
import { navigateSettled } from '../../utils/navigation'

const route = useRoute()
const router = useRouter()

interface Tab {
    label: string
    href: string
    shown?: boolean
    icon?: Component
    subpages?: string[]
    onHover?: () => void
}

interface Selection {
    index: number
    href: string | null
    isSubpage: boolean
}

interface OptimisticActivation {
    token: number
    href: string
    index: number
    source: 'pointer' | 'click'
}

const props = withDefaults(
    defineProps<{
        replace?: boolean
        links: Tab[]
        query?: string
        mode?: 'navigation' | 'local'
        activeIndex?: number
        /**
         * When true, the active tab follows the router after mount (back/forward,
         * redirects, programmatic pushes). When false (default), the route only
         * seeds the initial selection; later route changes do not move the pill.
         * Ignored in local mode, where `activeIndex` controls selection.
         */
        routeSync?: boolean
    }>(),
    {
        mode: 'navigation',
        query: undefined,
        activeIndex: undefined,
        routeSync: false,
    },
)

const emit = defineEmits<{
    tabClick: [index: number, tab: Tab]
}>()

// DOM refs
const scrollContainer = ref<HTMLElement | null>(null)
const slider = ref<HTMLElement | null>(null)
let containerResizeObserver: ResizeObserver | null = null

// Slider position state
const sliderLeft = ref(4)
const sliderTop = ref(4)
const sliderRight = ref(4)
const sliderBottom = ref(4)

// Selection state
const activeHref = ref<string | null>(null)
const subpageSelected = ref(false)
const confirmedHref = ref<string | null>(null)
const confirmedSubpage = ref(false)

// Animation state
const sliderReady = ref(false)
const transitionsEnabled = ref(false)
const isAnimating = ref(false)
const resizePending = ref(false)

// Stagger delays for the trailing edges of the slider animation
const sliderDelays = ref({ left: '0ms', top: '0ms', right: '0ms', bottom: '0ms' })

let activationToken = 0
let optimisticActivation: OptimisticActivation | null = null
let pointerCleanup: (() => void) | null = null
let clickSafetyTimer: number | null = null
let animationWatchdog: number | null = null
let snapRestoreFrame: number | null = null
let disposed = false
const animatedEdges = new Set<string>()

const filteredLinks = computed(() => props.links.filter((link) => link.shown ?? true))
const currentActiveIndex = computed(() => {
    if (activeHref.value === null) return -1
    return filteredLinks.value.findIndex((link) => link.href === activeHref.value)
})

const sliderStyle = computed(() => ({
    left: `${sliderLeft.value}px`,
    top: `${sliderTop.value}px`,
    right: `${sliderRight.value}px`,
    bottom: `${sliderBottom.value}px`,
    opacity: sliderReady.value && currentActiveIndex.value !== -1 ? 1 : 0,
}))

const leftDelay = computed(() => sliderDelays.value.left)
const rightDelay = computed(() => sliderDelays.value.right)
const topDelay = computed(() => sliderDelays.value.top)
const bottomDelay = computed(() => sliderDelays.value.bottom)

const isActiveAndNotSubpage = computed(
    () => (index: number) => currentActiveIndex.value === index && !subpageSelected.value,
)

function getSSRFallbackClasses(index: number) {
    if (sliderReady.value) return {}
    if (currentActiveIndex.value !== index) return {}

    return {
        'rounded-full': true,
        'bg-button-bgSelected': !subpageSelected.value,
        'bg-surface-4': subpageSelected.value,
    }
}

function getIconClasses(index: number) {
    return {
        'text-button-textSelected': isActiveAndNotSubpage.value(index),
        'text-[var(--color-text-tertiary)]': !isActiveAndNotSubpage.value(index),
    }
}

function getLabelClasses(index: number) {
    return {
        'text-button-textSelected': isActiveAndNotSubpage.value(index),
        'text-[var(--color-text-primary)]': !isActiveAndNotSubpage.value(index),
    }
}

function findSelection(
    location: { path: string; query: Record<string, unknown> },
    links: Tab[],
    queryProp?: string,
): Selection {
    for (let i = links.length - 1; i >= 0; i--) {
        const link = links[i]
        const decodedPath = decodeURIComponent(location.path)
        const decodedHref = decodeURIComponent(link.href.split('?')[0])

        if (queryProp) {
            const queryValue = location.query[queryProp]
            if (queryValue === link.href || (!queryValue && !link.href)) {
                return { index: i, href: link.href, isSubpage: false }
            }
            continue
        }

        if (decodedPath === decodedHref) {
            return { index: i, href: link.href, isSubpage: false }
        }

        const isSubpageMatch =
            (decodedPath.startsWith(decodedHref) &&
                (decodedPath.length === decodedHref.length ||
                    decodedPath[decodedHref.length] === '/')) ||
            link.subpages?.some((subpage) => decodedPath.includes(subpage))

        if (isSubpageMatch) {
            return { index: i, href: link.href, isSubpage: true }
        }
    }

    return { index: -1, href: null, isSubpage: false }
}

function selectionForIndex(index: number, isSubpage = false): Selection {
    const link = filteredLinks.value[index]
    if (!link) return { index: -1, href: null, isSubpage: false }
    return { index, href: link.href, isSubpage }
}

function clampLocalIndex(index: number): number {
    return Math.max(-1, Math.min(index, filteredLinks.value.length - 1))
}

function getTabElement(index: number): HTMLElement | null {
    if (index === -1) return null

    const container = scrollContainer.value
    if (!container) return null

    const tabs = container.querySelectorAll<HTMLElement>(':scope > .button-animation')
    return tabs[index] ?? null
}

function measureTab(index: number) {
    const el = getTabElement(index)
    if (!el?.offsetParent) return null
    const parent = el.offsetParent as HTMLElement
    return {
        left: el.offsetLeft,
        top: el.offsetTop,
        right: parent.offsetWidth - el.offsetLeft - el.offsetWidth,
        bottom: parent.offsetHeight - el.offsetTop - el.offsetHeight,
    }
}

function positionsEqual(position: {
    left: number
    top: number
    right: number
    bottom: number
}): boolean {
    return (
        position.left === sliderLeft.value &&
        position.top === sliderTop.value &&
        position.right === sliderRight.value &&
        position.bottom === sliderBottom.value
    )
}

function clearAnimationWatchdog() {
    if (animationWatchdog !== null) {
        window.clearTimeout(animationWatchdog)
        animationWatchdog = null
    }
}

function finishSliderAnimation() {
    if (!isAnimating.value && !resizePending.value) return
    isAnimating.value = false
    animatedEdges.clear()
    clearAnimationWatchdog()
    if (resizePending.value) {
        resizePending.value = false
        reconcileSliderAfterLayout()
    }
}

function armAnimationWatchdog() {
    clearAnimationWatchdog()
    animationWatchdog = window.setTimeout(() => {
        animationWatchdog = null
        finishSliderAnimation()
    }, 500)
}

function animateSliderTo(position: { left: number; top: number; right: number; bottom: number }) {
    if (positionsEqual(position)) {
        finishSliderAnimation()
        return
    }

    const STAGGER_DELAY = '200ms'
    sliderDelays.value = {
        left: position.left < sliderLeft.value ? '0ms' : STAGGER_DELAY,
        right: position.left < sliderLeft.value ? STAGGER_DELAY : '0ms',
        top: position.top < sliderTop.value ? '0ms' : STAGGER_DELAY,
        bottom: position.top < sliderTop.value ? STAGGER_DELAY : '0ms',
    }
    sliderLeft.value = position.left
    sliderRight.value = position.right
    sliderTop.value = position.top
    sliderBottom.value = position.bottom
    sliderReady.value = true
    isAnimating.value = true
    animatedEdges.clear()
    armAnimationWatchdog()
}

function snapSliderTo(position: { left: number; top: number; right: number; bottom: number }) {
    finishSliderAnimation()
    if (snapRestoreFrame !== null) {
        cancelAnimationFrame(snapRestoreFrame)
        snapRestoreFrame = null
    }

    transitionsEnabled.value = false
    sliderDelays.value = { left: '0ms', top: '0ms', right: '0ms', bottom: '0ms' }
    sliderLeft.value = position.left
    sliderTop.value = position.top
    sliderRight.value = position.right
    sliderBottom.value = position.bottom
    sliderReady.value = true
    snapRestoreFrame = requestAnimationFrame(() => {
        snapRestoreFrame = null
        transitionsEnabled.value = true
    })
}

function applySelection(selection: Selection, animate: boolean) {
    const wasHidden = currentActiveIndex.value === -1 || !sliderReady.value
    activeHref.value = selection.href
    subpageSelected.value = selection.isSubpage

    if (selection.index === -1) {
        return
    }

    const position = measureTab(selection.index)
    if (!position) return

    if (!animate || wasHidden) {
        snapSliderTo(position)
    } else {
        animateSliderTo(position)
    }
}

function reconcileSliderAfterLayout() {
    if (isAnimating.value) {
        resizePending.value = true
        return
    }

    requestAnimationFrame(() => {
        if (disposed || isAnimating.value) {
            if (!disposed) resizePending.value = true
            return
        }
        const position = measureTab(currentActiveIndex.value)
        if (!position || positionsEqual(position)) return
        snapSliderTo(position)
    })
}

function onSliderTransitionEnd(event: TransitionEvent) {
    if (!isAnimating.value || event.target !== slider.value) return
    if (['left', 'top', 'right', 'bottom'].includes(event.propertyName)) {
        animatedEdges.add(event.propertyName)
    }
    if (animatedEdges.size >= 4) finishSliderAnimation()
}

function resolveTabTarget(link: Tab): string {
    return props.query ? (link.href ? `?${props.query}=${link.href}` : '?') : link.href
}

function canInterceptEvent(event: MouseEvent, anchor: HTMLAnchorElement | null) {
    if (event.button !== 0 || isModifierKeyDown(event)) return false
    if (event.defaultPrevented || !anchor) return false
    if (anchor.target === '_blank' || anchor.hasAttribute('download')) return false
    return true
}

function resolveAnchor(target: EventTarget | null): HTMLAnchorElement | null {
    if (!(target instanceof Element)) return null
    const anchor = target.closest<HTMLAnchorElement>('a[href]')
    if (!anchor || !scrollContainer.value?.contains(anchor)) return null
    const tabs = scrollContainer.value.querySelectorAll<HTMLAnchorElement>(
        ':scope > a.button-animation',
    )
    return Array.from(tabs).includes(anchor) ? anchor : null
}

function anchorIndex(anchor: HTMLAnchorElement): number {
    const tabs = scrollContainer.value?.querySelectorAll<HTMLAnchorElement>(
        ':scope > a.button-animation',
    )
    return tabs ? Array.from(tabs).indexOf(anchor) : -1
}

function clearPointerListeners(clearTimer = true) {
    pointerCleanup?.()
    pointerCleanup = null
    if (clearTimer && clickSafetyTimer !== null) {
        window.clearTimeout(clickSafetyTimer)
        clickSafetyTimer = null
    }
}

function clearActivation() {
    clearPointerListeners()
    optimisticActivation = null
}

function restoreSelection(): Selection {
    if (props.mode === 'local') {
        if (props.activeIndex !== undefined) {
            return selectionForIndex(clampLocalIndex(props.activeIndex))
        }
        return confirmedHref.value
            ? selectionForIndex(
                  filteredLinks.value.findIndex((link) => link.href === confirmedHref.value),
                  false,
              )
            : { index: -1, href: null, isSubpage: false }
    }

    return props.routeSync
        ? findSelection(route, filteredLinks.value, props.query)
        : {
              index: filteredLinks.value.findIndex((link) => link.href === confirmedHref.value),
              href: confirmedHref.value,
              isSubpage: confirmedSubpage.value,
          }
}

function rollbackActivation(token: number) {
    if (!optimisticActivation || optimisticActivation.token !== token) return
    clearActivation()
    const selection = restoreSelection()
    applySelection(selection, true)
}

function confirmActivation(token: number, selection: Selection) {
    if (!optimisticActivation || optimisticActivation.token !== token) return
    clearActivation()
    confirmedHref.value = selection.href
    confirmedSubpage.value = selection.isSubpage
    activeHref.value = selection.href
    subpageSelected.value = selection.isSubpage
    void nextTick(() => reconcileSliderAfterLayout())
}

function activateOptimistically(index: number, source: OptimisticActivation['source']) {
    const link = filteredLinks.value[index]
    if (!link) return

    clearPointerListeners()
    const token = ++activationToken
    optimisticActivation = { token, href: link.href, index, source }

    const wasHidden = currentActiveIndex.value === -1 || !sliderReady.value
    activeHref.value = link.href
    subpageSelected.value = false

    const position = measureTab(index)
    if (!position) return

    if (wasHidden) {
        snapSliderTo(position)
    } else {
        animateSliderTo(position)
    }
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
            if (optimisticActivation?.token !== token) {
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

function onTabPointerDown(index: number, event: PointerEvent) {
    if (props.mode !== 'navigation') return
    if (event.button !== 0 || !event.isPrimary) return
    if (event.pointerType !== 'mouse') return
    if (isModifierKeyDown(event) || event.defaultPrevented) return

    const anchor = event.currentTarget instanceof HTMLAnchorElement ? event.currentTarget : null
    if (!anchor || anchor.target === '_blank' || anchor.hasAttribute('download')) return

    activateOptimistically(index, 'pointer')
    if (optimisticActivation) armPointerRelease(optimisticActivation.token, anchor)
}

function cancelPendingActivation() {
    if (optimisticActivation) rollbackActivation(optimisticActivation.token)
}

async function settleNavigation(token: number, target: string) {
    const outcome = await navigateSettled(router, target, props.replace)
    if (disposed || optimisticActivation?.token !== token) return

    if (outcome.ok) {
        const selection = findSelection(route, filteredLinks.value, props.query)
        if (selection.href === optimisticActivation.href) {
            confirmActivation(token, selection)
            return
        }

        clearActivation()
        confirmedHref.value = selection.href
        confirmedSubpage.value = selection.isSubpage
        applySelection(selection, true)
        return
    }

    rollbackActivation(token)
}

function onTabClickCapture(event: MouseEvent) {
    if (props.mode !== 'navigation') return
    const anchor = resolveAnchor(event.target)
    if (!canInterceptEvent(event, anchor)) return

    const index = anchorIndex(anchor!)
    if (index === -1) return
    const link = filteredLinks.value[index]
    if (!link) return

    event.preventDefault()
    clearPointerListeners()

    if (optimisticActivation?.href !== link.href) {
        activateOptimistically(index, 'click')
    }

    const token = optimisticActivation?.token
    if (token === undefined) return
    void settleNavigation(token, resolveTabTarget(link))
}

function onLocalTabClick(index: number, link: Tab) {
    if (props.activeIndex === undefined) {
        activeHref.value = link.href
        subpageSelected.value = false
        confirmedHref.value = link.href
        confirmedSubpage.value = false
        const position = measureTab(index)
        if (position) {
            if (!sliderReady.value) snapSliderTo(position)
            else animateSliderTo(position)
        }
    }
    emit('tabClick', index, link)
}

function onRouteChanged() {
    if (props.mode !== 'navigation') return

    const selection = findSelection(route, filteredLinks.value, props.query)

    if (optimisticActivation) {
        if (selection.href === optimisticActivation.href) {
            confirmActivation(optimisticActivation.token, selection)
        } else {
            clearActivation()
            confirmedHref.value = selection.href
            confirmedSubpage.value = selection.isSubpage
            applySelection(selection, true)
        }
        return
    }

    if (!props.routeSync) return

    confirmedHref.value = selection.href
    confirmedSubpage.value = selection.isSubpage
    applySelection(selection, true)
}

function onActiveIndexChanged() {
    if (props.mode !== 'local' || props.activeIndex === undefined) return
    const index = clampLocalIndex(props.activeIndex)
    const selection = selectionForIndex(index)
    confirmedHref.value = selection.href
    confirmedSubpage.value = selection.isSubpage
    applySelection(selection, true)
}

function linksFingerprint() {
    return props.links
        .map(
            (link) =>
                `${link.href}|${link.shown ?? true}|${link.label}|${link.icon ? 1 : 0}|${
                    link.subpages?.join(',') ?? ''
                }`,
        )
        .join('\n')
}

async function onLinksChanged() {
    await nextTick()

    if (props.mode === 'navigation') {
        const currentHref = activeHref.value
        const currentVisible =
            currentHref !== null && filteredLinks.value.some((link) => link.href === currentHref)

        if (!currentVisible) {
            if (props.routeSync) {
                const selection = findSelection(route, filteredLinks.value, props.query)
                confirmedHref.value = selection.href
                confirmedSubpage.value = selection.isSubpage
                applySelection(selection, false)
            } else if (currentHref !== null) {
                activeHref.value = null
                subpageSelected.value = false
                confirmedHref.value = null
                confirmedSubpage.value = false
            }
            return
        }

        // A route can point at a tab that only becomes visible once data loads
        // (e.g. the gallery tab). Re-resolve it when the link set settles.
        if (props.routeSync && !optimisticActivation) {
            const selection = findSelection(route, filteredLinks.value, props.query)
            if (selection.href !== currentHref) {
                confirmedHref.value = selection.href
                confirmedSubpage.value = selection.isSubpage
                applySelection(selection, false)
                return
            }
        }
    }

    reconcileSliderAfterLayout()
}

const initialSelection: Selection =
    props.mode === 'navigation'
        ? findSelection(route, filteredLinks.value, props.query)
        : props.activeIndex !== undefined
          ? selectionForIndex(clampLocalIndex(props.activeIndex))
          : { index: -1, href: null, isSubpage: false }

activeHref.value = initialSelection.href
subpageSelected.value = initialSelection.isSubpage
confirmedHref.value = initialSelection.href
confirmedSubpage.value = initialSelection.isSubpage

onMounted(async () => {
    await nextTick()
    if (disposed) return
    if (currentActiveIndex.value !== -1) {
        const position = measureTab(currentActiveIndex.value)
        if (position) snapSliderTo(position)
    }
    requestAnimationFrame(() => {
        if (!disposed) transitionsEnabled.value = true
    })
})

watch(
    scrollContainer,
    (el) => {
        containerResizeObserver?.disconnect()
        containerResizeObserver = null
        if (!el) return
        containerResizeObserver = new ResizeObserver(() => reconcileSliderAfterLayout())
        containerResizeObserver.observe(el)
    },
    { immediate: true },
)

onBeforeUnmount(() => {
    disposed = true
    clearActivation()
    finishSliderAnimation()
    if (snapRestoreFrame !== null) {
        cancelAnimationFrame(snapRestoreFrame)
        snapRestoreFrame = null
    }
    containerResizeObserver?.disconnect()
    containerResizeObserver = null
})

watch(
    () => [route.path, route.query],
    () => onRouteChanged(),
)

watch(
    () => props.activeIndex,
    () => onActiveIndexChanged(),
)

watch(
    () => linksFingerprint(),
    () => {
        void onLinksChanged()
    },
)
</script>

<style scoped>
.navtabs-transition {
    transition:
        left 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(leftDelay),
        right 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(rightDelay),
        top 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(topDelay),
        bottom 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(bottomDelay),
        opacity 250ms cubic-bezier(0.5, 0, 0.2, 1) 50ms;
}
</style>
