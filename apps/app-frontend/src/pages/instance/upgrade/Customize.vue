<template>
    <section class="flex flex-col gap-6 py-2">
        <header>
            <h2 class="m-0 text-xl font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h2>
            <p class="mb-0 mt-1 max-w-2xl text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
        </header>

        <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            <button
                v-if="availableStrategies.includes('newest') && plan.newestSolution"
                type="button"
                class="flex min-h-36 flex-col gap-2 rounded-lg border border-solid p-4 text-left transition-colors"
                :class="strategyClass('newest')"
                :disabled="requestBusy"
                @click="chooseStrategy('newest')"
            >
                <div class="flex items-center gap-2 font-semibold text-[var(--color-text-primary)]">
                    <SparklesIcon aria-hidden="true" />
                    {{ formatMessage(messages.newestTitle) }}
                    <CheckIcon
                        v-if="activeStrategy === 'newest'"
                        class="ml-auto text-brand"
                        aria-hidden="true"
                    />
                </div>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.newestDescription) }}
                </p>
                <span class="mt-auto text-sm text-[var(--color-text-tertiary)]">{{
                    summaryText(plan.newestSolution)
                }}</span>
            </button>

            <button
                v-if="availableStrategies.includes('minimal_change') && plan.minimalChangeSolution"
                type="button"
                class="flex min-h-36 flex-col gap-2 rounded-lg border border-solid p-4 text-left transition-colors"
                :class="strategyClass('minimal_change')"
                :disabled="requestBusy"
                @click="chooseStrategy('minimal_change')"
            >
                <div class="flex items-center gap-2 font-semibold text-[var(--color-text-primary)]">
                    <MinimizeIcon aria-hidden="true" />
                    {{ formatMessage(messages.minimalTitle) }}
                    <CheckIcon
                        v-if="activeStrategy === 'minimal_change'"
                        class="ml-auto text-brand"
                        aria-hidden="true"
                    />
                </div>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.minimalDescription) }}
                </p>
                <span class="mt-auto text-sm text-[var(--color-text-tertiary)]">{{
                    summaryText(plan.minimalChangeSolution)
                }}</span>
            </button>

            <button
                type="button"
                class="flex min-h-36 flex-col gap-2 rounded-lg border border-solid p-4 text-left transition-colors"
                :class="strategyClass('custom')"
                :disabled="requestBusy"
                @click="chooseStrategy('custom')"
            >
                <div class="flex items-center gap-2 font-semibold text-[var(--color-text-primary)]">
                    <SettingsIcon aria-hidden="true" />
                    {{ formatMessage(messages.customTitle) }}
                    <CheckIcon
                        v-if="activeStrategy === 'custom'"
                        class="ml-auto text-brand"
                        aria-hidden="true"
                    />
                </div>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.customDescription) }}
                </p>
                <span class="mt-auto text-sm text-[var(--color-text-tertiary)]">
                    {{
                        formatMessage(messages.customConstraintCount, {
                            count: draftConstraints.length,
                        })
                    }}
                </span>
            </button>
        </div>

        <Admonition
            v-if="pendingStrategy"
            type="warning"
            :header="formatMessage(messages.unsavedTitle)"
        >
            <div class="flex flex-col gap-3">
                <span>{{ formatMessage(messages.unsavedBody) }}</span>
                <div class="flex flex-wrap gap-2">
                    <Button type="outlined" size="2xs" @click="pendingStrategy = null">{{
                        formatMessage(messages.cancel)
                    }}</Button>
                    <Button type="colored" color="orange" size="2xs" @click="discardAndSwitch"
                        >{{ formatMessage(messages.discardAndSwitch) }}
                    </Button>
                </div>
            </div>
        </Admonition>

        <Admonition
            v-if="requestError"
            type="critical"
            :header="formatMessage(messages.requestErrorTitle)"
        >
            {{ requestError }}
        </Admonition>

        <section v-if="activeStrategy === 'custom'" class="flex flex-col gap-3">
            <div class="flex flex-wrap items-end justify-between gap-3">
                <div>
                    <h3 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.customChoices) }}
                    </h3>
                    <p class="mb-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.customChoicesDescription) }}
                    </p>
                </div>
                <Button
                    type="colored"
                    color="brand"
                    :disabled="!canApplyCustom"
                    @click="applyCustomChoices"
                    ><SpinnerIcon v-if="requestBusy" class="animate-spin" aria-hidden="true" />
                    <RefreshCwIcon v-else aria-hidden="true" />
                    {{
                        formatMessage(
                            customWasResolved ? messages.recalculate : messages.applyCustom,
                        )
                    }}
                </Button>
            </div>

            <Admonition
                v-if="customDraftDirty"
                type="warning"
                :header="formatMessage(messages.unappliedTitle)"
            >
                {{ formatMessage(messages.unappliedBody) }}
            </Admonition>

            <div class="rounded-lg border border-solid border-surface-4">
                <article
                    v-for="item in editableRoots"
                    :key="item.contentId"
                    class="relative flex flex-col gap-3 border-0 border-b border-solid border-surface-4 bg-surface-2 p-3 first:rounded-t-lg last:rounded-b-lg last:border-b-0 focus-within:z-20 sm:flex-row sm:items-center"
                >
                    <div class="flex min-w-0 flex-1 items-center gap-3">
                        <Avatar
                            :src="itemIcon(item)"
                            :tint-by="itemName(item)"
                            size="2.5rem"
                            no-shadow
                        />
                        <div class="min-w-0">
                            <RouterLink
                                v-if="projectPath(item)"
                                :to="projectPath(item)!"
                                class="inline-flex max-w-full items-center gap-1 font-semibold text-[var(--color-text-primary)] hover:text-brand hover:underline focus-visible:underline"
                                @click="parkProjectReturn"
                                ><span class="truncate">{{ itemName(item) }}</span
                                ><ExternalIcon class="size-3 shrink-0" aria-hidden="true"
                            /></RouterLink>
                            <div
                                v-else
                                class="truncate font-semibold text-[var(--color-text-primary)]"
                            >
                                {{ itemName(item) }}
                            </div>
                            <div
                                class="flex flex-wrap gap-x-3 text-sm text-[var(--color-text-tertiary)]"
                            >
                                <span>{{ providerLabel(item.provider) }}</span>
                                <span>
                                    <UpgradeVersionChangelogPopout
                                        v-if="item.currentReleaseId"
                                        :label="currentVersionLabel(item)"
                                        :provider="item.provider"
                                        :project-id="item.projectId"
                                        :release-id="item.currentReleaseId"
                                    />
                                    <span v-else>{{ currentVersionLabel(item) }}</span>
                                </span>
                                <span v-if="!item.currentEnabled">{{
                                    formatMessage(messages.currentlyDisabled)
                                }}</span>
                            </div>
                            <div class="mt-1 text-sm text-[var(--color-text-tertiary)]">
                                <span>{{ formatMessage(messages.effectiveTargetPrefix) }}</span>
                                <UpgradeVersionChangelogPopout
                                    v-if="effectiveTargetRelease(item)"
                                    :label="effectiveTargetVersion(item)"
                                    :provider="item.provider"
                                    :project-id="item.projectId"
                                    :release-id="effectiveTargetRelease(item)"
                                />
                                <span v-else>{{ formatMessage(messages.noTarget) }}</span>
                            </div>
                        </div>
                    </div>
                    <div class="min-w-0 w-full shrink-0 sm:w-72 sm:max-w-[45%]">
                        <label
                            class="mb-1 block text-sm font-medium text-[var(--color-text-primary)]"
                            :for="`custom-${item.contentId}`"
                        >
                            {{ formatMessage(messages.choice) }}
                        </label>
                        <Combobox
                            width="full"
                            class="max-w-full min-w-0"
                            :model-value="draftChoice(item.contentId)"
                            :options="
                                constraintOptions(item).map((value) => ({
                                    value,
                                    label: constraintOptionLabel(item, value),
                                }))
                            "
                            :disabled="requestBusy"
                            @update:model-value="setDraftChoice(item, String($event))"
                        />
                    </div>
                </article>
            </div>
        </section>

        <section v-if="customIssues.length" class="flex flex-col gap-2">
            <h3 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.customIssues) }}
            </h3>
            <Admonition
                v-for="(issue, index) in customIssues"
                :key="`${issue.code}:${issue.contentId ?? issue.projectId ?? index}`"
                :type="issue.code === 'search_limit_reached' ? 'warning' : 'critical'"
                :header="customIssueTitle(issue)"
            >
                {{ customIssueBody(issue) }}
            </Admonition>
        </section>

        <section v-if="effectiveSolution" class="flex flex-col gap-3">
            <div>
                <h3 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.effectiveChanges) }}
                </h3>
                <p class="mb-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                    {{
                        activeStrategy === 'custom' && !customWasResolved
                            ? formatMessage(messages.baselineHint)
                            : summaryText(effectiveSolution)
                    }}
                </p>
            </div>

            <div
                class="grid grid-cols-2 gap-px overflow-hidden rounded-lg bg-surface-4 sm:grid-cols-3 lg:grid-cols-6"
            >
                <div
                    v-for="metric in effectiveMetrics"
                    :key="metric.label"
                    class="bg-surface-2 p-3"
                >
                    <div class="text-xl font-semibold text-[var(--color-text-primary)]">
                        {{ metric.value }}
                    </div>
                    <div class="text-sm text-[var(--color-text-tertiary)]">{{ metric.label }}</div>
                </div>
            </div>

            <div v-if="effectiveDependencyChanges.length" class="flex flex-col gap-2">
                <h4 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.dependenciesTitle) }}
                </h4>
                <div class="overflow-hidden rounded-lg border border-solid border-surface-4">
                    <div
                        v-for="change in effectiveDependencyChanges"
                        :key="`${change.provider}:${change.projectId}:${change.existingContentId ?? 'new'}`"
                        class="flex items-center justify-between gap-4 border-0 border-b border-solid border-surface-4 bg-surface-2 p-3 last:border-b-0"
                    >
                        <div class="min-w-0">
                            <div class="truncate font-semibold text-[var(--color-text-primary)]">
                                {{ change.projectId }}
                            </div>
                            <div class="text-sm text-[var(--color-text-tertiary)]">
                                {{ dependencyChangeDescription(change) }}
                            </div>
                        </div>
                        <strong class="shrink-0 text-sm text-[var(--color-text-primary)]">{{
                            dependencyActionLabel(change.kind)
                        }}</strong>
                    </div>
                </div>
            </div>
        </section>
    </section>
