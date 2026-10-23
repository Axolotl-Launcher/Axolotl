import { computed, nextTick, onBeforeUnmount, onMounted, type Ref, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import { usePreview } from '../../../../../lib/state/index'
import { injectPreviewWindow, WINDOW_HEIGHT, WINDOW_WIDTH } from '../../../../../lib/window'
import {
    type CreationPath,
    type OnboardingMode,
    onboardingTargetSelector,
    onboardingTours,
    type StepDestination,
} from './onboarding-config'

/**
 * The tour runtime (apps/app-frontend/src/components/ui/onboarding/useOnboardingTour.ts).
 * The app measures in viewport pixels; the launcher window is a scaled element
 * of the website, so every rectangle here is converted to the window's own
 * 1280 × 800 space, the space the overlay's fixed elements resolve against.
 * Only clicks inside the window take part in the tour, and target scrolling
 * stays inside the window's scrollers instead of moving the website page.
 */
type TourEvents = {
    complete: () => void
    skip: () => void
    closeSettings: () => void
}

/** A rectangle in window coordinates. */
export interface WindowRect {
    left: number
    top: number
    right: number
    bottom: number
    width: number
    height: number
}

const controlSpotlightPadding = 6
const missingTargetRetryLimit = 12
const missingTargetRetryDelay = 200
const SCROLLER_SELECTOR =
    '[data-onboarding-scroll], .overflow-y-auto, .settings-content-scroll, .app-viewport'

export function useOnboardingTour(
    visible: Ref<boolean>,
    mode: Ref<OnboardingMode>,
    events: TourEvents,
) {
    const route = useRoute()
    const { clock } = usePreview()
    const { root, scale, toLocal } = injectPreviewWindow()
    const stepIndex = ref(0)
    const targetRect = ref<WindowRect | null>(null)
    const bubbleElement = ref<HTMLElement>()
    const bubbleSize = ref({ width: 512, height: 160 })
    const waitingForRoute = ref(false)
    const creationPath = ref<CreationPath>()
    const targetElement = ref<HTMLElement>()
    let targetObserver: ResizeObserver | undefined
    let bubbleObserver: ResizeObserver | undefined
    let cancelTargetRetry: (() => void) | undefined
    let measureTimer: ReturnType<typeof requestAnimationFrame> | undefined
    let cancelAdvance: (() => void) | undefined
    let cancelUnlock: (() => void) | undefined
    let targetRetryCount = 0
    let transitionLocked = false

    function windowRect(element: Element): WindowRect {
        const rect = element.getBoundingClientRect()
        const factor = scale.value || 1
        const origin = toLocal(rect.left, rect.top)
        const width = rect.width / factor
        const height = rect.height / factor
        return {
            left: origin.x,
            top: origin.y,
            right: origin.x + width,
            bottom: origin.y + height,
            width,
            height,
        }
    }

    function readCssLength(variableName: string, fallback: number) {
        const element = root.value
        if (!element) return fallback
        const raw = getComputedStyle(element).getPropertyValue(variableName).trim()
        if (!raw) return fallback
        const parsed = Number.parseFloat(raw)
        if (Number.isFinite(parsed)) {
            return raw.endsWith('px') || !raw.endsWith('rem')
                ? parsed
                : parsed * Number.parseFloat(getComputedStyle(element).fontSize)
        }
        return fallback
    }

    function safeTopInset() {
        // Match the live chrome instead of assuming a fixed 48px status bar.
        return Math.max(16, readCssLength('--top-bar-height', 48) + 8)
    }

    /** `scrollIntoView({ block: 'nearest' })` limited to the scrollers inside the window. */
    function scrollWithinWindow(target: HTMLElement) {
        let scroller = target.parentElement
        while (scroller && root.value?.contains(scroller)) {
            if (scroller.scrollHeight > scroller.clientHeight) {
                const style = getComputedStyle(scroller)
                if (/(auto|scroll)/.test(style.overflowY)) {
                    const targetBox = windowRect(target)
                    const scrollerBox = windowRect(scroller)
                    if (targetBox.top < scrollerBox.top) {
                        scroller.scrollTop -= scrollerBox.top - targetBox.top
                    } else if (targetBox.bottom > scrollerBox.bottom) {
                        scroller.scrollTop += Math.min(
                            targetBox.bottom - scrollerBox.bottom,
                            targetBox.top - scrollerBox.top,
                        )
                    }
                }
            }
            scroller = scroller.parentElement
        }
    }

    function scrollTargetIntoView(target: HTMLElement, reservedBottom: number) {
        scrollWithinWindow(target)

        // Custom sidebars, collapsed rails, and docked dialogue can leave the target
        // under chrome or under the bottom bubble after replay. Nudge scrollable
        // ancestors so the control stays in the free area.
        const rect = windowRect(target)
        const maxBottom = WINDOW_HEIGHT - reservedBottom
        if (rect.bottom > maxBottom) {
            const delta = rect.bottom - maxBottom + 12
            const scroller = target.closest<HTMLElement>(SCROLLER_SELECTOR)
            if (scroller && scroller.scrollHeight > scroller.clientHeight) {
                scroller.scrollTop += delta
            }
        } else if (rect.top < safeTopInset()) {
            const delta = safeTopInset() - rect.top + 12
            const scroller = target.closest<HTMLElement>(SCROLLER_SELECTOR)
            if (scroller && scroller.scrollTop > 0) {
                scroller.scrollTop = Math.max(0, scroller.scrollTop - delta)
            }
        }
    }

    const steps = computed(() => onboardingTours[mode.value])
    const step = computed(() => steps.value[stepIndex.value])
    const isWelcomeStep = computed(() => step.value.id === 'welcome')
    const isDialogueStep = computed(
        () => !!step.value.targetId && (step.value.spotlight !== 'control' || !targetRect.value),
    )
    const showSpotlight = computed(
        () => !!targetRect.value && !!step.value.targetId && !isWelcomeStep.value,
    )
    const showSpotlightCorners = computed(
        () => showSpotlight.value && step.value.spotlight === 'control',
    )
    const spotlightStyle = computed(() => {
        if (!targetRect.value) return {}
        const rect = targetRect.value
        const pad = controlSpotlightPadding
        return {
            left: `${Math.max(0, rect.left - pad)}px`,
            top: `${Math.max(0, rect.top - pad)}px`,
            width: `${rect.width + pad * 2}px`,
            height: `${rect.height + pad * 2}px`,
        }
    })
    const bubblePlacement = computed(() => {
        if (!targetRect.value || isDialogueStep.value) return { direction: 'center', style: {} }

        const rect = targetRect.value
        const safeInset = 16
        const topInset = safeTopInset()
        const bubbleWidth = Math.min(bubbleSize.value.width, WINDOW_WIDTH - safeInset * 2)
        const bubbleHeight = Math.min(bubbleSize.value.height, WINDOW_HEIGHT - topInset - safeInset)
        const gap = 20
        const positions = [
            {
                direction: 'right',
                left: rect.right + gap,
                top: rect.top + rect.height / 2 - bubbleHeight / 2,
            },
            {
                direction: 'bottom',
                left: rect.left + rect.width / 2 - bubbleWidth / 2,
                top: rect.bottom + gap,
            },
            {
                direction: 'left',
                left: rect.left - bubbleWidth - gap,
                top: rect.top + rect.height / 2 - bubbleHeight / 2,
            },
            {
                direction: 'top',
                left: rect.left + rect.width / 2 - bubbleWidth / 2,
                top: rect.top - bubbleHeight - gap,
            },
        ]
        const position =
            positions.find(
                (candidate) =>
                    candidate.left >= safeInset &&
                    candidate.top >= topInset &&
                    candidate.left + bubbleWidth <= WINDOW_WIDTH - safeInset &&
                    candidate.top + bubbleHeight <= WINDOW_HEIGHT - safeInset,
            ) ?? positions[0]

        return {
            direction: position.direction,
            style: {
                left: `${Math.min(
                    Math.max(safeInset, position.left),
                    Math.max(safeInset, WINDOW_WIDTH - bubbleWidth - safeInset),
                )}px`,
                top: `${Math.min(
                    Math.max(topInset, position.top),
                    Math.max(topInset, WINDOW_HEIGHT - bubbleHeight - safeInset),
                )}px`,
            },
        }
    })

    function clearTargetTracking() {
        targetObserver?.disconnect()
        targetObserver = undefined
        cancelTargetRetry?.()
        cancelTargetRetry = undefined
        if (measureTimer !== undefined) cancelAnimationFrame(measureTimer)
        measureTimer = undefined
    }

    function clearModalReservation() {
        root.value?.classList.remove('onboarding-reserve-dialogue-space')
        root.value?.style.removeProperty('--onboarding-dialogue-reserved-space')
    }

    function updateModalReservation() {
        const targetIsInModal = !!targetElement.value?.closest('[role="dialog"]')
        if (!visible.value || !isDialogueStep.value || !targetIsInModal) {
            clearModalReservation()
            return
        }

        root.value?.classList.add('onboarding-reserve-dialogue-space')
        root.value?.style.setProperty(
            '--onboarding-dialogue-reserved-space',
            `${Math.ceil(bubbleSize.value.height)}px`,
        )
    }

    function updateBubbleSize() {
        if (!bubbleElement.value) return
        const { width, height } = windowRect(bubbleElement.value)
        bubbleSize.value = { width, height }
        updateModalReservation()
    }

    function scheduleMissingTargetRetry(stepId: string) {
        if (targetRetryCount >= missingTargetRetryLimit) {
            void advance()
            return
        }

        targetRetryCount++
        cancelTargetRetry = clock.after(missingTargetRetryDelay, () => {
            cancelTargetRetry = undefined
            if (visible.value && step.value.id === stepId && !targetRect.value) updateTarget()
        })
    }

    function updateTarget() {
        clearTargetTracking()
        if (!visible.value || !step.value.targetId) {
            targetElement.value = undefined
            targetRect.value = null
            clearModalReservation()
            return
        }

        const target = root.value?.querySelector<HTMLElement>(
            onboardingTargetSelector(step.value.targetId),
        )
        if (!target) {
            targetElement.value = undefined
            targetRect.value = null
            clearModalReservation()
            scheduleMissingTargetRetry(step.value.id)
            return
        }

        // Replay on a customized shell can leave the target scrolled out or under
        // the docked dialogue / status bar. Bring it into the free area first,
        // then measure against the live layout.
        const reservedBottom = isDialogueStep.value ? bubbleSize.value.height + 24 : 24
        scrollTargetIntoView(target, reservedBottom)

        targetRetryCount = 0
        targetElement.value = target
        const updateRect = () => {
            const rect = windowRect(target)
            if (rect.width < 1 || rect.height < 1) {
                targetRect.value = null
                scheduleMissingTargetRetry(step.value.id)
                return
            }
            targetRect.value = rect
        }
        updateRect()
        targetObserver = new ResizeObserver(updateRect)
        targetObserver.observe(target)
        updateModalReservation()
        measureTimer = requestAnimationFrame(() => {
            updateRect()
            updateBubbleSize()
            // Second pass after any scroll/layout settle from custom chrome.
            measureTimer = requestAnimationFrame(updateRect)
        })
    }

    function goTo(destination: StepDestination) {
        if (destination === 'complete') {
            events.complete()
            return
        }

        const destinationIndex = steps.value.findIndex((candidate) => candidate.id === destination)
        if (destinationIndex === -1) {
            events.complete()
            return
        }

        stepIndex.value = destinationIndex
    }

    async function advance() {
        if (transitionLocked) return
        transitionLocked = true

        const destination = creationPath.value
            ? step.value.nextByCreationPath?.[creationPath.value]
            : step.value.next
        if (destination) {
            goTo(destination)
            scheduleUnlock()
            return
        }

        if (stepIndex.value === steps.value.length - 1) {
            events.complete()
            scheduleUnlock()
            return
        }

        if (step.value.closeSettingsAfter) events.closeSettings()
        stepIndex.value++
        await nextTick()
        updateTarget()
        scheduleUnlock()
    }

    function scheduleUnlock() {
        cancelUnlock?.()
        cancelUnlock = clock.after(180, () => {
            transitionLocked = false
            cancelUnlock = undefined
        })
    }

    function scheduleDestination(destination?: StepDestination) {
        if (cancelAdvance || transitionLocked) return
        if (destination) transitionLocked = true
        cancelAdvance = clock.after(150, () => {
            cancelAdvance = undefined
            if (destination) {
                goTo(destination)
                scheduleUnlock()
            } else {
                void advance()
            }
        })
    }

    function handleManualClick() {
        if (step.value.interaction === 'manual') void advance()
    }

    function handleBranchClick(target: Element) {
        if (!step.value.branchByTarget) return false

        for (const [targetId, branch] of Object.entries(step.value.branchByTarget)) {
            if (!target.closest(onboardingTargetSelector(targetId))) continue
            creationPath.value = branch.creationPath
            scheduleDestination(branch.next)
            return true
        }
        return false
    }

    function handleDocumentClick(event: MouseEvent) {
        if (!visible.value) return
        const clickedElement = event.target instanceof Element ? event.target : null
        // The website around the launcher window is not part of the app.
        if (!clickedElement || !root.value?.contains(clickedElement)) return
        if (clickedElement.closest('[data-onboarding-overlay-ui]')) return

        if (step.value.interaction === 'inspect') {
            event.preventDefault()
            event.stopImmediatePropagation()
            void advance()
            return
        }

        if (!step.value.targetId || !['navigate', 'activate'].includes(step.value.interaction))
            return
        if (handleBranchClick(clickedElement)) return
        if (!targetElement.value?.contains(clickedElement)) return

        if (step.value.expectedPath && route.path !== step.value.expectedPath) {
            waitingForRoute.value = true
            return
        }
        scheduleDestination()
    }

    function handleKeydown(event: KeyboardEvent) {
        if (!visible.value || event.key !== 'Escape') return
        event.preventDefault()
        events.skip()
    }

    watch(
        () => route.path,
        (path) => {
            if (!waitingForRoute.value || !step.value.expectedPath) return
            if (path !== step.value.expectedPath) return
            waitingForRoute.value = false
            void advance()
        },
    )

    async function onVisibleChange(isVisible: boolean) {
        if (isVisible) {
            stepIndex.value = 0
            waitingForRoute.value = false
            creationPath.value = undefined
            targetRetryCount = 0
            await nextTick()
            updateBubbleSize()
            if (bubbleElement.value) bubbleObserver?.observe(bubbleElement.value)
        } else {
            bubbleObserver?.disconnect()
            clearModalReservation()
        }
        updateTarget()
    }

    watch(visible, onVisibleChange)

    watch(step, async () => {
        if (!visible.value) return
        targetRetryCount = 0
        await nextTick()
        updateBubbleSize()
        updateTarget()
    })

    watch(mode, () => {
        stepIndex.value = 0
    })

    onMounted(() => {
        document.addEventListener('click', handleDocumentClick, true)
        document.addEventListener('keydown', handleKeydown)
        window.addEventListener('resize', updateTarget)
        window.addEventListener('scroll', updateTarget, true)
        bubbleObserver = new ResizeObserver(updateBubbleSize)
        if (bubbleElement.value) bubbleObserver.observe(bubbleElement.value)
        // The overlay host mounts the tour already visible.
        if (visible.value) void onVisibleChange(true)
    })

    onBeforeUnmount(() => {
        clearTargetTracking()
        clearModalReservation()
        bubbleObserver?.disconnect()
        cancelAdvance?.()
        cancelUnlock?.()
        document.removeEventListener('click', handleDocumentClick, true)
        document.removeEventListener('keydown', handleKeydown)
        window.removeEventListener('resize', updateTarget)
        window.removeEventListener('scroll', updateTarget, true)
    })

    return {
        advance,
        bubbleElement,
        bubblePlacement,
        spotlightStyle,
        showSpotlight,
        showSpotlightCorners,
        handleManualClick,
        isDialogueStep,
        isWelcomeStep,
        step,
        stepIndex,
        steps,
        targetRect,
    }
}
