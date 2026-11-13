<script setup lang="ts">
import { CheckIcon, LeftArrowIcon, RightArrowIcon, SpinnerIcon } from '@modrinth/assets'
import {
    Button,
    defineMessages,
    injectNotificationManager,
    provideNotificationManager,
    useVIntl,
} from '@modrinth/ui'
import { computed, onMounted, reactive, ref } from 'vue'

import AxolotlLogo from '@/components/ui/AxolotlLogo.vue'
import WindowControls from '@/components/ui/WindowControls.vue'
import { AppNotificationManager } from '@/providers/app-notifications'

import OobeStepScope from './OobeStepScope.vue'
import type { OobeStepDefinition } from './steps'

const props = defineProps<{
    steps: readonly OobeStepDefinition[]
}>()

const emit = defineEmits<{
    /** The user moved past this step; the caller records it. */
    complete: [id: string]
    finish: []
}>()

const { formatMessage } = useVIntl()

// The app holds its own notifications while the OOBE is open; what a step raises must show now.
const notifications = injectNotificationManager()
if (notifications instanceof AppNotificationManager)
    provideNotificationManager(notifications.immediate())

const messages = defineMessages({
    next: { id: 'app.oobe.action.next', defaultMessage: 'Next' },
    back: { id: 'app.oobe.action.back', defaultMessage: 'Back' },
    skip: { id: 'app.oobe.action.skip', defaultMessage: 'Skip' },
    finish: { id: 'app.oobe.action.finish', defaultMessage: 'Finish' },
    progress: { id: 'app.oobe.progress', defaultMessage: 'Setup progress' },
    dialogLabel: { id: 'app.oobe.dialog-label', defaultMessage: 'Set up Axolotl Launcher' },
})

const index = ref(0)
const direction = ref<'forward' | 'back'>('forward')
type StepState = { busy: boolean; ready: boolean; changed: boolean }
const stepState = reactive(new Map<string, StepState>())
const heading = ref<HTMLElement | null>(null)

const step = computed(() => props.steps[Math.min(index.value, props.steps.length - 1)])
const isLast = computed(() => index.value >= props.steps.length - 1)
const railSteps = computed(() => props.steps.filter((entry) => entry.layout === 'card'))
const state = computed(
    () => stepState.get(step.value.id) ?? { busy: false, ready: true, changed: false },
)
const skipping = computed(() => step.value.skippable && !state.value.changed)

function patchState(stepId: string, patch: Partial<StepState>) {
    stepState.set(stepId, {
        busy: false,
        ready: true,
        changed: false,
        ...stepState.get(stepId),
        ...patch,
    })
}

const controls = {
    setBusy: (stepId: string, busy: boolean) => patchState(stepId, { busy }),
    setReady: (stepId: string, ready: boolean) => patchState(stepId, { ready }),
    // A change is recorded at once, so quitting before Next never offers to repeat an import.
    markChanged: (stepId: string) => {
        patchState(stepId, { changed: true })
        emit('complete', stepId)
    },
    next: (stepId: string) => {
        if (stepId === step.value.id) advance()
    },
}

function advance() {
    if (state.value.busy) return
    emit('complete', step.value.id)
    if (isLast.value) {
        emit('finish')
        return
    }
    direction.value = 'forward'
    index.value += 1
}

function goBack() {
    if (state.value.busy || index.value === 0) return
    direction.value = 'back'
    index.value -= 1
}

function railState(entry: OobeStepDefinition) {
    const position = props.steps.indexOf(entry)
    if (position === index.value) return 'current'
    return position < index.value ? 'done' : 'upcoming'
}

/** Runs once a step has finished entering; out-in keeps the old heading mounted until then. */
function focusHeading() {
    heading.value?.focus({ preventScroll: true })
}

onMounted(focusHeading)
</script>