</template>

<script setup lang="ts">
import {
    CheckIcon,
    ExternalIcon,
    MinimizeIcon,
    RefreshCwIcon,
    SettingsIcon,
    SparklesIcon,
    SpinnerIcon,
} from '@modrinth/assets'
import { Admonition, Avatar, Button, Combobox, defineMessages, useVIntl } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import type { InstanceContentSnapshotItem } from '@/helpers/instance'
import {
    type InstanceContentData,
    loadInstanceContentData,
    localContentIconUrl,
} from '@/helpers/instance-content'
import type {
    ContentProvider,
    InstanceUpgradeDependencyChange,
    InstanceUpgradeDependencyChangeKind,
    InstanceUpgradeFixedConstraint,
    InstanceUpgradeIssue,
    InstanceUpgradePlanItem,
    InstanceUpgradeSolution,
    InstanceUpgradeSolutionKind,
} from '@/helpers/instance-upgrade'
import {
    resolve_custom_instance_upgrade_solution,
    select_instance_upgrade_solution,
} from '@/helpers/instance-upgrade'
import {
    loadUpgradeVersionDisplayMetadata,
    type UpgradeReleaseIdentity,
    upgradeVersionCacheKey,
    upgradeVersionDisplayLabel,
} from '@/helpers/upgrade-version-metadata'
import { parkUpgradeFlow, upgradeProjectPath } from '@/store/navigation-return'

