<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

import { useModalStack } from '../../composables/modal-stack'
import { injectPageContext } from '../../providers'

const visibleFloatingActionBars = new Map<symbol, number>()

function updateFloatingActionBarDocumentState() {
    if (typeof document === 'undefined') return

    document.body.classList.toggle('floating-action-bar-shown', visibleFloatingActionBars.size > 0)
    const clearance = Math.max(0, ...visibleFloatingActionBars.values())
    if (clearance > 0) {
        document.documentElement.style.setProperty(
            '--floating-action-bar-clearance',
            `${clearance}px`,
        )
    } else {
        document.documentElement.style.removeProperty('--floating-action-bar-clearance')
    }
}

const props = defineProps<{
    shown: boolean
    ariaLabel?: string
    hideWhenModalOpen?: boolean
    position?: 'bottom' | 'top'
    allowOverflow?: boolean
}>()

const INTERCOM_BUBBLE_GAP = 8

const barEl = ref<HTMLElement | null>(null)
const toolbarEl = ref<HTMLElement | null>(null)
const compact = ref(false)

const { stackCount } = useModalStack()
const pageContext = injectPageContext(null)
const shown = computed(() => props.shown && (!props.hideWhenModalOpen || stackCount.value === 0))
const floatingActionBarId = Symbol('floating-action-bar')
const intercomBubbleClearanceRequestId = Symbol('floating-action-bar')
const zIndex = computed(() => 100 + stackCount.value * 10 + 7)
const leftOffset = computed(
    () => pageContext?.floatingActionBarOffsets?.left.value ?? 'var(--left-bar-width, 0px)',
)
const scrollbarWidth = ref(0)

const rightOffset = computed(() => {
    const base = pageContext?.floatingActionBarOffsets?.right.value ?? 'var(--right-bar-width, 0px)'
    if (stackCount.value > 0) {
        return `calc(${base} + ${scrollbarWidth.value}px)`
    }
    return base
})
const barStyle = computed(() => ({
    zIndex: zIndex.value,
    '--floating-action-bar-left-offset': leftOffset.value,
    '--floating-action-bar-right-offset': rightOffset.value,
}))

const barClasses = computed(() => ({
    'bottom-0': !props.position || props.position === 'bottom',
    'top-12': props.position === 'top',
}))

function checkCompact() {
    const el = toolbarEl.value
    if (!el) return

    const clone = el.cloneNode(true) as HTMLElement
    clone.classList.remove('bar-compact')
    clone.style.position = 'absolute'
    clone.style.visibility = 'hidden'
    clone.style.pointerEvents = 'none'
    clone.style.width = `${el.offsetWidth}px`

    el.parentElement!.appendChild(clone)
    const needsCompact = clone.offsetHeight > 70
    clone.remove()

    compact.value = needsCompact
}

function clearIntercomBubbleClearance() {
    pageContext?.intercomBubble?.requestVerticalClearance(intercomBubbleClearanceRequestId, null)
}

function updateIntercomBubbleClearance() {
    const intercomBubble = pageContext?.intercomBubble
    if (!intercomBubble) return

    if (
        typeof window === 'undefined' ||
        !shown.value ||
        stackCount.value > 0 ||
        !barEl.value ||
        !toolbarEl.value
    ) {
        clearIntercomBubbleClearance()
        return
    }

    const barRect = barEl.value.getBoundingClientRect()
    const toolbarRight = barRect.left + toolbarEl.value.offsetLeft + toolbarEl.value.offsetWidth
    const bubbleLeft =
        window.innerWidth - intercomBubble.horizontalPadding.value - intercomBubble.width.value

    if (toolbarRight + INTERCOM_BUBBLE_GAP <= bubbleLeft) {
        clearIntercomBubbleClearance()
        return
    }

    const barStyle = window.getComputedStyle(barEl.value)
    const bottomOffset = Number.parseFloat(barStyle.bottom) || 0
    intercomBubble.requestVerticalClearance(
        intercomBubbleClearanceRequestId,
        Math.ceil(bottomOffset + barEl.value.offsetHeight + INTERCOM_BUBBLE_GAP),
    )
}

function getBottomClearance() {
    if (
        typeof window === 'undefined' ||
        (props.position !== undefined && props.position !== 'bottom')
    ) {
        return 0
    }

    // Reserve the bar's full rendered height, including its padding, so floating
    // controls clear it even when the centered toolbar reaches the right edge.
    return barEl.value?.offsetHeight || 44
}

function updateFloatingActionBarState(isShown = shown.value) {
    if (isShown) {
        visibleFloatingActionBars.set(floatingActionBarId, getBottomClearance())
    } else {
        visibleFloatingActionBars.delete(floatingActionBarId)
    }

    updateFloatingActionBarDocumentState()
    if (!isShown) {
        clearIntercomBubbleClearance()
    }
}

