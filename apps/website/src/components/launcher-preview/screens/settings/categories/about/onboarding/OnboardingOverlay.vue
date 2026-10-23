<script setup lang="ts">
import { ref, toRef } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import type { OnboardingMode } from './onboarding-config'
import OnboardingDialogue from './OnboardingDialogue.vue'
import OnboardingWelcome from './OnboardingWelcome.vue'
import { useOnboardingTour } from './use-onboarding-tour'

/**
 * The guided tour (apps/app-frontend/src/components/ui/onboarding/OnboardingOverlay.vue)
 * with the part of App.vue that hosts it: a replayed tour writes no onboarding
 * state, and finishing or leaving it just closes the overlay. The shell's
 * app-overlay host renders it, so it keeps running across pages.
 */
const props = defineProps<{
    mode: OnboardingMode
}>()

const emit = defineEmits<{
    close: []
}>()

const route = useRoute()
const router = useRouter()
const visible = ref(true)

/** App.vue `finishOnboarding` for a replay: nothing is stored. */
function finishOnboarding() {
    visible.value = false
    emit('close')
}

/** App.vue `closeOnboardingSettings`. */
async function closeOnboardingSettings() {
    if (route.path === '/settings') await router.replace('/')
}

const {
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
    advance,
} = useOnboardingTour(visible, toRef(props, 'mode'), {
    complete: finishOnboarding,
    skip: finishOnboarding,
    closeSettings: () => void closeOnboardingSettings(),
})
</script>

<template>
    <div
        v-if="visible"
        class="pointer-events-none fixed inset-0 z-[10001] overflow-hidden"
        aria-live="polite"
    >
        <div v-if="showSpotlight" class="onboarding-mask" aria-hidden="true">
            <div class="onboarding-mask-hole" :style="spotlightStyle" />
        </div>

        <div
            v-if="showSpotlightCorners && targetRect"
            class="onboarding-spotlight"
            :style="spotlightStyle"
            aria-hidden="true"
        >
            <span class="onboarding-corner onboarding-corner-top-left"></span>
            <span class="onboarding-corner onboarding-corner-top-right"></span>
            <span class="onboarding-corner onboarding-corner-bottom-right"></span>
            <span class="onboarding-corner onboarding-corner-bottom-left"></span>
        </div>

        <section
            ref="bubbleElement"
            data-onboarding-overlay-ui
            class="onboarding-surface"
            :class="[
                `onboarding-surface-${bubblePlacement.direction}`,
                {
                    'onboarding-surface-centered': !targetRect,
                    'onboarding-surface-dialogue': isDialogueStep,
                    'onboarding-surface-welcome': isWelcomeStep,
                },
            ]"
            :style="bubblePlacement.style"
            :aria-labelledby="`onboarding-title-${step.id}`"
            :aria-describedby="`onboarding-description-${step.id}`"
            @click="step.interaction === 'inspect' ? advance() : undefined"
        >
            <OnboardingWelcome
                v-if="isWelcomeStep"
                :step="step"
                @start="handleManualClick"
                @skip="finishOnboarding"
            />
            <OnboardingDialogue
                v-else
                :key="`${mode}-${step.id}`"
                :step="step"
                :current="stepIndex + 1"
                :total="steps.length"
                :docked="isDialogueStep"
                @advance="handleManualClick"
                @skip="finishOnboarding"
            />
        </section>
    </div>
</template>

<style scoped>
/* Viewport sizes of the app resolve against the 1280 × 800 window. */
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

.onboarding-spotlight {
    position: fixed;
    z-index: 1;
    pointer-events: none;
    transform-origin: center;
    animation: onboarding-focus 1.6s ease-in-out infinite;
}

.onboarding-corner {
    position: absolute;
    width: 0.75rem;
    height: 0.75rem;
    border-color: var(--color-brand);
    border-style: solid;
    border-width: 0;
}

.onboarding-corner-top-left {
    top: 0;
    left: 0;
    border-top-width: 2px;
    border-left-width: 2px;
}

.onboarding-corner-top-right {
    top: 0;
    right: 0;
    border-top-width: 2px;
    border-right-width: 2px;
}

.onboarding-corner-bottom-right {
    right: 0;
    bottom: 0;
    border-right-width: 2px;
    border-bottom-width: 2px;
}

.onboarding-corner-bottom-left {
    bottom: 0;
    left: 0;
    border-bottom-width: 2px;
    border-left-width: 2px;
}

.onboarding-surface {
    position: fixed;
    z-index: 2;
    width: 32rem;
    max-height: 736px;
    overflow-y: auto;
    border: 1px solid var(--color-divider);
    border-radius: var(--radius-lg);
    background: var(--color-super-raised-bg);
    padding: 1.25rem;
    box-sizing: border-box;
    pointer-events: auto;
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.35);
}

.onboarding-surface-centered {
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
}

.onboarding-surface-welcome {
    inset: 0;
    z-index: 3;
    width: auto;
    height: 800px;
    min-height: 0;
    max-height: none;
    transform: none;
    overflow: hidden;
    border: 0;
    border-radius: 0;
    padding: 0;
}

.onboarding-surface-dialogue {
    top: auto !important;
    right: 0;
    bottom: 0;
    left: 0 !important;
    z-index: 2;
    width: auto;
    min-height: 0;
    max-height: 15rem;
    transform: none;
    overflow-y: auto;
    border: 0;
    border-top: 1px solid var(--color-divider);
    border-radius: 0;
    padding: 0.75rem 1.5rem;
}

:global(.launcher-preview-window.onboarding-reserve-dialogue-space .modal-container) {
    height: calc(100% - var(--onboarding-dialogue-reserved-space, 0px));
}

@keyframes onboarding-focus {
    0%,
    100% {
        transform: scale(1);
    }
    50% {
        transform: scale(1.015);
    }
}

@media (prefers-reduced-motion: reduce) {
    .onboarding-spotlight {
        animation: none;
    }

    .onboarding-mask-hole {
        transition: none;
    }
}
</style>