import {
    availablePredefinedStrategies,
    contentIdentityKeys,
    customConstraintsEqual,
    editableUpgradeRoots,
    normalizeUpgradePath,
    setFixedConstraint,
    solutionSummary,
    upgradeContentDisplayMetadata,
} from './analysis'
import { useInstanceUpgradeFlow } from './flow'
import { initialCustomizeStrategy } from './flow-controls'
import UpgradeVersionChangelogPopout from './UpgradeVersionChangelogPopout.vue'

const AUTOMATIC = '__automatic__'

const messages = defineMessages({
    title: { id: 'instance.upgrade.customize.title', defaultMessage: 'Upgrade strategy' },
    description: {
        id: 'instance.upgrade.customize.description',
        defaultMessage: 'Choose how aggressively Axolotl should update content in this instance.',
    },
    newestTitle: {
        id: 'instance.upgrade.customize.newest.title',
        defaultMessage: 'Newest versions',
    },
    newestDescription: {
        id: 'instance.upgrade.customize.newest.description',
        defaultMessage:
            'Update compatible content to the newest versions available for the target environment.',
    },
    minimalTitle: {
        id: 'instance.upgrade.customize.minimal.title',
        defaultMessage: 'Minimal changes',
    },
    minimalDescription: {
        id: 'instance.upgrade.customize.minimal.description',
        defaultMessage:
            'Keep compatible current versions and change as little installed content as possible.',
    },
    customTitle: { id: 'instance.upgrade.customize.custom.title', defaultMessage: 'Custom' },
    customDescription: {
        id: 'instance.upgrade.customize.custom.description',
        defaultMessage:
            'Fix exact versions for selected content and let Axolotl solve the remaining dependency graph.',
    },
    customConstraintCount: {
        id: 'instance.upgrade.customize.custom.constraint-count',
        defaultMessage: '{count, plural, one {# exact choice} other {# exact choices}}',
    },
    rootSummary: {
        id: 'instance.upgrade.customize.summary.roots',
        defaultMessage:
            '{updates} updates, {kept} kept, {disabled} disabled, {dependencies, plural, one {# dependency change} other {# dependency changes}}',
    },
    customChoices: {
        id: 'instance.upgrade.customize.custom.choices',
        defaultMessage: 'Custom choices',
    },
    customChoicesDescription: {
        id: 'instance.upgrade.customize.custom.choices-description',
        defaultMessage:
            'Only user-owned root content is editable. Dependencies remain solver-managed.',
    },
    choice: { id: 'instance.upgrade.customize.custom.choice', defaultMessage: 'Target version' },
    automatic: { id: 'instance.upgrade.customize.custom.automatic', defaultMessage: 'Automatic' },
    specificVersion: {
        id: 'instance.upgrade.customize.custom.specific-version',
        defaultMessage: 'Exact release {version}',
    },
    specificVersionWithChannel: {
        id: 'instance.upgrade.customize.custom.specific-version-channel',
        defaultMessage: '{version} ({channel})',
    },
    channelRelease: { id: 'instance.upgrade.customize.channel.release', defaultMessage: 'Release' },
    channelBeta: { id: 'instance.upgrade.customize.channel.beta', defaultMessage: 'Beta' },
    channelAlpha: { id: 'instance.upgrade.customize.channel.alpha', defaultMessage: 'Alpha' },
    applyCustom: {
        id: 'instance.upgrade.customize.custom.apply',
        defaultMessage: 'Apply custom choices',
    },
    recalculate: {
        id: 'instance.upgrade.customize.custom.recalculate',
        defaultMessage: 'Recalculate',
    },
    unappliedTitle: {
        id: 'instance.upgrade.customize.custom.unapplied-title',
        defaultMessage: 'Custom choices have not been applied',
    },
    unappliedBody: {
        id: 'instance.upgrade.customize.custom.unapplied-body',
        defaultMessage: 'Apply these choices to calculate a globally compatible solution.',
    },
    unsavedTitle: {
        id: 'instance.upgrade.customize.custom.unsaved-title',
        defaultMessage: 'Discard unapplied custom choices?',
    },
    unsavedBody: {
        id: 'instance.upgrade.customize.custom.unsaved-body',
        defaultMessage: 'Switching strategy will discard changes that have not been calculated.',
    },
    discardAndSwitch: {
        id: 'instance.upgrade.customize.custom.discard-switch',
        defaultMessage: 'Discard and switch',
    },
    cancel: { id: 'instance.upgrade.customize.cancel', defaultMessage: 'Cancel' },
    requestErrorTitle: {
        id: 'instance.upgrade.customize.request-error-title',
        defaultMessage: 'Strategy could not be updated',
    },
    customIssues: {
        id: 'instance.upgrade.customize.custom.issues',
        defaultMessage: 'Unable to resolve custom choices',
    },
    searchLimitTitle: {
        id: 'instance.upgrade.customize.search-limit.title',
        defaultMessage: 'Search limit reached',
    },
    searchLimitBody: {
        id: 'instance.upgrade.customize.search-limit.body',
        defaultMessage:
            "Axolotl couldn't find a solution within the search limit. Try relaxing one of your custom choices.",
    },
    conflictTitle: {
        id: 'instance.upgrade.customize.conflict.title',
        defaultMessage: 'Custom choices conflict',
    },
    effectiveChanges: {
        id: 'instance.upgrade.customize.effective-changes',
        defaultMessage: 'Effective changes',
    },
    dependenciesTitle: {
        id: 'instance.upgrade.customize.dependencies-title',
        defaultMessage: 'Dependency changes',
    },
    baselineHint: {
        id: 'instance.upgrade.customize.baseline-hint',
        defaultMessage:
            'Current selected solution shown as the baseline. Apply custom choices to recalculate it.',
    },
    metricUpdated: {
        id: 'instance.upgrade.customize.metric.updated',
        defaultMessage: 'Content updated',
    },
    metricKept: { id: 'instance.upgrade.customize.metric.kept', defaultMessage: 'Kept' },
    metricDisabled: {
        id: 'instance.upgrade.customize.metric.disabled',
        defaultMessage: 'Disabled',
    },
    metricAdded: {
        id: 'instance.upgrade.customize.metric.added',
        defaultMessage: 'Dependencies added',
    },
    metricDependencyUpdated: {
        id: 'instance.upgrade.customize.metric.dependency-updated',
        defaultMessage: 'Dependencies updated',
    },
    metricRemoved: {
        id: 'instance.upgrade.customize.metric.removed',
        defaultMessage: 'Dependencies removed',
    },
    currentVersion: {
        id: 'instance.upgrade.customize.current-version',
        defaultMessage: 'Current: {version}',
    },
    effectiveTarget: {
        id: 'instance.upgrade.customize.effective-target',
        defaultMessage: 'Calculated target: {version}',
    },
    effectiveTargetPrefix: {
        id: 'instance.upgrade.customize.effective-target-prefix',
        defaultMessage: 'Calculated target: ',
    },
    noTarget: { id: 'instance.upgrade.customize.no-target', defaultMessage: 'No target release' },
    currentlyDisabled: {
        id: 'instance.upgrade.customize.currently-disabled',
        defaultMessage: 'Currently disabled',
    },
    providerModrinth: { id: 'instance.upgrade.provider.modrinth', defaultMessage: 'Modrinth' },
    providerCurseForge: {
        id: 'instance.upgrade.provider.curseforge',
        defaultMessage: 'CurseForge',
    },
    providerUnknown: {
        id: 'instance.upgrade.provider.unknown',
        defaultMessage: 'Unknown provider',
    },
    dependencyAdd: {
        id: 'instance.upgrade.customize.dependency.add',
        defaultMessage: 'Add dependency',
    },
    dependencyUpgrade: {
        id: 'instance.upgrade.customize.dependency.upgrade',
        defaultMessage: 'Update dependency',
    },
    dependencyRemove: {
        id: 'instance.upgrade.customize.dependency.remove',
        defaultMessage: 'Remove dependency',
    },
    dependencyKeep: {
        id: 'instance.upgrade.customize.dependency.keep',
        defaultMessage: 'Keep dependency',
    },
    dependencyReused: {
        id: 'instance.upgrade.customize.dependency.reused',
        defaultMessage: 'Reuses existing content: {current} to {target}',
    },
    dependencyNew: {
        id: 'instance.upgrade.customize.dependency.new',
        defaultMessage: 'New content: {target}',
    },
    back: { id: 'instance.upgrade.customize.back', defaultMessage: 'Back' },
    continue: { id: 'instance.upgrade.customize.continue', defaultMessage: 'Continue' },
    applyBeforeContinue: {
        id: 'instance.upgrade.customize.apply-before-continue',
        defaultMessage: 'Apply your custom choices first.',
    },
    resolveBeforeContinue: {
        id: 'instance.upgrade.customize.resolve-before-continue',
        defaultMessage: 'Resolve custom conflicts before continuing.',
    },
})