<template>
    <div
        class="oobe-root fixed inset-0 z-[95] flex flex-col overflow-hidden bg-[var(--color-bg)] text-[var(--color-text-default)]"
        role="dialog"
        aria-modal="true"
        :aria-label="formatMessage(messages.dialogLabel)"
    >
        <div
            class="oobe-backdrop pointer-events-none absolute inset-0 overflow-hidden"
            aria-hidden="true"
        >
            <span class="oobe-glow oobe-glow-a" />
            <span class="oobe-glow oobe-glow-b" />
        </div>

        <!--
            Tauri only drags from an element that carries the attribute itself, so every
            part of the strip that is not a control carries it, and decoration lets clicks
            through. Double-clicking it maximizes, as on the app's own title bar.
        -->
        <header
            data-tauri-drag-region
            class="relative z-[1] grid h-12 shrink-0 grid-cols-[1fr_auto_1fr] items-stretch pl-4"
        >
            <div data-tauri-drag-region class="flex items-center gap-2">
                <span class="pointer-events-none flex h-6"><AxolotlLogo icon-only /></span>
                <span
                    data-tauri-drag-region
                    class="text-sm font-semibold text-[var(--color-text-tertiary)]"
                    >Axolotl Launcher</span
                >
            </div>
            <ol
                v-if="railSteps.length > 1 && step.layout === 'card'"
                data-tauri-drag-region
                class="m-0 flex list-none items-center gap-1 p-0"
                :aria-label="formatMessage(messages.progress)"
            >
                <li
                    v-for="entry in railSteps"
                    :key="entry.id"
                    data-tauri-drag-region
                    class="oobe-rail-item flex items-center gap-1.5 rounded-full px-3 py-1 text-sm font-medium"
                    :data-state="railState(entry)"
                    :aria-current="railState(entry) === 'current' ? 'step' : undefined"
                >
                    <CheckIcon
                        v-if="railState(entry) === 'done'"
                        class="pointer-events-none size-3.5"
                    />
                    <span v-else class="oobe-rail-dot pointer-events-none size-1.5 rounded-full" />
                    {{ formatMessage(entry.label) }}
                </li>
            </ol>
            <span v-else data-tauri-drag-region />
            <div data-tauri-drag-region class="flex items-center justify-end">
                <WindowControls />
            </div>
        </header>

        <main class="relative z-[1] flex min-h-0 flex-1 items-center justify-center px-8 pb-10">
            <Transition name="oobe-stage" mode="out-in" appear @after-enter="focusHeading">
                <section
                    v-if="step.layout === 'card'"
                    key="card"
                    class="oobe-card grid h-[min(44rem,calc(100dvh-6rem))] w-[min(68rem,100%)] grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)] grid-rows-[minmax(0,1fr)] overflow-hidden rounded-[var(--radius-xl)] border border-solid border-surface-4 bg-surface-2"
                >
                    <div class="oobe-art-panel relative overflow-hidden" aria-hidden="true">
                        <Transition name="oobe-art" mode="out-in">
                            <component :is="step.art" :key="step.id" class="absolute inset-0" />
                        </Transition>
                    </div>
                    <div class="flex min-h-0 flex-col px-10 pb-8 pt-10">
                        <Transition
                            :name="`oobe-slide-${direction}`"
                            mode="out-in"
                            @after-enter="focusHeading"
                        >
                            <div :key="step.id" class="flex min-h-0 flex-1 flex-col gap-6">
                                <div class="flex flex-col gap-2">
                                    <h1
                                        ref="heading"
                                        tabindex="-1"
                                        class="m-0 text-[1.875rem] font-semibold leading-tight text-[var(--color-text-primary)] outline-none"
                                    >
                                        {{ formatMessage(step.title) }}
                                    </h1>
                                    <p
                                        v-if="step.description"
                                        class="m-0 text-base text-[var(--color-text-tertiary)]"
                                    >
                                        {{ formatMessage(step.description) }}
                                    </p>
                                </div>
                                <div class="oobe-scroll -mx-2 min-h-0 flex-1 overflow-y-auto px-2">
                                    <OobeStepScope :step-id="step.id" :controls="controls">
                                        <component :is="step.component" />
                                    </OobeStepScope>
                                </div>
                            </div>
                        </Transition>
                        <footer class="flex items-center gap-2 pt-6">
                            <Button
                                v-if="index > 0"
                                type="quiet"
                                :disabled="state.busy"
                                @click="goBack"
                            >
                                <LeftArrowIcon />
                                {{ formatMessage(messages.back) }}
                            </Button>
                            <span class="flex-1" />
                            <Button
                                v-if="step.skippable || state.ready"
                                :type="skipping ? 'base' : 'colored'"
                                color="brand"
                                size="lg"
                                :disabled="state.busy"
                                @click="advance"
                            >
                                <SpinnerIcon v-if="state.busy" class="animate-spin" />
                                {{
                                    formatMessage(
                                        skipping
                                            ? messages.skip
                                            : isLast
                                              ? messages.finish
                                              : messages.next,
                                    )
                                }}
                                <RightArrowIcon v-if="!state.busy" />
                            </Button>
                            <!-- A step that needs input first shows Next as plainly unavailable, not as a dimmed accent. -->
                            <Button v-else size="lg" disabled>
                                {{ formatMessage(isLast ? messages.finish : messages.next) }}
                                <RightArrowIcon />
                            </Button>
                        </footer>
                    </div>
                </section>
                <section v-else key="hero" class="flex h-full w-full">
                    <OobeStepScope :step-id="step.id" :controls="controls">
                        <component :is="step.component" :title="step.title" />
                    </OobeStepScope>
                </section>
            </Transition>
        </main>
    </div>