let observer: ResizeObserver | null = null
let updateFrame: number | null = null

function scheduleFloatingActionBarLayoutUpdate() {
    if (typeof window === 'undefined') return
    if (updateFrame !== null) {
        window.cancelAnimationFrame(updateFrame)
    }

    updateFrame = window.requestAnimationFrame(() => {
        updateFrame = null
        updateFloatingActionBarState()
        updateIntercomBubbleClearance()
    })
}

watch(
    toolbarEl,
    (el) => {
        observer?.disconnect()
        if (!el) return
        observer = new ResizeObserver(() => {
            checkCompact()
            scheduleFloatingActionBarLayoutUpdate()
        })
        observer.observe(el.parentElement!)
        checkCompact()
        scheduleFloatingActionBarLayoutUpdate()
    },
    { immediate: true },
)

watch(
    shown,
    async (isShown) => {
        await nextTick()
        updateFloatingActionBarState(isShown)
        scheduleFloatingActionBarLayoutUpdate()
    },
    { immediate: true },
)

watch(
    [
        shown,
        leftOffset,
        rightOffset,
        stackCount,
        () => pageContext?.intercomBubble?.horizontalPadding.value,
        () => pageContext?.intercomBubble?.width.value,
    ],
    () => scheduleFloatingActionBarLayoutUpdate(),
    { immediate: true },
)

function handleResize() {
    if (stackCount.value === 0) {
        scrollbarWidth.value = window.innerWidth - document.documentElement.clientWidth
    }
    scheduleFloatingActionBarLayoutUpdate()
}

function handleTransitionEnd(event: TransitionEvent) {
    if (event.target === barEl.value && event.propertyName === 'bottom') {
        scheduleFloatingActionBarLayoutUpdate()
    }
}

onMounted(() => {
    scrollbarWidth.value = window.innerWidth - document.documentElement.clientWidth
    window.addEventListener('resize', handleResize)
    scheduleFloatingActionBarLayoutUpdate()
})

onUnmounted(() => {
    observer?.disconnect()
    window.removeEventListener('resize', handleResize)
    if (updateFrame !== null) {
        window.cancelAnimationFrame(updateFrame)
    }
    clearIntercomBubbleClearance()
    visibleFloatingActionBars.delete(floatingActionBarId)
    updateFloatingActionBarDocumentState()
})
</script>

<template>
    <Teleport to="body">
        <Transition name="floating-action-bar" appear>
            <div
                v-if="shown"
                ref="barEl"
                class="floating-action-bar pointer-events-none drop-shadow-2xl fixed p-4"
                :class="barClasses"
                :style="barStyle"
                aria-live="polite"
                @transitionend.self="handleTransitionEnd"
            >
                <div
                    ref="toolbarEl"
                    role="toolbar"
                    :aria-label="ariaLabel"
                    class="pointer-events-auto relative flex items-center gap-1.5 rounded-[var(--radius-xl)] bg-surface-3 border border-surface-5 border-solid mx-auto md:max-w-[60vw] px-3 py-2.5 shadow-[0px_1px_3px_0px_rgba(0,0,0,0.3),0px_6px_10px_0px_rgba(0,0,0,0.15)]"
                    :class="{
                        'overflow-visible': allowOverflow,
                        'overflow-clip': !allowOverflow,
                        'bar-compact': compact,
                    }"
                >
                    <slot />
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
.floating-action-bar {
    left: var(--floating-action-bar-left-offset, var(--left-bar-width, 0px));
    right: var(--floating-action-bar-right-offset, var(--right-bar-width, 0px));
    transition:
        bottom 0.25s ease-in-out,
        top 0.25s ease-in-out;
}

.floating-action-bar-enter-active {
    transition:
        transform 0.25s cubic-bezier(0.15, 1.4, 0.64, 0.96),
        opacity 0.25s cubic-bezier(0.15, 1.4, 0.64, 0.96);
}

.floating-action-bar-leave-active {
    transition:
        transform 0.25s ease,
        opacity 0.25s ease;
}

.floating-action-bar-enter-from {
    transform: scale(0.5) translateY(-10rem);
    opacity: 0;
}

.floating-action-bar-leave-to {
    transform: scale(0.96) translateY(-0.25rem);
    opacity: 0;
}

@media (any-hover: none) and (max-width: 640px) {
    .floating-action-bar.bottom-0 {
        bottom: var(--size-mobile-navbar-height);
    }
}
</style>

<style>
.bar-compact .bar-label {
    display: none;
}

.bar-compact .cq-show-icon {
    display: block;
}
</style>