const flow = useInstanceUpgradeFlow()
const router = useRouter()
const { formatMessage } = useVIntl()
const plan = computed(() => flow.plan.value!)
const activeStrategy = ref<InstanceUpgradeSolutionKind>(
    initialCustomizeStrategy(
        flow.customizeActiveStrategy.value,
        plan.value.selectedSolution?.kind,
        'custom',
    ),
)
flow.customizeActiveStrategy.value = activeStrategy.value
const draftConstraints = ref<InstanceUpgradeFixedConstraint[]>(
    plan.value.customConstraints.map((item) => ({ ...item })),
)
const pendingStrategy = ref<Exclude<InstanceUpgradeSolutionKind, 'custom'> | null>(null)
const requestBusy = ref(false)
const requestError = ref<string | null>(null)

const contentDataQuery = useQuery({
    queryKey: computed(() => ['instance-upgrade', 'content-data', flow.instanceId.value]),
    queryFn: () => loadInstanceContentData(flow.instanceId.value),
    staleTime: Number.POSITIVE_INFINITY,
})
const releaseIdentities = computed(() => {
    const identities: UpgradeReleaseIdentity[] = []
    const add = (
        provider: ContentProvider | null,
        projectId: string | null,
        releaseId: string | null,
    ) => {
        if ((provider === 'modrinth' || provider === 'curseforge') && projectId && releaseId) {
            identities.push({ provider, projectId, releaseId })
        }
    }
    for (const item of plan.value.items) {
        add(item.provider, item.projectId, item.currentReleaseId)
        item.candidateReleaseIds.forEach((releaseId) =>
            add(item.provider, item.projectId, releaseId),
        )
    }
    for (const selection of plan.value.selectedSolution?.selections ?? []) {
        add(selection.provider, selection.projectId, selection.targetReleaseId)
    }
    for (const change of plan.value.selectedSolution?.dependencyChanges ?? []) {
        add(change.provider, change.projectId, change.currentReleaseId)
        add(change.provider, change.projectId, change.targetReleaseId)
    }
    return identities
})
const versionMetadataQuery = useQuery({
    queryKey: computed(() => [
        'instance-upgrade',
        'version-display',
        ...releaseIdentities.value.map((identity) =>
            upgradeVersionCacheKey(identity.provider, identity.projectId, identity.releaseId),
        ),
    ]),
    queryFn: () => loadUpgradeVersionDisplayMetadata(releaseIdentities.value),
    staleTime: Number.POSITIVE_INFINITY,
})
const snapshotByContentId = computed(() => {
    const entries = (contentDataQuery.data.value?.snapshot.items ?? []).flatMap((item) =>
        contentIdentityKeys({
            instanceEntryId: item.entryId,
            instanceMemberId: item.memberId,
            instanceFileId: item.fileId,
            relativePath: item.expectedRelativePath,
        }).map((key) => [key, item] as const),
    )
    return new Map<string, InstanceContentSnapshotItem>(entries)
})
const contentByContentId = computed(() => {
    const data = contentDataQuery.data.value as InstanceContentData | null | undefined
    return new Map(
        [...(data?.contentItems ?? []), ...(data?.linkedContentItems ?? [])].flatMap((item) =>
            contentIdentityKeys(item).map((key) => [key, item] as const),
        ),
    )
})
const editableRoots = computed(() => editableUpgradeRoots(plan.value))
const availableStrategies = computed(() => availablePredefinedStrategies(plan.value))
const customDraftDirty = computed(
    () => !customConstraintsEqual(draftConstraints.value, plan.value.customConstraints),
)
const customWasResolved = computed(() => plan.value.selectedSolution?.kind === 'custom')
const effectiveSolution = computed(() => plan.value.selectedSolution)
const effectiveDependencyChanges = computed(() =>
    (effectiveSolution.value?.dependencyChanges ?? []).filter((entry) => entry.kind !== 'keep'),
)
const effectiveSummary = computed(() =>
    effectiveSolution.value ? solutionSummary(effectiveSolution.value) : null,
)
const effectiveMetrics = computed(() => {
    const summary = effectiveSummary.value
    if (!summary) return []
    return [
        { label: formatMessage(messages.metricUpdated), value: summary.upgraded },
        { label: formatMessage(messages.metricKept), value: summary.kept },
        { label: formatMessage(messages.metricDisabled), value: summary.disabled },
        { label: formatMessage(messages.metricAdded), value: summary.dependencyAdditions },
        {
            label: formatMessage(messages.metricDependencyUpdated),
            value: summary.dependencyUpdates,
        },
        { label: formatMessage(messages.metricRemoved), value: summary.dependencyRemovals },
    ]
})
const customIssues = computed(() =>
    activeStrategy.value === 'custom' && !customDraftDirty.value ? plan.value.blockingIssues : [],
)
const canApplyCustom = computed(
    () =>
        activeStrategy.value === 'custom' &&
        !requestBusy.value &&
        (customDraftDirty.value || !customWasResolved.value),
)
const canContinue = computed(
    () =>
        !requestBusy.value &&
        !customDraftDirty.value &&
        plan.value.blockingIssues.length === 0 &&
        plan.value.selectedSolution !== null &&
        plan.value.selectedSolution.kind === activeStrategy.value,
)
function registerControls() {
    flow.registerStepControls({
        canNext: canContinue,
        busy: requestBusy,
        nextLabel: formatMessage(messages.continue),
        onNext: continueUpgrade,
        onBack: goBack,
    })
}
onMounted(registerControls)
watch([canContinue, requestBusy], registerControls)
onBeforeUnmount(() => flow.registerStepControls(null))

