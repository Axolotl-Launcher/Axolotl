<script setup lang="ts">
import { CheckCircleIcon, SparklesIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import { modTranslationMessages, modTranslationPhaseSteps } from '@/lab/mod-translation/i18n'
import { phaseIndex } from '@/lab/mod-translation/job-state'
import type { ModTranslationJob, ModTranslationPhase } from '@/lab/mod-translation/types.ts'

const props = defineProps<{ job: ModTranslationJob }>()
const { formatMessage } = useVIntl()

const messages = defineMessages({
    workloadMeasuring: {
        id: 'app.lab.mod-translation.progress.workload-measuring',
        defaultMessage: 'Establishing a verifiable workload…',
    },
    verifiedWeights: {
        id: 'app.lab.mod-translation.progress.verified-weights',
        defaultMessage: 'Verified {verified} / {total}',
    },
    currentBatch: {
        id: 'app.lab.mod-translation.progress.current-batch',
        defaultMessage: 'Batch {completed} / {total}',
    },
    recentlyWritten: {
        id: 'app.lab.mod-translation.progress.recently-written',
        defaultMessage: 'Recently written',
    },
    progressAriaLabel: {
        id: 'app.lab.mod-translation.progress.aria-label',
        defaultMessage: 'Task progress',
    },
})

const activeIndex = computed(() => phaseIndex(props.job.phase))
const measurable = computed(() => props.job.weightTotal > 0)
const verificationLabel = computed(() => {
    if (!measurable.value) return formatMessage(messages.workloadMeasuring)
    return formatMessage(messages.verifiedWeights, {
        verified: formatWeight(props.job.weightVerified),
        total: formatWeight(props.job.weightTotal),
    })
})
const itemLabel = computed(() => {
    if (props.job.total <= 0) return undefined
    return formatMessage(messages.currentBatch, {
        completed: props.job.completed.toLocaleString(),
        total: props.job.total.toLocaleString(),
    })
})

function stepState(step: ModTranslationPhase): 'done' | 'current' | 'failed' | 'pending' {
    const index = phaseIndex(step)
    if (props.job.status === 'completed') return 'done'
    if (props.job.status === 'failed' && index === activeIndex.value) return 'failed'
    if (index < activeIndex.value) return 'done'
    if (index === activeIndex.value) return 'current'
    return 'pending'
}

function formatWeight(value: number): string {
    return Number.isInteger(value) ? value.toLocaleString() : value.toFixed(1)
}
</script>

<template>
    <section class="progress-section" :aria-label="formatMessage(messages.progressAriaLabel)">
        <div class="stepper">
            <template v-for="(step, index) in modTranslationPhaseSteps" :key="step.id">
                <span
                    v-if="index > 0"
                    class="connector"
                    :class="{ 'connector--on': index <= activeIndex || job.status === 'completed' }"
                />
                <span
                    class="step"
                    :class="`step--${stepState(step.id)}`"
                    :title="formatMessage(step.label)"
                >
                    <component :is="step.icon" />
                </span>
            </template>
        </div>

        <div
            class="track"
            :class="{ 'track--indeterminate': !measurable && job.status === 'running' }"
            role="progressbar"
            :aria-valuenow="measurable ? job.percent : undefined"
            aria-valuemin="0"
            aria-valuemax="100"
            :aria-valuetext="verificationLabel"
        >
            <div
                class="fill"
                :class="[`fill--${job.level}`, { 'fill--failed': job.status === 'failed' }]"
                :style="{ width: measurable ? `${job.percent}%` : '36%' }"
            />
        </div>

        <div
            class="live"
            :class="[`live--${job.level}`, { 'live--failed': job.status === 'failed' }]"
        >
            <span v-if="job.status === 'running'" class="live-dot" />
            <strong>{{ job.message || formatMessage(modTranslationMessages.preparing) }}</strong>
            <span class="verification max-sm:hidden">{{ verificationLabel }}</span>
        </div>

        <div v-if="itemLabel" class="stats">
            <span class="stat-pill"><CheckCircleIcon />{{ itemLabel }}</span>
        </div>

        <div v-if="job.sample" class="sample">
            <span class="sample-label"
                ><SparklesIcon />{{ formatMessage(messages.recentlyWritten) }}</span
            >
            <div class="sample-row">
                <span :title="job.sample.source">{{ job.sample.source }}</span>
                <span class="sample-arrow">→</span>
                <strong :title="job.sample.translation">{{ job.sample.translation }}</strong>
            </div>
        </div>
    </section>
</template>

<style scoped>
.progress-section {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
}

.stepper {
    display: flex;
    align-items: center;
}

.step {
    display: grid;
    width: 1.7rem;
    height: 1.7rem;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 999px;
    background: var(--surface-4);
    color: var(--color-text-tertiary);
    transition:
        background 0.25s ease,
        color 0.25s ease;
}

.step :deep(svg) {
    width: 0.85rem;
    height: 0.85rem;
}

.step--current {
    background: color-mix(in srgb, var(--color-brand) 18%, var(--surface-4));
    color: var(--color-brand);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-brand) 10%, transparent);
}

