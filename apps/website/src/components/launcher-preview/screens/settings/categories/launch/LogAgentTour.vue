<script setup lang="ts">
import RightArrowIcon from '@modrinth/assets/icons/right-arrow.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { usePreview } from '../../../../lib/state/index'
import { injectPreviewWindow, WINDOW_HEIGHT } from '../../../../lib/window'
import { messages } from '../../messages'
import searchingVisual from './axo-searching.svg?url'
import teachingVisual from './axo-teaching.svg?url'

/**
 * The app's onboarding overlay (apps/app-frontend/src/components/ui/onboarding/
 * OnboardingOverlay.vue, OnboardingDialogue.vue, useOnboardingTour.ts) in the
 * `logagent` mode that "About analysis sources" replays: three inspect steps
 * that spotlight a region of this page above a dialogue docked to the bottom.
 * Any click advances, Esc leaves; a target that never appears is skipped.
 * It lives in the window's overlay layer and works in the window's own
 * (unscaled) coordinates.
 */
const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ complete: []; skip: [] }>()

const { formatMessage } = useVIntl()
const { clock } = usePreview()
const { root, layer, scale, toLocal } = injectPreviewWindow()

interface TourStep {
    id: string
    targetId: string
    title: MessageDescriptor
    description: MessageDescriptor
    action: MessageDescriptor
}

/** `onboardingTours.logagent` (onboardingConfig.ts). */
const steps: TourStep[] = [
    {
        id: 'logagent-source',
        targetId: 'logagent-settings-source',
        title: messages.appOnboardingLogagentSourceTitle,
        description: messages.appOnboardingLogagentSourceDescription,
        action: messages.appOnboardingActionContinueArea,
    },
    {
        id: 'logagent-sharing',
        targetId: 'logagent-settings-sharing',
        title: messages.appOnboardingLogagentSharingTitle,
        description: messages.appOnboardingLogagentSharingDescription,
        action: messages.appOnboardingActionContinueArea,
    },
    {
        id: 'logagent-cloud',
        targetId: 'logagent-settings-cloud',
        title: messages.appOnboardingLogagentCloudTitle,
        description: messages.appOnboardingLogagentCloudDescription,
        action: messages.appOnboardingActionContinueArea,
    },
]

const controlSpotlightPadding = 6
const missingTargetRetryLimit = 12
const missingTargetRetryDelay = 200

interface LocalRect {
    left: number
    top: number
    width: number
    height: number
    bottom: number
}

const stepIndex = ref(0)
const targetRect = ref<LocalRect | null>(null)
const bubbleElement = ref<HTMLElement>()
const bubbleHeight = ref(160)
const step = computed(() => steps[stepIndex.value])
const progressWidth = computed(
    () => `${Math.min(100, ((stepIndex.value + 1) / steps.length) * 100)}%`,
)
/** OnboardingMascotStage picks one of the two visuals each time a step's dialogue mounts. */
const mascotVisuals = [teachingVisual, searchingVisual]
const mascotVisual = ref(mascotVisuals[0])

let targetObserver: ResizeObserver | undefined
let bubbleObserver: ResizeObserver | undefined
let cancelTargetRetry: (() => void) | undefined
let cancelUnlock: (() => void) | undefined
let measureFrame: number | undefined
let targetRetryCount = 0
let transitionLocked = false

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