function strategyClass(kind: InstanceUpgradeSolutionKind) {
    return activeStrategy.value === kind
        ? 'border-brand bg-surface-2 ring-1 ring-brand'
        : 'border-surface-4 bg-surface-2 hover:bg-surface-3'
}

function summaryText(solution: InstanceUpgradeSolution): string {
    const summary = solutionSummary(solution)
    return formatMessage(messages.rootSummary, {
        updates: summary.upgraded,
        kept: summary.kept,
        disabled: summary.disabled,
        dependencies:
            summary.dependencyAdditions + summary.dependencyUpdates + summary.dependencyRemovals,
    })
}

function errorMessage(error: unknown): string {
    if (error instanceof Error) return error.message
    if (typeof error === 'string') return error
    if (typeof error === 'object' && error && 'message' in error) return String(error.message)
    return String(error)
}

async function chooseStrategy(kind: InstanceUpgradeSolutionKind) {
    if (requestBusy.value || kind === activeStrategy.value) return
    requestError.value = null
    if (kind === 'custom') {
        activeStrategy.value = 'custom'
        flow.customizeActiveStrategy.value = 'custom'
        return
    }
    if (customDraftDirty.value) {
        pendingStrategy.value = kind
        return
    }
    activeStrategy.value = kind
    flow.customizeActiveStrategy.value = kind
    await selectPredefined(kind)
}

