<script setup lang="ts">
import RightArrowIcon from '@modrinth/assets/icons/right-arrow.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'

import { messages } from '../../../messages'
import type { OnboardingStep } from './onboarding-config'
import OnboardingMascotStage from './OnboardingMascotStage.vue'

/** One tour step's card (apps/app-frontend/src/components/ui/onboarding/OnboardingDialogue.vue). */
const props = defineProps<{
    step: OnboardingStep
    current: number
    total: number
    docked: boolean
}>()

defineEmits<{
    advance: []
    skip: []
}>()

const { formatMessage } = useVIntl()
const progressWidth = computed(() => `${Math.min(100, (props.current / props.total) * 100)}%`)
</script>

<template>
    <div
        class="onboarding-dialogue-layout"
        :class="{ 'onboarding-dialogue-layout-docked': docked }"
    >
        <div v-if="docked" class="onboarding-dialogue-progress" aria-hidden="true">
            <span :style="{ width: progressWidth }"></span>
        </div>
        <OnboardingMascotStage :alt="formatMessage(messages.appOnboardingMascotAlt)" />
        <div class="onboarding-dialogue-copy">
            <div class="onboarding-dialogue-header">
                <div class="onboarding-dialogue-heading">
                    <p class="m-0 mb-1.5 text-[0.8125rem] font-bold text-brand">
                        {{ current }} / {{ total }}
                    </p>
                    <h2 :id="`onboarding-title-${step.id}`">{{ formatMessage(step.title) }}</h2>
                </div>
                <Button
                    type="quiet"
                    circular
                    icon-only
                    :label="formatMessage(messages.appOnboardingActionSkip)"
                    @click.stop="$emit('skip')"
                    ><XIcon />
                </Button>
            </div>
            <div class="onboarding-dialogue-body">
                <p :id="`onboarding-description-${step.id}`">
                    {{ formatMessage(step.description) }}
                </p>
                <Button
                    v-if="step.interaction === 'manual'"
                    type="colored"
                    color="brand"
                    @click="$emit('advance')"
                    >{{ formatMessage(step.action) }}
                    <RightArrowIcon />
                </Button>
                <p v-else class="onboarding-action-hint">
                    <RightArrowIcon aria-hidden="true" />
                    {{ formatMessage(step.action) }}
                </p>
            </div>
        </div>
    </div>
</template>

<style scoped>
.onboarding-dialogue-layout {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: start;
    gap: 0.875rem;
}

.onboarding-dialogue-layout > :deep(.onboarding-mascot) {
    width: 6rem;
    align-self: center;
    transform: scale(1.35);
    transform-origin: center;
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

.onboarding-dialogue-layout-docked > :deep(.onboarding-mascot) {
    width: 8rem;
    align-self: end;
    transform: none;
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
    max-width: 46ch;
    color: var(--color-text-primary);
    font-size: 1rem;
    line-height: 1.55;
}

.onboarding-dialogue-layout-docked .onboarding-dialogue-body > p:not(.onboarding-action-hint) {
    max-width: 68ch;
}

.onboarding-action-hint {
    font-size: 0.8125rem;
    font-weight: 700;
}

.onboarding-action-hint {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    margin-top: 0.625rem !important;
    color: var(--color-brand);
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
    .onboarding-dialogue-progress span {
        transition: none;
    }
}
</style>
