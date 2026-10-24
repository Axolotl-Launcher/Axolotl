<template>
    <FloatingActionBar
        :shown="true"
        :aria-label="formatMessage(messages.aria)"
        hide-when-modal-open
        allow-overflow
    >
        <div
            class="grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-3"
        >
            <div class="justify-self-start">
                <Button
                    v-if="!progress.complete && controls"
                    type="outlined"
                    size="2xs"
                    :disabled="!controls"
                    @click="controls?.onBack()"
                    ><ArrowLeftIcon aria-hidden="true" />
                    <span class="bar-label">{{ formatMessage(messages.back) }}</span>
                </Button>
            </div>

            <span
                class="relative min-w-0 flex-1 select-none"
                tabindex="0"
                :aria-label="formatMessage(messages.steps)"
                @mouseenter="progressOpen = true"
                @mouseleave="progressOpen = false"
                @focus="progressOpen = true"
                @blur="progressOpen = false"
            >
                <span
                    class="flex items-center justify-center gap-1.5 truncate text-center text-sm text-[var(--color-text-tertiary)]"
                >
                    <CheckCircleIcon
                        v-if="progress.complete"
                        class="size-4 shrink-0 text-green"
                        aria-hidden="true"
                    />
                    <template v-if="progress.complete">{{
                        formatMessage(messages.complete)
                    }}</template>
                    <template v-else>
                        {{ progress.currentIndex + 1 }} / {{ progress.steps.length }} ·
                        {{ formatMessage(stepLabels[progress.currentIndex]) }}
                    </template>
                </span>
                <div
                    v-if="progressOpen"
                    class="absolute bottom-[calc(100%+0.75rem)] left-1/2 z-10 flex w-max max-w-[calc(100vw-2rem)] -translate-x-1/2 flex-col gap-2 rounded-md border border-solid border-surface-5 bg-surface-3 px-3 py-2 text-sm shadow-lg"
                    style="background-color: var(--color-tooltip-bg)"
                >
                    <div
                        v-for="(step, index) in progress.steps"
                        :key="step"
                        class="flex items-center gap-2 whitespace-nowrap"
                        :class="stepClass(index)"
                    >
                        <CheckCircleIcon
                            v-if="progress.complete || index < progress.currentIndex"
                            class="size-4 shrink-0"
                            aria-hidden="true"
                        />
                        <span
                            v-else
                            class="size-3 shrink-0 rounded-full border-2 border-solid border-current"
                            :class="{ 'bg-current': index === progress.currentIndex }"
                            aria-hidden="true"
                        />
                        {{ formatMessage(stepLabels[index]) }}
                    </div>
                </div>
            </span>

            <div class="justify-self-end">
                <span v-tooltip="blockerTooltip" tabindex="0" :aria-label="blockerTooltip">
                    <Button
                        v-if="!progress.complete && controls"
                        type="colored"
                        color="brand"
                        size="2xs"
                        :disabled="!controls || !canNext || busy"
                        @click="controls?.onNext()"
                        ><SpinnerIcon v-if="busy" class="animate-spin" aria-hidden="true" />
                        <CircleArrowRightIcon v-else aria-hidden="true" />
                        <span class="bar-label">{{
                            controls?.nextLabel ?? formatMessage(messages.next)
                        }}</span>
                    </Button>
                </span>
            </div>
        </div>
    </FloatingActionBar>
</template>

<script setup lang="ts">
import './messages'

import ArrowLeftIcon from '@modrinth/assets/icons/arrow-left.svg?component'
import CheckCircleIcon from '@modrinth/assets/icons/check-circle.svg?component'
import CircleArrowRightIcon from '@modrinth/assets/icons/circle-arrow-right.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'

import FloatingActionBar from '../../../overlays/PreviewFloatingActionBar.vue'
import { isSharedUpgradeInstance } from './analysis'
import { useInstanceUpgradeFlow } from './flow'
import { upgradeControlEnabled, upgradeProgressModel } from './flow-controls'

const messages = defineMessages({
    aria: { id: 'instance.upgrade.flow.aria', defaultMessage: 'Instance upgrade navigation' },
    back: { id: 'instance.upgrade.flow.back', defaultMessage: 'Previous' },
    next: { id: 'instance.upgrade.flow.next', defaultMessage: 'Next' },
    steps: { id: 'instance.upgrade.flow.steps', defaultMessage: 'Upgrade steps' },
    target: { id: 'instance.upgrade.flow.target', defaultMessage: 'Upgrade target' },
    issues: { id: 'instance.upgrade.flow.issues', defaultMessage: 'Resolve issues' },
    preferences: { id: 'instance.upgrade.flow.preferences', defaultMessage: 'Upgrade preferences' },
    confirm: { id: 'instance.upgrade.flow.confirm', defaultMessage: 'Confirm upgrade' },
    progress: { id: 'instance.upgrade.flow.progress', defaultMessage: 'Upgrading' },
    complete: { id: 'instance.upgrade.flow.complete', defaultMessage: 'Upgrade complete' },
    resolveBlockers: {
        id: 'instance.upgrade.compatibility.resolve-blockers-tooltip',
        defaultMessage: 'Please resolve all blocking items before continuing.',
    },
    chooseSharedMode: {
        id: 'instance.upgrade.confirm.choose-shared-mode-tooltip',
        defaultMessage: 'Choose how this shared instance should be upgraded.',
    },
})
const flow = useInstanceUpgradeFlow()
const route = useRoute()
const { formatMessage } = useVIntl()
const progressOpen = ref(false)
const stepLabels = [
    messages.target,
    messages.issues,
    messages.preferences,
    messages.confirm,
    messages.progress,
]
const progress = computed(() => upgradeProgressModel(route.path))
const controls = computed(() => flow.controls.value)
const canNext = computed(() => upgradeControlEnabled(flow.controls.value?.canNext))
const busy = computed(() => upgradeControlEnabled(flow.controls.value?.busy))
const showBlockerTooltip = computed(
    () =>
        route.path.endsWith('/upgrade/compatibility') &&
        (flow.plan.value?.blockingIssues.length ?? 0) > 0 &&
        !busy.value,
)
const blockerTooltip = computed(() => {
    if (
        route.path.endsWith('/upgrade/confirm') &&
        flow.instance.value &&
        flow.sharedUpgradeMode.value === null &&
        isSharedUpgradeInstance(flow.instance.value)
    ) {
        return formatMessage(messages.chooseSharedMode)
    }
    return showBlockerTooltip.value ? formatMessage(messages.resolveBlockers) : undefined
})

function stepClass(index: number) {
    if (progress.value.complete || index < progress.value.currentIndex) return 'text-green'
    if (index === progress.value.currentIndex) return 'font-semibold text-brand'
    return 'text-[var(--color-text-tertiary)]'
}
</script>