async function discardAndSwitch() {
    const target = pendingStrategy.value
    if (!target) return
    draftConstraints.value = plan.value.customConstraints.map((item) => ({ ...item }))
    pendingStrategy.value = null
    await selectPredefined(target)
}

async function selectPredefined(kind: Exclude<InstanceUpgradeSolutionKind, 'custom'>) {
    if (plan.value.selectedSolution?.kind === kind) {
        activeStrategy.value = kind
        flow.customizeActiveStrategy.value = kind
        return
    }
    requestBusy.value = true
    try {
        const updatedPlan = await select_instance_upgrade_solution(plan.value.id, kind)
        flow.setPlan(updatedPlan)
        activeStrategy.value = kind
        flow.customizeActiveStrategy.value = kind
    } catch (error) {
        requestError.value = errorMessage(error)
    } finally {
        requestBusy.value = false
    }
}

function contentMetadata(item: InstanceUpgradePlanItem) {
    return upgradeContentDisplayMetadata(
        item,
        contentByContentId.value.get(item.contentId) ??
            contentByContentId.value.get(normalizeUpgradePath(item.relativePath)),
        snapshotByContentId.value.get(item.contentId) ??
            snapshotByContentId.value.get(normalizeUpgradePath(item.relativePath)),
    )
}

function itemName(item: InstanceUpgradePlanItem): string {
    return contentMetadata(item).title
}