</template>

<style scoped lang="scss">
.oobe-glow {
    position: absolute;
    width: 60vmax;
    height: 60vmax;
    border-radius: 50%;
    filter: blur(80px);
    opacity: 0.55;
    animation: oobe-drift 24s ease-in-out infinite alternate;
}

.oobe-glow-a {
    top: -30vmax;
    left: -18vmax;
    background: radial-gradient(
        circle,
        color-mix(in srgb, var(--color-brand) 30%, transparent),
        transparent 65%
    );
}

.oobe-glow-b {
    right: -22vmax;
    bottom: -34vmax;
    background: radial-gradient(
        circle,
        color-mix(in srgb, var(--color-blue) 22%, transparent),
        transparent 65%
    );
    animation-delay: -12s;
}

@keyframes oobe-drift {
    from {
        transform: translate3d(0, 0, 0) scale(1);
    }
    to {
        transform: translate3d(6vmax, 4vmax, 0) scale(1.08);
    }
}

.oobe-card {
    box-shadow: var(--shadow-inset-sm), var(--shadow-floating);
}

.oobe-art-panel {
    background:
        radial-gradient(
            120% 90% at 30% 20%,
            color-mix(in srgb, var(--color-brand) 16%, transparent),
            transparent 70%
        ),
        var(--surface-1);
}

.oobe-rail-item {
    color: var(--color-text-tertiary);
    transition:
        color 200ms ease,
        background-color 200ms ease;

    &[data-state='current'] {
        background: color-mix(in srgb, var(--color-brand) 14%, transparent);
        color: var(--color-text-primary);
    }

    &[data-state='done'] {
        color: var(--color-brand);
    }
}

.oobe-rail-dot {
    background: currentColor;
    opacity: 0.6;
}

.oobe-scroll {
    scrollbar-width: thin;
}

.oobe-stage-enter-active,
.oobe-stage-leave-active {
    transition:
        opacity 320ms ease,
        transform 420ms cubic-bezier(0.22, 1, 0.36, 1);
}

.oobe-stage-enter-from {
    opacity: 0;
    transform: translateY(16px) scale(0.98);
}

.oobe-stage-leave-to {
    opacity: 0;
    transform: scale(0.98);
}

.oobe-art-enter-active,
.oobe-art-leave-active {
    transition:
        opacity 260ms ease,
        transform 420ms cubic-bezier(0.22, 1, 0.36, 1);
}

.oobe-art-enter-from {
    opacity: 0;
    transform: scale(0.94);
}

.oobe-art-leave-to {
    opacity: 0;
    transform: scale(1.04);
}

.oobe-slide-forward-enter-active,
.oobe-slide-forward-leave-active,
.oobe-slide-back-enter-active,
.oobe-slide-back-leave-active {
    transition:
        opacity 200ms ease,
        transform 300ms cubic-bezier(0.22, 1, 0.36, 1);
}

.oobe-slide-forward-enter-from,
.oobe-slide-back-leave-to {
    opacity: 0;
    transform: translateX(24px);
}

.oobe-slide-forward-leave-to,
.oobe-slide-back-enter-from {
    opacity: 0;
    transform: translateX(-24px);
}

@media (prefers-reduced-motion: reduce) {
    .oobe-glow {
        animation: none;
    }

    .oobe-stage-enter-active,
    .oobe-stage-leave-active,
    .oobe-art-enter-active,
    .oobe-art-leave-active,
    .oobe-slide-forward-enter-active,
    .oobe-slide-forward-leave-active,
    .oobe-slide-back-enter-active,
    .oobe-slide-back-leave-active {
        transition: none;
    }
}
</style>