function localRect(element: Element): LocalRect {
    const rect = element.getBoundingClientRect()
    const origin = toLocal(rect.left, rect.top)
    const width = rect.width / scale.value
    const height = rect.height / scale.value
    return { left: origin.x, top: origin.y, width, height, bottom: origin.y + height }
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

/**
 * Brings the target into the free area of its scroller: visible, not under the
 * window chrome and not under the docked dialogue. Only the window's own
 * scroller moves, never the page around the window.
 */
function scrollTargetIntoView(target: HTMLElement, reservedBottom: number) {
    const scroller = target.closest<HTMLElement>(
        '[data-onboarding-scroll], .overflow-y-auto, .settings-content-scroll, .app-viewport',
    )
    if (!scroller) return
    const box = localRect(scroller)
    let rect = localRect(target)
    if (rect.top < box.top) {
        scroller.scrollTop -= box.top - rect.top
    } else if (rect.bottom > box.top + box.height) {
        scroller.scrollTop += Math.min(rect.bottom - (box.top + box.height), rect.top - box.top)
    }

    rect = localRect(target)
    const maxBottom = WINDOW_HEIGHT - reservedBottom
    if (rect.bottom > maxBottom) {
        const delta = rect.bottom - maxBottom + 12
        if (scroller.scrollHeight > scroller.clientHeight) scroller.scrollTop += delta
    } else if (rect.top < safeTopInset()) {
        const delta = safeTopInset() - rect.top + 12
        if (scroller.scrollTop > 0) scroller.scrollTop = Math.max(0, scroller.scrollTop - delta)
    }
}

function clearTargetTracking() {
    targetObserver?.disconnect()
    targetObserver = undefined
    cancelTargetRetry?.()
    cancelTargetRetry = undefined
    if (measureFrame !== undefined) cancelAnimationFrame(measureFrame)
    measureFrame = undefined
}

function updateBubbleSize() {
    if (bubbleElement.value) bubbleHeight.value = bubbleElement.value.offsetHeight
}

function scheduleMissingTargetRetry(stepId: string) {
    if (targetRetryCount >= missingTargetRetryLimit) {
        void advance()
        return
    }

    targetRetryCount++
    cancelTargetRetry = clock.after(missingTargetRetryDelay, () => {
        if (props.visible && step.value.id === stepId && !targetRect.value) updateTarget()
    })
}

function updateTarget() {
    clearTargetTracking()
    if (!props.visible) {
        targetRect.value = null
        return
    }

    const target = root.value?.querySelector<HTMLElement>(
        `[data-onboarding-id="${step.value.targetId}"]`,
    )
    if (!target) {
        targetRect.value = null
        scheduleMissingTargetRetry(step.value.id)
        return
    }

    // Every step docks the dialogue, so keep the target clear of it.
    scrollTargetIntoView(target, bubbleHeight.value + 24)

    targetRetryCount = 0
    const stepId = step.value.id
    const updateRect = () => {
        const rect = localRect(target)
        if (rect.width < 1 || rect.height < 1) {
            targetRect.value = null
            scheduleMissingTargetRetry(stepId)
            return
        }
        targetRect.value = rect
    }
    updateRect()
    targetObserver = new ResizeObserver(updateRect)
    targetObserver.observe(target)
    measureFrame = requestAnimationFrame(() => {
        updateRect()
        updateBubbleSize()
        // Second pass after any scroll/layout settle.
        measureFrame = requestAnimationFrame(updateRect)
    })
}

function scheduleUnlock() {
    cancelUnlock?.()
    cancelUnlock = clock.after(180, () => {
        transitionLocked = false
        cancelUnlock = undefined
    })
}

async function advance() {
    if (transitionLocked) return
    transitionLocked = true

    if (stepIndex.value === steps.length - 1) {
        emit('complete')
        scheduleUnlock()
        return
    }

    stepIndex.value++
    await nextTick()
    updateTarget()
    scheduleUnlock()
}

/** Inspect steps: a click anywhere outside the overlay advances without reaching the page. */
function handleWindowClick(event: MouseEvent) {
    if (!props.visible) return
    const clickedElement = event.target instanceof Element ? event.target : null
    if (clickedElement?.closest('[data-onboarding-overlay-ui]')) return
    event.preventDefault()
    event.stopImmediatePropagation()
    void advance()
}

function handleKeydown(event: KeyboardEvent) {
    if (!props.visible || event.key !== 'Escape') return
    const target = event.target
    const inWindow =
        target === document.body || (target instanceof Node && !!root.value?.contains(target))
    if (!inWindow) return
    event.preventDefault()
    emit('skip')
}

function handleViewportChange() {
    if (props.visible) updateTarget()
}

watch(
    () => props.visible,
    async (isVisible) => {
        if (isVisible) {
            stepIndex.value = 0
            targetRetryCount = 0
            mascotVisual.value = mascotVisuals[Math.floor(Math.random() * mascotVisuals.length)]
            await nextTick()
            updateBubbleSize()
            if (bubbleElement.value) bubbleObserver?.observe(bubbleElement.value)
        } else {
            bubbleObserver?.disconnect()
        }
        updateTarget()
    },
)

watch(step, async () => {
    if (!props.visible) return
    targetRetryCount = 0
    mascotVisual.value = mascotVisuals[Math.floor(Math.random() * mascotVisuals.length)]
    await nextTick()
    updateBubbleSize()
    updateTarget()
})

onMounted(() => {
    root.value?.addEventListener('click', handleWindowClick, true)
    root.value?.addEventListener('scroll', handleViewportChange, true)
    document.addEventListener('keydown', handleKeydown)
    window.addEventListener('resize', handleViewportChange)
    bubbleObserver = new ResizeObserver(updateBubbleSize)
    if (bubbleElement.value) bubbleObserver.observe(bubbleElement.value)
})

onBeforeUnmount(() => {
    clearTargetTracking()
    bubbleObserver?.disconnect()
    cancelUnlock?.()
    root.value?.removeEventListener('click', handleWindowClick, true)
    root.value?.removeEventListener('scroll', handleViewportChange, true)
    document.removeEventListener('keydown', handleKeydown)
    window.removeEventListener('resize', handleViewportChange)
})
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <div
            v-if="visible"
            class="pointer-events-none fixed inset-0 z-[10001] overflow-hidden"
            aria-live="polite"
        >
            <div v-if="targetRect" class="onboarding-mask" aria-hidden="true">
                <div class="onboarding-mask-hole" :style="spotlightStyle" />
            </div>

            <section
                ref="bubbleElement"
                data-onboarding-overlay-ui
                class="onboarding-surface onboarding-surface-dialogue"
                :aria-labelledby="`onboarding-title-${step.id}`"
                :aria-describedby="`onboarding-description-${step.id}`"
                @click="advance()"
            >
                <div
                    :key="`logagent-${step.id}`"
                    class="onboarding-dialogue-layout onboarding-dialogue-layout-docked"
                >
                    <div class="onboarding-dialogue-progress" aria-hidden="true">
                        <span :style="{ width: progressWidth }"></span>
                    </div>
                    <img
                        class="onboarding-mascot"
                        :src="mascotVisual"
                        :alt="formatMessage(messages.appOnboardingMascotAlt)"
                    />
                    <div class="onboarding-dialogue-copy">
                        <div class="onboarding-dialogue-header">
                            <div class="onboarding-dialogue-heading">
                                <p class="m-0 mb-1.5 text-[0.8125rem] font-bold text-brand">
                                    {{ stepIndex + 1 }} / {{ steps.length }}
                                </p>
                                <h2 :id="`onboarding-title-${step.id}`">
                                    {{ formatMessage(step.title) }}
                                </h2>
                            </div>
                            <Button
                                type="quiet"
                                circular
                                icon-only
                                :label="formatMessage(messages.appOnboardingActionSkip)"
                                @click.stop="emit('skip')"
                                ><XIcon />
                            </Button>
                        </div>
                        <div class="onboarding-dialogue-body">
                            <p :id="`onboarding-description-${step.id}`">
                                {{ formatMessage(step.description) }}
                            </p>
                            <p class="onboarding-action-hint">
                                <RightArrowIcon aria-hidden="true" />
                                {{ formatMessage(step.action) }}
                            </p>
                        </div>
                    </div>
                </div>
            </section>
        </div>
    </Teleport>
</template>

<style scoped>
.onboarding-mask {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
}

.onboarding-mask-hole {
    position: fixed;
    border-radius: var(--radius-md);
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.55);
    transition:
        left 160ms ease,
        top 160ms ease,
        width 160ms ease,
        height 160ms ease;
}