function projectPath(item: InstanceUpgradePlanItem): string | null {
    return upgradeProjectPath(item.provider, item.projectId)
}

function parkProjectReturn() {
    parkUpgradeFlow({
        instanceId: flow.instanceId.value,
        returnFullPath: router.currentRoute.value.fullPath,
        targetEnvironment: flow.targetEnvironment.value,
        plan: flow.plan.value,
        createFullBackup: flow.createFullBackup.value,
        directFullBackupPreference: flow.directFullBackupPreference.value,
        sharedUpgradeMode: flow.sharedUpgradeMode.value,
        activeJobId: flow.activeJobId.value,
        result: flow.result.value,
        initialBlockingPlanId: flow.initialBlockingPlanId.value,
        initialBlockingIssues: flow.initialBlockingIssues.value,
        customizeActiveStrategy: flow.customizeActiveStrategy.value,
    })
}

function itemIcon(item: InstanceUpgradePlanItem): string {
    return localContentIconUrl(contentMetadata(item).iconUrl)
}

function providerLabel(provider: ContentProvider | null): string {
    if (provider === 'modrinth') return formatMessage(messages.providerModrinth)
    if (provider === 'curseforge') return formatMessage(messages.providerCurseForge)
    return formatMessage(messages.providerUnknown)
}

function currentVersionLabel(item: InstanceUpgradePlanItem): string {
    const version = releaseLabel(
        item.provider,
        item.projectId,
        item.currentReleaseId,
        contentMetadata(item).currentVersion,
    )
    return formatMessage(messages.currentVersion, { version })
}

function effectiveTargetRelease(item: InstanceUpgradePlanItem): string | null {
    return (
        effectiveSolution.value?.selections.find((entry) => entry.contentId === item.contentId)
            ?.targetReleaseId ?? null
    )
}

function effectiveTargetVersion(item: InstanceUpgradePlanItem): string {
    return releaseLabel(item.provider, item.projectId, effectiveTargetRelease(item))
}