.step--done {
    background: color-mix(in srgb, var(--color-green) 16%, var(--surface-4));
    color: var(--color-green);
}

.step--failed {
    background: color-mix(in srgb, var(--color-red) 16%, var(--surface-4));
    color: var(--color-red);
}

.connector {
    height: 2px;
    min-width: 0.45rem;
    flex: 1;
    background: var(--color-divider);
    transition: background 0.25s ease;
}

.connector--on {
    background: color-mix(in srgb, var(--color-green) 65%, var(--color-divider));
}

.track {
    height: 0.42rem;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-4);
}

.fill {
    height: 100%;
    border-radius: inherit;
    background: var(--color-brand);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.22);
    transition: width 0.35s ease;
}

.fill--warn {
    background: var(--color-orange);
}

.fill--error,
.fill--failed {
    background: var(--color-red);
}

.track--indeterminate .fill {
    animation: indeterminate 1.25s ease-in-out infinite;
}

@keyframes indeterminate {
    0% {
        transform: translateX(-110%);
    }
    100% {
        transform: translateX(310%);
    }
}

.live {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 0.45rem;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--color-brand) 8%, var(--surface-4));
    padding: 0.5rem 0.65rem;
}

.live--warn {
    background: color-mix(in srgb, var(--color-orange) 9%, var(--surface-4));
}

.live--error,
.live--failed {
    background: color-mix(in srgb, var(--color-red) 9%, var(--surface-4));
}

.live-dot {
    width: 0.45rem;
    height: 0.45rem;
    flex: 0 0 auto;
    border-radius: 999px;
    background: var(--color-brand);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-brand) 15%, transparent);
}

.live strong {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    font-size: 0.73rem;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-primary);
}

.verification {
    flex: 0 0 auto;
    font-size: 0.66rem;
    font-variant-numeric: tabular-nums;
    color: var(--color-text-tertiary);
}

.stats {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
}

.stat-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    border-radius: 999px;
    background: var(--surface-4);
    padding: 0.2rem 0.55rem;
    font-size: 0.65rem;
    font-weight: 700;
    color: var(--color-text-tertiary);
}

.stat-pill :deep(svg),
.sample-label :deep(svg) {
    width: 0.78rem;
    height: 0.78rem;
}

.sample {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    border: 1px solid color-mix(in srgb, var(--color-brand) 18%, var(--color-divider));
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--color-brand) 5%, var(--surface-4));
    padding: 0.55rem 0.65rem;
}

.sample-label {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.62rem;
    font-weight: 800;
    text-transform: uppercase;
    color: var(--color-brand);
}

.sample-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 0.45rem;
    font-size: 0.72rem;
}

.sample-row span,
.sample-row strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.sample-row strong {
    color: var(--color-text-primary);
}

.sample-arrow {
    color: var(--color-brand);
}
</style>