.onboarding-surface {
    position: fixed;
    z-index: 2;
    box-sizing: border-box;
    border: 1px solid var(--color-divider);
    background: var(--color-super-raised-bg);
    pointer-events: auto;
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.35);
}

/* Docked across the bottom of the window (40vh of the window is 320px). */
.onboarding-surface-dialogue {
    right: 0;
    bottom: 0;
    left: 0;
    width: auto;
    min-height: 0;
    max-height: min(15rem, 320px);
    overflow-y: auto;
    border: 0;
    border-top: 1px solid var(--color-divider);
    border-radius: 0;
    padding: 0.75rem 1.5rem;
}

.onboarding-dialogue-layout {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: start;
    gap: 0.875rem;
}

.onboarding-dialogue-layout-docked {
    width: min(72rem, 100%);
    margin-inline: auto;
    gap: 1.25rem;
}

.onboarding-dialogue-progress {
    position: absolute;
    top: -1px;
    left: 0;
    width: 100%;
    height: 2px;
    overflow: hidden;
    pointer-events: none;
}

.onboarding-dialogue-progress span {
    display: block;
    height: 100%;
    background: var(--color-brand);
    transition: width 280ms cubic-bezier(0.22, 1, 0.36, 1);
}

.onboarding-mascot {
    width: 8rem;
    height: auto;
    align-self: end;
    object-fit: contain;
}

.onboarding-dialogue-copy {
    min-width: 0;
    color: var(--color-text-primary);
}

.onboarding-dialogue-header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: flex-start;
    gap: 1rem;
    padding-bottom: 0.375rem;
}

.onboarding-dialogue-heading h2,
.onboarding-dialogue-body p {
    margin: 0;
}

.onboarding-dialogue-heading h2 {
    color: var(--color-text-primary);
    font-size: 1.25rem;
    font-weight: 700;
    line-height: 1.25;
    letter-spacing: 0;
}

.onboarding-dialogue-body > p:not(.onboarding-action-hint) {
    max-width: 68ch;
    color: var(--color-text-primary);
    font-size: 1rem;
    line-height: 1.55;
}

.onboarding-action-hint {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    margin-top: 0.625rem !important;
    color: var(--color-brand);
    font-size: 0.8125rem;
    font-weight: 700;
    text-wrap: pretty;
}

.onboarding-action-hint :deep(svg) {
    flex: none;
}

.onboarding-dialogue-copy :deep(svg) {
    width: 1rem;
    height: 1rem;
}

@media (prefers-reduced-motion: reduce) {
    .onboarding-mask-hole,
    .onboarding-dialogue-progress span {
        transition: none;
    }
}
</style>