function releaseLabel(
    provider: ContentProvider | null,
    projectId: string | null,
    releaseId: string | null | undefined,
    fallback?: string | null,
): string {
    if (!provider || !projectId || !releaseId) return fallback ?? formatMessage(messages.noTarget)
    const resolved = upgradeVersionDisplayLabel(versionMetadataQuery.data.value, {
        provider,
        projectId,
        releaseId,
    })
    return resolved === releaseId && fallback ? fallback : resolved
}

function draftChoice(contentId: string): string {
    return (
        draftConstraints.value.find((constraint) => constraint.contentId === contentId)
            ?.versionId ?? AUTOMATIC
    )
}

function constraintOptions(item: InstanceUpgradePlanItem): string[] {
    const selected = draftChoice(item.contentId)
    return [
        AUTOMATIC,
        ...new Set([...item.candidateReleaseIds, ...(selected === AUTOMATIC ? [] : [selected])]),
    ]
}

function constraintOptionLabel(item: InstanceUpgradePlanItem, value: string): string {
    if (value === AUTOMATIC) return formatMessage(messages.automatic)
    const key =
        item.provider && item.projectId
            ? upgradeVersionCacheKey(item.provider, item.projectId, value)
            : null
    const version = key ? versionMetadataQuery.data.value?.get(key) : null
    if (!version) {
        return formatMessage(messages.specificVersion, {
            version: releaseLabel(item.provider, item.projectId, value),
        })
    }
    const channel =
        version.channel === 'release' || version.channel === 1
            ? formatMessage(messages.channelRelease)
            : version.channel === 'beta' || version.channel === 2
              ? formatMessage(messages.channelBeta)
              : formatMessage(messages.channelAlpha)
    return formatMessage(messages.specificVersionWithChannel, {
        version: version.version,
        channel,
    })
}

function setDraftChoice(item: InstanceUpgradePlanItem, versionId: string) {
    if (!item.provider || !item.projectId) return
    const constraint =
        versionId === AUTOMATIC
            ? null
            : {
                  contentId: item.contentId,
                  provider: item.provider,
                  projectId: item.projectId,
                  versionId,
              }
    draftConstraints.value = setFixedConstraint(draftConstraints.value, constraint, item.contentId)
    requestError.value = null
}

async function applyCustomChoices() {
    if (!canApplyCustom.value) return
    requestBusy.value = true
    requestError.value = null
    try {
        const updatedPlan = await resolve_custom_instance_upgrade_solution(
            plan.value.id,
            draftConstraints.value,
        )
        flow.setPlan(updatedPlan)
        draftConstraints.value = updatedPlan.customConstraints.map((item) => ({ ...item }))
        activeStrategy.value = 'custom'
        flow.customizeActiveStrategy.value = 'custom'
    } catch (error) {
        requestError.value = errorMessage(error)
    } finally {
        requestBusy.value = false
    }
}

function customIssueTitle(issue: InstanceUpgradeIssue): string {
    return issue.code === 'search_limit_reached'
        ? formatMessage(messages.searchLimitTitle)
        : formatMessage(messages.conflictTitle)
}

function customIssueBody(issue: InstanceUpgradeIssue): string {
    if (issue.code === 'search_limit_reached') return formatMessage(messages.searchLimitBody)
    return issue.message || issue.code
}

function dependencyActionLabel(kind: InstanceUpgradeDependencyChangeKind): string {
    if (kind === 'add') return formatMessage(messages.dependencyAdd)
    if (kind === 'upgrade') return formatMessage(messages.dependencyUpgrade)
    if (kind === 'remove') return formatMessage(messages.dependencyRemove)
    return formatMessage(messages.dependencyKeep)
}

function dependencyChangeDescription(change: InstanceUpgradeDependencyChange): string {
    const target = releaseLabel(change.provider, change.projectId, change.targetReleaseId)
    return change.existingContentId
        ? formatMessage(messages.dependencyReused, {
              current: releaseLabel(change.provider, change.projectId, change.currentReleaseId),
              target,
          })
        : formatMessage(messages.dependencyNew, { target })
}

async function goBack() {
    await router.push(
        `/instance/${encodeURIComponent(flow.instanceId.value)}/upgrade/compatibility`,
    )
}

async function continueUpgrade() {
    if (!canContinue.value) return
    await router.push(`/instance/${encodeURIComponent(flow.instanceId.value)}/upgrade/confirm`)
}
</script>
