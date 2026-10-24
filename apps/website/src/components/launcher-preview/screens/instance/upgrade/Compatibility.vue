<template>
    <section class="flex flex-col gap-6 py-2">
        <header class="flex flex-col gap-3">
            <div>
                <h2 class="m-0 text-xl font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.title) }}
                </h2>
                <p class="mb-0 mt-1 text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.description) }}
                </p>
            </div>
            <div class="flex flex-wrap gap-x-8 gap-y-2 text-sm">
                <div>
                    <span class="text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.minecraft)
                    }}</span>
                    <strong class="ml-2 text-[var(--color-text-primary)]">
                        {{ plan.sourceEnvironment.gameVersion }} <span aria-hidden="true">→</span>
                        {{ plan.targetEnvironment.gameVersion }}
                    </strong>
                </div>
                <div>
                    <span class="text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.loader)
                    }}</span>
                    <strong class="ml-2 text-[var(--color-text-primary)]">
                        {{ formatLoaderLabel(plan.sourceEnvironment.modLoader) }}
                        <span aria-hidden="true">→</span> {{ targetLoaderLabel }}
                    </strong>
                </div>
            </div>
        </header>

        <div class="grid grid-cols-2 gap-px overflow-hidden rounded-lg bg-divider sm:grid-cols-5">
            <div v-for="metric in summaryMetrics" :key="metric.label" class="bg-bg p-3">
                <div class="text-2xl font-semibold text-[var(--color-text-primary)]">
                    {{ metric.value }}
                </div>
                <div class="text-sm text-[var(--color-text-tertiary)]">{{ metric.label }}</div>
            </div>
        </div>

        <section v-for="severity in severityGroups" :key="severity.key" class="flex flex-col gap-2">
            <Accordion
                :open-by-default="true"
                :force-open="severity.key === 'blocking'"
                :overflow-visible="true"
                button-class="flex w-full items-center gap-2 border-0 bg-transparent p-0 text-left text-[var(--color-text-primary)]"
                content-class="pt-2"
            >
                <template #title>
                    <h3 class="m-0 text-lg font-semibold">{{ severity.title }}</h3>
                    <span class="text-sm text-[var(--color-text-tertiary)]">{{
                        severity.items.length
                    }}</span>
                    <span
                        v-if="severity.key === 'blocking' && initialBlockingCount"
                        class="text-sm text-[var(--color-text-tertiary)]"
                    >
                        ·
                        {{
                            formatMessage(messages.resolved, {
                                count: resolvedBlockingCount,
                                total: initialBlockingCount,
                            })
                        }}
                    </span>
                </template>
                <p
                    v-if="severity.key === 'blocking'"
                    class="mb-2 mt-1 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.blockingDescription) }}
                </p>
                <p
                    v-else-if="severity.key === 'warnings'"
                    class="mb-2 mt-1 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.warningsDescription) }}
                </p>

                <div
                    v-if="severity.key === 'warnings' && actionableWarningIds.length"
                    class="mb-2 flex flex-wrap items-center justify-between gap-2"
                >
                    <span
                        v-if="bulkAction"
                        class="text-sm text-[var(--color-text-tertiary)]"
                        role="status"
                    >
                        {{
                            formatMessage(messages.bulkProgress, {
                                current: bulkCompleted,
                                total: bulkTotal,
                            })
                        }}
                    </span>
                    <span v-else class="text-sm text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(messages.bulkEligible, {
                                count: actionableWarningIds.length,
                            })
                        }}
                    </span>
                    <div class="flex flex-wrap gap-2">
                        <template v-if="confirmDisableAll">
                            <span class="self-center text-sm text-[var(--color-text-tertiary)]">
                                {{
                                    formatMessage(messages.confirmDisableAll, {
                                        count: actionableWarningIds.length,
                                    })
                                }}
                            </span>
                            <Button
                                type="outlined"
                                size="2xs"
                                :disabled="bulkAction !== null"
                                @click="confirmDisableAll = false"
                                >{{ formatMessage(messages.cancel) }}
                            </Button>
                            <Button
                                type="colored"
                                color="orange"
                                size="2xs"
                                :disabled="bulkAction !== null"
                                @click="runBulkAction('disable')"
                                >{{ formatMessage(messages.confirm) }}
                            </Button>
                        </template>
                        <template v-else>
                            <Button
                                type="outlined"
                                size="2xs"
                                :disabled="bulkControlsDisabled"
                                @click="runBulkAction('keep')"
                                ><CheckIcon
                                    v-if="allWarningsAction === 'keep'"
                                    aria-hidden="true"
                                />
                                {{ formatMessage(messages.keepAll) }}
                            </Button>
                            <Button
                                type="outlined"
                                size="2xs"
                                :disabled="bulkControlsDisabled"
                                @click="confirmDisableAll = true"
                                ><CheckIcon
                                    v-if="allWarningsAction === 'disable'"
                                    aria-hidden="true"
                                />
                                {{ formatMessage(messages.disableAll) }}
                            </Button>
                        </template>
                    </div>
                </div>
                <p
                    v-if="bulkError && severity.key === 'warnings'"
                    class="mb-2 mt-0 text-sm text-red"
                >
                    {{ bulkError }}
                </p>
                <p
                    v-if="bulkWarning && severity.key === 'warnings'"
                    class="mb-2 mt-0 text-sm text-orange"
                >
                    {{ bulkWarning }}
                </p>
                <p
                    v-if="bulkInfo && severity.key === 'warnings'"
                    class="mb-2 mt-0 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ bulkInfo }}
                </p>

                <div
                    v-for="(issue, index) in severity.globalIssues"
                    :key="issueKey(issue, index)"
                    class="mb-2 flex gap-2 rounded-md bg-surface-2 px-3 py-2 text-sm"
                    :class="severity.key === 'blocking' ? 'text-red' : 'text-orange'"
                >
                    <XIcon
                        v-if="severity.key === 'blocking'"
                        class="mt-0.5 size-4 shrink-0"
                        aria-hidden="true"
                    />
                    <TriangleAlertIcon v-else class="mt-0.5 size-4 shrink-0" aria-hidden="true" />
                    <div class="min-w-0">
                        <strong>{{ formatMessage(messages.globalIssue) }}</strong>
                        <span class="ml-2 break-words text-[var(--color-text-tertiary)]">{{
                            issueDescription(issue)
                        }}</span>
                    </div>
                </div>

                <div class="upgrade-card-grid">
                    <div class="upgrade-card-list grid gap-3">
                        <article
                            v-for="group in severity.items"
                            :key="group.item.contentId"
                            class="flex min-w-0 flex-col gap-3 rounded-md border border-solid border-surface-4 bg-surface-2 p-3"
                        >
                            <div class="flex min-w-0 flex-1 items-start gap-3">
                                <Avatar
                                    :src="itemIcon(group.item)"
                                    :tint-by="itemName(group.item)"
                                    size="2.5rem"
                                    no-shadow
                                />
                                <div class="min-w-0 flex-1">
                                    <RouterLink
                                        v-if="projectPath(group.item)"
                                        :to="projectPath(group.item)!"
                                        class="inline-flex max-w-full items-center gap-1 font-semibold text-[var(--color-text-primary)] hover:text-brand hover:underline focus-visible:underline"
                                        @click="parkProjectReturn"
                                        ><span class="truncate">{{ itemName(group.item) }}</span
                                        ><ExternalIcon class="size-3 shrink-0" aria-hidden="true"
                                    /></RouterLink>
                                    <div
                                        v-else
                                        class="truncate font-semibold text-[var(--color-text-primary)]"
                                    >
                                        {{ itemName(group.item) }}
                                    </div>
                                    <div
                                        class="flex flex-wrap gap-x-3 text-sm text-[var(--color-text-tertiary)]"
                                    >
                                        <span>{{ providerLabel(group.item.provider) }}</span>
                                        <span
                                            class="flex min-w-0 flex-wrap items-center gap-1 break-words"
                                        >
                                            <UpgradeVersionChangelogPopout
                                                v-if="currentRelease(group.item)"
                                                :label="currentVersionLabel(group.item)"
                                                :provider="group.item.provider"
                                                :project-id="group.item.projectId"
                                                :release-id="currentRelease(group.item)"
                                            />
                                            <span v-else>{{
                                                currentVersionLabel(group.item)
                                            }}</span>
                                            <span
                                                v-if="targetRelease(group.item)"
                                                aria-hidden="true"
                                                >→</span
                                            >
                                            <UpgradeVersionChangelogPopout
                                                v-if="targetRelease(group.item)"
                                                :label="targetVersionLabel(group.item)"
                                                :provider="group.item.provider"
                                                :project-id="group.item.projectId"
                                                :release-id="targetRelease(group.item)"
                                            />
                                        </span>
                                        <span v-if="!group.item.currentEnabled">{{
                                            formatMessage(messages.disabled)
                                        }}</span>
                                    </div>
                                    <div
                                        v-if="group.blockingIssues.length || group.warnings.length"
                                        class="mt-2 space-y-1"
                                    >
                                        <div
                                            v-for="(issue, index) in group.blockingIssues"
                                            :key="issueKey(issue, index)"
                                            class="flex gap-1.5 text-sm text-red"
                                        >
                                            <XIcon
                                                class="mt-0.5 size-4 shrink-0"
                                                aria-hidden="true"
                                            />
                                            <span class="break-words">{{
                                                issueDescription(issue)
                                            }}</span>
                                        </div>
                                        <div
                                            v-for="(issue, index) in group.warnings"
                                            :key="issueKey(issue, index)"
                                            class="flex gap-1.5 text-sm text-orange"
                                        >
                                            <TriangleAlertIcon
                                                class="mt-0.5 size-4 shrink-0"
                                                aria-hidden="true"
                                            />
                                            <span class="break-words">{{
                                                issueDescription(issue)
                                            }}</span>
                                        </div>
                                    </div>
                                    <div class="mt-3 flex min-w-0 flex-col items-start gap-2">
                                        <div
                                            v-if="
                                                requiresNoCompatibleResolution(group.item) ||
                                                warningSupportsResolution(group)
                                            "
                                            class="flex flex-wrap gap-2"
                                        >
                                            <Button
                                                type="outlined"
                                                size="2xs"
                                                :disabled="itemControlsDisabled(group.item)"
                                                :class="{
                                                    'text-green':
                                                        group.item.resolution.action === 'keep',
                                                }"
                                                @click="updateAction(group.item, 'keep')"
                                                ><SpinnerIcon
                                                    v-if="
                                                        pendingAction[group.item.contentId] ===
                                                        'keep'
                                                    "
                                                    class="animate-spin"
                                                    aria-hidden="true"
                                                />
                                                <CheckIcon
                                                    v-else-if="
                                                        twoOptionPresentation(group.item)
                                                            .selectedAction === 'keep'
                                                    "
                                                    aria-hidden="true"
                                                />
                                                {{ formatMessage(messages.keepCurrent) }}
                                            </Button>
                                            <Button
                                                type="outlined"
                                                size="2xs"
                                                :disabled="itemControlsDisabled(group.item)"
                                                :class="{
                                                    'text-green':
                                                        group.item.resolution.action === 'disable',
                                                }"
                                                @click="updateAction(group.item, 'disable')"
                                                ><SpinnerIcon
                                                    v-if="
                                                        pendingAction[group.item.contentId] ===
                                                        'disable'
                                                    "
                                                    class="animate-spin"
                                                    aria-hidden="true"
                                                />
                                                <CheckIcon
                                                    v-else-if="
                                                        twoOptionPresentation(group.item)
                                                            .selectedAction === 'disable'
                                                    "
                                                    aria-hidden="true"
                                                />
                                                {{ formatMessage(messages.disableCurrent) }}
                                            </Button>
                                        </div>
                                        <div
                                            v-if="isPrereleaseBlocker(group.item)"
                                            class="flex max-w-xl flex-col gap-2"
                                        >
                                            <span
                                                v-if="group.item.candidateReleaseIds[0]"
                                                class="text-sm text-[var(--color-text-tertiary)]"
                                            >
                                                {{
                                                    formatMessage(messages.candidateRelease, {
                                                        version: releaseLabel(
                                                            group.item.provider,
                                                            group.item.projectId,
                                                            group.item.candidateReleaseIds[0],
                                                        ),
                                                    })
                                                }}
                                            </span>
                                            <div class="flex flex-wrap gap-2">
                                                <Button
                                                    type="outlined"
                                                    size="2xs"
                                                    :disabled="itemControlsDisabled(group.item)"
                                                    :class="{
                                                        'text-green':
                                                            prereleasePresentation(group.item)
                                                                .selectedAction === 'upgrade',
                                                    }"
                                                    @click="allowPrerelease(group.item)"
                                                    ><SpinnerIcon
                                                        v-if="
                                                            resolutionBusy.has(group.item.contentId)
                                                        "
                                                        class="animate-spin"
                                                        aria-hidden="true"
                                                    />
                                                    <CheckIcon
                                                        v-else-if="
                                                            prereleasePresentation(group.item)
                                                                .selectedAction === 'upgrade'
                                                        "
                                                        aria-hidden="true"
                                                    />
                                                    {{ formatMessage(messages.allowPrerelease) }}
                                                </Button>
                                                <Button
                                                    v-if="
                                                        prereleasePresentation(group.item).showUndo
                                                    "
                                                    type="quiet"
                                                    size="2xs"
                                                    :disabled="itemControlsDisabled(group.item)"
                                                    @click="resetAction(group.item)"
                                                    >{{ formatMessage(messages.undo) }}
                                                </Button>
                                            </div>
                                        </div>
                                        <p
                                            v-if="resolutionErrors[group.item.contentId]"
                                            class="m-0 break-words text-sm text-red"
                                        >
                                            {{ resolutionErrors[group.item.contentId] }}
                                        </p>
                                    </div>
                                </div>
                            </div>
                        </article>
                    </div>
                </div>
            </Accordion>
        </section>

        <section v-if="visibleDependencyChanges.length" class="flex flex-col gap-2">
            <h3 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.dependenciesTitle) }}
            </h3>
            <div class="overflow-hidden rounded-lg border border-solid border-divider">
                <div
                    v-for="change in visibleDependencyChanges"
                    :key="`${change.provider}:${change.projectId}:${change.existingContentId ?? 'new'}`"
                    class="flex items-center justify-between gap-4 border-0 border-b border-solid border-divider p-3 last:border-b-0"
                >
                    <div class="min-w-0">
                        <div class="truncate font-semibold text-[var(--color-text-primary)]">
                            {{ change.projectId }}
                        </div>
                        <div class="text-sm text-[var(--color-text-tertiary)]">
                            {{ providerLabel(change.provider) }} ·
                            {{ dependencyVersionChange(change) }}
                        </div>
                    </div>
                    <strong class="shrink-0 text-sm text-[var(--color-text-primary)]">{{
                        dependencyActionLabel(change.kind)
                    }}</strong>
                </div>
            </div>
        </section>
    </section>
</template>

<script setup lang="ts">
import './messages'

import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TriangleAlertIcon from '@modrinth/assets/icons/triangle-alert.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Accordion from '@modrinth/ui/src/components/base/Accordion.vue'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { formatLoaderLabel } from '@modrinth/ui/src/utils/loaders.ts'
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import {
    actionableWarningContentIds,
    captureInitialUpgradeBlockingIssues,
    compatibilitySummary,
    contentIdentityKeys,
    groupUpgradeIssues,
    normalizeUpgradePath,
    upgradeContentDisplayMetadata,
    type UpgradeContentIssueGroup,
    upgradeResolutionPresentation,
} from './analysis'
import type {
    ContentProvider,
    InstanceUpgradeAction,
    InstanceUpgradeDependencyChange,
    InstanceUpgradeDependencyChangeKind,
    InstanceUpgradeIssue,
    InstanceUpgradePlanItem,
} from './backend'
import {
    reset_instance_upgrade_resolution,
    update_instance_upgrade_resolution,
    update_instance_upgrade_resolutions,
} from './backend'
import {
    type InstanceContentData,
    type InstanceContentSnapshotItem,
    loadInstanceContentData,
    localContentIconUrl,
} from './content'
import { useInstanceUpgradeFlow } from './flow'
import { bulkResolutionAction, filterBulkResolutionIds } from './flow-controls'
import {
    loadUpgradeVersionDisplayMetadata,
    type UpgradeReleaseIdentity,
    upgradeVersionDisplayLabel,
} from './metadata'
import { parkUpgradeFlow, upgradeProjectPath } from './navigation'
import { useQuery } from './query'
import UpgradeVersionChangelogPopout from './UpgradeVersionChangelogPopout.vue'

const messages = defineMessages({
    title: { id: 'instance.upgrade.compatibility.title', defaultMessage: 'Compatibility' },
    description: {
        id: 'instance.upgrade.compatibility.description',
        defaultMessage: 'Review content changes and resolve issues before continuing.',
    },
    minecraft: { id: 'instance.upgrade.compatibility.minecraft', defaultMessage: 'Minecraft' },
    loader: { id: 'instance.upgrade.compatibility.loader', defaultMessage: 'Loader' },
    updates: { id: 'instance.upgrade.compatibility.updates', defaultMessage: 'Will update' },
    compatible: {
        id: 'instance.upgrade.compatibility.compatible',
        defaultMessage: 'Compatible or kept',
    },
    disabledCount: {
        id: 'instance.upgrade.compatibility.disabled-count',
        defaultMessage: 'Will disable',
    },
    dependencyChanges: {
        id: 'instance.upgrade.compatibility.dependency-changes',
        defaultMessage: 'Dependencies changed',
    },
    needsAttention: {
        id: 'instance.upgrade.compatibility.needs-attention',
        defaultMessage: 'Needs attention',
    },
    blockersTitle: {
        id: 'instance.upgrade.compatibility.blockers',
        defaultMessage: 'Blocking issues',
    },
    resolved: {
        id: 'instance.upgrade.compatibility.resolved',
        defaultMessage: 'Resolved {count}/{total}',
    },
    warningsTitle: { id: 'instance.upgrade.compatibility.warnings', defaultMessage: 'Warnings' },
    noIssuesTitle: { id: 'instance.upgrade.compatibility.no-issues', defaultMessage: 'No issues' },
    contentTitle: { id: 'instance.upgrade.compatibility.content', defaultMessage: 'Content' },
    contentCount: {
        id: 'instance.upgrade.compatibility.content-count',
        defaultMessage: '{count, plural, one {# item} other {# items}}',
    },
    keepAll: { id: 'instance.upgrade.compatibility.keep-all', defaultMessage: 'Keep all' },
    disableAll: { id: 'instance.upgrade.compatibility.disable-all', defaultMessage: 'Disable all' },
    bulkEligible: {
        id: 'instance.upgrade.compatibility.bulk-eligible',
        defaultMessage: '{count, plural, one {# actionable item} other {# actionable items}}',
    },
    bulkProgress: {
        id: 'instance.upgrade.compatibility.bulk-progress',
        defaultMessage: 'Updating {current} of {total}…',
    },
    bulkPartial: {
        id: 'instance.upgrade.compatibility.bulk-partial',
        defaultMessage: '{applied} updated; {remaining} still need review.',
    },
    bulkSkipped: {
        id: 'instance.upgrade.compatibility.bulk-skipped',
        defaultMessage: '{count} items were skipped.',
    },
    confirmDisableAll: {
        id: 'instance.upgrade.compatibility.confirm-disable-all',
        defaultMessage: 'Disable {count, plural, one {# warning item} other {# warning items}}?',
    },
    cancel: { id: 'instance.upgrade.compatibility.cancel', defaultMessage: 'Cancel' },
    confirm: { id: 'instance.upgrade.compatibility.confirm', defaultMessage: 'Confirm' },
    globalIssue: {
        id: 'instance.upgrade.compatibility.global-issue',
        defaultMessage: 'Instance-wide issue',
    },
    dependenciesTitle: {
        id: 'instance.upgrade.compatibility.dependencies',
        defaultMessage: 'Dependency changes',
    },
    disabled: {
        id: 'instance.upgrade.compatibility.disabled',
        defaultMessage: 'Currently disabled',
    },
    noCompatibleExplanation: {
        id: 'instance.upgrade.compatibility.no-compatible-explanation',
        defaultMessage:
            'No compatible release was found for the target Minecraft version. Keeping it may prevent the game from working; you can also disable it temporarily.',
    },
    keepCurrent: {
        id: 'instance.upgrade.compatibility.keep-current',
        defaultMessage: 'Keep current',
    },
    disableCurrent: {
        id: 'instance.upgrade.compatibility.disable-current',
        defaultMessage: 'Disable current content',
    },
    prereleaseExplanation: {
        id: 'instance.upgrade.compatibility.prerelease-explanation',
        defaultMessage:
            'No compatible stable release was found, but a beta or alpha release is available.',
    },
    candidateRelease: {
        id: 'instance.upgrade.compatibility.candidate-release',
        defaultMessage: 'Candidate: {version}',
    },
    allowPrerelease: {
        id: 'instance.upgrade.compatibility.allow-prerelease',
        defaultMessage: 'Allow prerelease for this content',
    },
    back: { id: 'instance.upgrade.compatibility.back', defaultMessage: 'Back' },
    continue: { id: 'instance.upgrade.compatibility.continue', defaultMessage: 'Continue' },
    resolveBeforeContinue: {
        id: 'instance.upgrade.compatibility.resolve-before-continue',
        defaultMessage: 'Resolve blocking issues before continuing.',
    },
    blockingDescription: {
        id: 'instance.upgrade.compatibility.blocking-description',
        defaultMessage: 'These items need your decision before the upgrade can continue.',
    },
    warningsDescription: {
        id: 'instance.upgrade.compatibility.warnings-description',
        defaultMessage: 'Review these items before choosing an upgrade solution.',
    },
    undo: { id: 'instance.upgrade.compatibility.undo', defaultMessage: 'Undo' },
    providerModrinth: { id: 'instance.upgrade.provider.modrinth', defaultMessage: 'Modrinth' },
    providerCurseForge: {
        id: 'instance.upgrade.provider.curseforge',
        defaultMessage: 'CurseForge',
    },
    providerLocal: { id: 'instance.upgrade.provider.local', defaultMessage: 'Local' },
    providerUnknown: {
        id: 'instance.upgrade.provider.unknown',
        defaultMessage: 'Unknown provider',
    },
    actionUpgrade: { id: 'instance.upgrade.action.upgrade', defaultMessage: 'Update' },
    actionKeep: { id: 'instance.upgrade.action.keep', defaultMessage: 'Keep' },
    actionDisable: { id: 'instance.upgrade.action.disable', defaultMessage: 'Disable' },
    actionAdd: { id: 'instance.upgrade.action.add-dependency', defaultMessage: 'Add dependency' },
    actionRemove: {
        id: 'instance.upgrade.action.remove-dependency',
        defaultMessage: 'Remove dependency',
    },
    actionDependencyUpgrade: {
        id: 'instance.upgrade.action.update-dependency',
        defaultMessage: 'Update dependency',
    },
    unknownVersion: {
        id: 'instance.upgrade.compatibility.unknown-version',
        defaultMessage: 'Unknown version',
    },
    versionChange: {
        id: 'instance.upgrade.compatibility.version-change',
        defaultMessage: '{current} → {target}',
    },
    currentVersionOnly: {
        id: 'instance.upgrade.compatibility.current-version-only',
        defaultMessage: 'Current: {current}',
    },
    issuePrerelease: {
        id: 'instance.upgrade.issue.prerelease-only',
        defaultMessage: 'Prerelease confirmation required',
    },
    issueNoCompatible: {
        id: 'instance.upgrade.issue.no-compatible-release',
        defaultMessage: 'No compatible release',
    },
    issueDependency: {
        id: 'instance.upgrade.issue.dependency',
        defaultMessage: 'Dependency conflict',
    },
    issueShader: {
        id: 'instance.upgrade.issue.shader',
        defaultMessage: 'Shader compatibility issue',
    },
    issueGeneric: { id: 'instance.upgrade.issue.generic', defaultMessage: 'Compatibility issue' },
    issuePrereleaseDescription: {
        id: 'instance.upgrade.issue.prerelease-only.description',
        defaultMessage: 'Only a prerelease version is available for Minecraft {version}.',
    },
    issueNoCompatibleDescription: {
        id: 'instance.upgrade.issue.no-compatible-release.description',
        defaultMessage: 'No compatible release was found for Minecraft {version}.',
    },
    issueShaderMissing: {
        id: 'instance.upgrade.issue.shader-runtime-missing',
        defaultMessage: 'No shader runtime is planned for the target instance.',
    },
    issueShaderUnknown: {
        id: 'instance.upgrade.issue.shader-runtime-unknown',
        defaultMessage: 'The target shader runtime could not be determined.',
    },
    issueShaderIncompatible: {
        id: 'instance.upgrade.issue.no-compatible-shader-runtime',
        defaultMessage: 'No release supports the target Minecraft version and shader runtime.',
    },
    issueUnidentified: {
        id: 'instance.upgrade.issue.unidentified',
        defaultMessage: 'This local file could not be matched to a verified provider project.',
    },
    issueUnsupported: {
        id: 'instance.upgrade.issue.unsupported-content-type',
        defaultMessage: 'This content type cannot be migrated automatically.',
    },
    issueSearchLimit: {
        id: 'instance.upgrade.issue.search-limit-reached',
        defaultMessage: 'The bounded compatibility search could not prove a valid solution.',
    },
    issueKeepIncompatible: {
        id: 'instance.upgrade.issue.keep-incompatible',
        defaultMessage: 'Keeping this version may make the upgraded instance incompatible.',
    },
})

const flow = useInstanceUpgradeFlow()
const router = useRouter()
const { formatMessage } = useVIntl()
const plan = computed(() => flow.plan.value!)
const resolutionBusy = ref(new Set<string>())
const pendingAction = reactive<Record<string, InstanceUpgradeAction | undefined>>({})
const resolutionErrors = reactive<Record<string, string | undefined>>({})
const bulkAction = ref<'keep' | 'disable' | null>(null)
const bulkCompleted = ref(0)
const bulkTotal = ref(0)
const bulkError = ref<string | null>(null)
const bulkWarning = ref<string | null>(null)
const bulkInfo = ref<string | null>(null)
const confirmDisableAll = ref(false)
let resolutionQueue = Promise.resolve()

const contentDataQuery = useQuery({
    queryKey: computed(() => ['instance-upgrade', 'content-data', flow.instanceId.value]),
    queryFn: () => loadInstanceContentData(flow.instanceId.value),
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
const selectionByContentId = computed(
    () =>
        new Map(
            (plan.value.selectedSolution?.selections ?? []).map((item) => [item.contentId, item]),
        ),
)
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
    for (const change of plan.value.selectedSolution?.dependencyChanges ??
        plan.value.dependencyChanges) {
        add(change.provider, change.projectId, change.currentReleaseId)
        add(change.provider, change.projectId, change.targetReleaseId)
    }
    return identities
})
const versionMetadataQuery = useQuery({
    queryKey: computed(() => [
        'instance-upgrade',
        'version-display',
        ...releaseIdentities.value.map(
            (identity) => `${identity.provider}:${identity.projectId}:${identity.releaseId}`,
        ),
    ]),
    queryFn: () => loadUpgradeVersionDisplayMetadata(releaseIdentities.value),
    staleTime: Number.POSITIVE_INFINITY,
})
const summary = computed(() => compatibilitySummary(plan.value))
watch(
    () => plan.value.id,
    (planId) => {
        if (flow.initialBlockingPlanId.value === planId) return
        flow.initialBlockingPlanId.value = planId
        flow.initialBlockingIssues.value = captureInitialUpgradeBlockingIssues(plan.value)
    },
    { immediate: true },
)
const issueGroups = computed(() => groupUpgradeIssues(plan.value, flow.initialBlockingIssues.value))
const initialBlockingCount = computed(() => Object.keys(flow.initialBlockingIssues.value).length)
const resolvedBlockingCount = computed(() => {
    const unresolved = new Set(
        issueGroups.value.blocking
            .filter((group) => group.currentlyBlocking)
            .map((group) => group.item.contentId),
    )
    return Object.keys(flow.initialBlockingIssues.value).filter((id) => !unresolved.has(id)).length
})
const actionableWarningIds = computed(() => actionableWarningContentIds(issueGroups.value))
const actionableWarningIdSet = computed(() => new Set(actionableWarningIds.value))
const allWarningsAction = computed<'keep' | 'disable' | null>(() => {
    const items = issueGroups.value.warnings
        .filter((group) => actionableWarningIdSet.value.has(group.item.contentId))
        .map((group) => group.item)
    if (!items.length) return null
    return bulkResolutionAction(items.map((item) => item.resolution.action))
})
const severityGroups = computed(() => [
    {
        key: 'blocking' as const,
        title: formatMessage(messages.blockersTitle),
        items: issueGroups.value.blocking,
        globalIssues: issueGroups.value.globalBlockingIssues,
    },
    {
        key: 'warnings' as const,
        title: formatMessage(messages.warningsTitle),
        items: issueGroups.value.warnings,
        globalIssues: issueGroups.value.globalWarnings,
    },
    {
        key: 'none' as const,
        title: formatMessage(messages.noIssuesTitle),
        items: issueGroups.value.noIssues,
        globalIssues: [],
    },
])
const summaryMetrics = computed(() => [
    { label: formatMessage(messages.updates), value: summary.value.updates },
    { label: formatMessage(messages.compatible), value: summary.value.keptOrCompatible },
    { label: formatMessage(messages.disabledCount), value: summary.value.disabled },
    { label: formatMessage(messages.dependencyChanges), value: summary.value.dependencyChanges },
    { label: formatMessage(messages.needsAttention), value: summary.value.needsAttention },
])
const visibleDependencyChanges = computed(() =>
    (plan.value.selectedSolution?.dependencyChanges ?? plan.value.dependencyChanges).filter(
        (change) => change.kind !== 'keep',
    ),
)
const targetLoaderLabel = computed(() => {
    const loader = formatLoaderLabel(plan.value.targetEnvironment.modLoader)
    return plan.value.targetEnvironment.modLoaderVersion
        ? `${loader} ${plan.value.targetEnvironment.modLoaderVersion}`
        : loader
})
const canContinue = computed(
    () =>
        plan.value.blockingIssues.length === 0 &&
        resolutionBusy.value.size === 0 &&
        bulkAction.value === null &&
        !flow.busy.value,
)

const bulkControlsDisabled = computed(
    () => bulkAction.value !== null || resolutionBusy.value.size > 0 || flow.busy.value,
)

function registerControls() {
    flow.registerStepControls({
        canNext: canContinue,
        busy: computed(() => bulkAction.value !== null || resolutionBusy.value.size > 0),
        nextLabel: formatMessage(messages.continue),
        onNext: continueUpgrade,
        onBack: goBack,
    })
}
onMounted(registerControls)
watch([canContinue, bulkAction, resolutionBusy], registerControls)
onBeforeUnmount(() => flow.registerStepControls(null))

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
    if (provider === 'local') return formatMessage(messages.providerLocal)
    return formatMessage(messages.providerUnknown)
}

function releaseLabel(
    provider: ContentProvider | null,
    projectId: string | null,
    releaseId: string | null,
    fallback?: string | null,
): string {
    if (!provider || !projectId || !releaseId)
        return fallback ?? formatMessage(messages.unknownVersion)
    const resolved = upgradeVersionDisplayLabel(versionMetadataQuery.data.value, {
        provider,
        projectId,
        releaseId,
    })
    return resolved === releaseId && fallback ? fallback : resolved
}

function currentRelease(item: InstanceUpgradePlanItem): string | null {
    return projectPath(item) ? item.currentReleaseId : null
}

function currentVersionLabel(item: InstanceUpgradePlanItem): string {
    return releaseLabel(
        item.provider,
        item.projectId,
        item.currentReleaseId,
        contentMetadata(item).currentVersion,
    )
}

function targetVersionLabel(item: InstanceUpgradePlanItem): string {
    return releaseLabel(item.provider, item.projectId, targetRelease(item))
}

function targetRelease(item: InstanceUpgradePlanItem): string | null {
    return (
        selectionByContentId.value.get(item.contentId)?.targetReleaseId ??
        item.candidateReleaseIds[0] ??
        null
    )
}

function hasBlockingIssue(item: InstanceUpgradePlanItem, code: InstanceUpgradeIssue['code']) {
    return (
        issueGroups.value.blocking
            .find((group) => group.item.contentId === item.contentId)
            ?.blockingIssues.some((issue) => issue.code === code) ?? false
    )
}

function requiresNoCompatibleResolution(item: InstanceUpgradePlanItem): boolean {
    return (
        item.status === 'no_compatible_release' || hasBlockingIssue(item, 'no_compatible_release')
    )
}

function isPrereleaseBlocker(item: InstanceUpgradePlanItem): boolean {
    return (
        item.status === 'prerelease_only' ||
        hasBlockingIssue(item, 'prerelease_only') ||
        (flow.initialBlockingIssues.value[item.contentId] ?? []).some(
            (issue) => issue.code === 'prerelease_only',
        )
    )
}

function twoOptionPresentation(item: InstanceUpgradePlanItem) {
    return upgradeResolutionPresentation('two-option', item.resolution)
}

function prereleasePresentation(item: InstanceUpgradePlanItem) {
    return upgradeResolutionPresentation('single-prerelease', item.resolution)
}

function dependencyVersionChange(change: InstanceUpgradeDependencyChange): string {
    const current = releaseLabel(change.provider, change.projectId, change.currentReleaseId)
    return change.targetReleaseId
        ? formatMessage(messages.versionChange, {
              current,
              target: releaseLabel(change.provider, change.projectId, change.targetReleaseId),
          })
        : formatMessage(messages.currentVersionOnly, { current })
}

function dependencyActionLabel(kind: InstanceUpgradeDependencyChangeKind): string {
    if (kind === 'add') return formatMessage(messages.actionAdd)
    if (kind === 'remove') return formatMessage(messages.actionRemove)
    if (kind === 'upgrade') return formatMessage(messages.actionDependencyUpgrade)
    return formatMessage(messages.actionKeep)
}

function issueDescription(issue: InstanceUpgradeIssue): string {
    if (issue.code === 'prerelease_only') {
        return formatMessage(messages.issuePrereleaseDescription, {
            version: plan.value.targetEnvironment.gameVersion,
        })
    }
    if (issue.code === 'no_compatible_release') {
        return formatMessage(messages.issueNoCompatibleDescription, {
            version: plan.value.targetEnvironment.gameVersion,
        })
    }
    if (issue.code === 'shader_runtime_missing') return formatMessage(messages.issueShaderMissing)
    if (issue.code === 'shader_runtime_unknown') return formatMessage(messages.issueShaderUnknown)
    if (issue.code === 'no_compatible_shader_runtime') {
        return formatMessage(messages.issueShaderIncompatible)
    }
    if (issue.code === 'unidentified') return formatMessage(messages.issueUnidentified)
    if (issue.code === 'unsupported_content_type') return formatMessage(messages.issueUnsupported)
    if (issue.code === 'search_limit_reached') return formatMessage(messages.issueSearchLimit)
    if (issue.code === 'keep_incompatible') return formatMessage(messages.issueKeepIncompatible)
    if (
        ['dependency_conflict', 'missing_required_dependency', 'incompatible_dependency'].includes(
            issue.code,
        )
    )
        return formatMessage(messages.issueDependency)
    return issue.message || formatMessage(messages.issueGeneric)
}

function issueKey(issue: InstanceUpgradeIssue, index: number): string {
    return `${issue.code}:${issue.contentId ?? issue.projectId ?? index}`
}

function warningSupportsResolution(group: UpgradeContentIssueGroup): boolean {
    return actionableWarningIdSet.value.has(group.item.contentId)
}

function itemControlsDisabled(item: InstanceUpgradePlanItem): boolean {
    return bulkAction.value !== null || resolutionBusy.value.has(item.contentId)
}

function setBusy(contentId: string, busy: boolean) {
    const next = new Set(resolutionBusy.value)
    if (busy) next.add(contentId)
    else next.delete(contentId)
    resolutionBusy.value = next
}

function errorMessage(error: unknown): string {
    if (error instanceof Error) return error.message
    if (typeof error === 'string') return error
    if (typeof error === 'object' && error && 'message' in error) return String(error.message)
    return String(error)
}

async function queueResolution(
    item: InstanceUpgradePlanItem,
    transform: (current: InstanceUpgradePlanItem) => InstanceUpgradePlanItem['resolution'],
) {
    if (resolutionBusy.value.has(item.contentId)) return
    setBusy(item.contentId, true)
    resolutionErrors[item.contentId] = undefined
    const task = resolutionQueue.then(async () => {
        const current = flow.plan.value?.items.find(
            (candidate) => candidate.contentId === item.contentId,
        )
        if (!current || !flow.plan.value) return
        const updatedPlan = await update_instance_upgrade_resolution(
            flow.plan.value.id,
            transform(current),
        )
        flow.setPlan(updatedPlan)
    })
    resolutionQueue = task.catch(() => undefined)
    try {
        await task
    } catch (error) {
        resolutionErrors[item.contentId] = errorMessage(error)
    } finally {
        setBusy(item.contentId, false)
        pendingAction[item.contentId] = undefined
    }
}

async function updateAction(item: InstanceUpgradePlanItem, action: 'keep' | 'disable') {
    pendingAction[item.contentId] = action
    await queueResolution(item, (current) => ({ ...current.resolution, action }))
}

async function runBulkAction(action: 'keep' | 'disable') {
    if (bulkControlsDisabled.value) return
    const initialContentIds = [...actionableWarningIds.value]
    bulkAction.value = action
    bulkCompleted.value = 0
    bulkTotal.value = initialContentIds.length
    bulkError.value = null
    bulkWarning.value = null
    bulkInfo.value = null
    confirmDisableAll.value = false
    try {
        const currentPlan = flow.plan.value
        if (!currentPlan) return
        const requestedIds = filterBulkResolutionIds(
            initialContentIds.flatMap((contentId) => {
                const current = currentPlan.items.find((item) => item.contentId === contentId)
                return current ? [{ contentId, action: current.resolution.action }] : []
            }),
            action,
        )
        const resolutions = requestedIds.flatMap((contentId) => {
            const current = currentPlan.items.find((item) => item.contentId === contentId)
            return current ? [{ ...current.resolution, action }] : []
        })
        if (!resolutions.length) return
        const result = await update_instance_upgrade_resolutions(currentPlan.id, resolutions)
        flow.setPlan(result.plan)
        bulkCompleted.value = result.applied.length
        if (result.failed.length) {
            bulkWarning.value = formatMessage(messages.bulkPartial, {
                applied: result.applied.length,
                remaining: result.failed.length + result.skipped.length,
            })
        }
        if (result.skipped.length) {
            bulkInfo.value = formatMessage(messages.bulkSkipped, { count: result.skipped.length })
        }
    } catch (error) {
        bulkError.value = errorMessage(error)
    } finally {
        bulkAction.value = null
    }
}

async function resetAction(item: InstanceUpgradePlanItem) {
    if (itemControlsDisabled(item)) return
    setBusy(item.contentId, true)
    try {
        flow.setPlan(await reset_instance_upgrade_resolution(plan.value.id, item.contentId))
    } catch (error) {
        resolutionErrors[item.contentId] = errorMessage(error)
    } finally {
        setBusy(item.contentId, false)
    }
}

async function allowPrerelease(item: InstanceUpgradePlanItem) {
    if (item.resolution.allowPrerelease) return
    pendingAction[item.contentId] = 'upgrade'
    await queueResolution(item, (current) => {
        const confirmations = plan.value.blockingIssues
            .filter((issue) => issue.code === 'prerelease_only')
            .flatMap((issue) => issue.dependencyRequirements)
            .filter(
                (requirement) =>
                    requirement.rootContentId === current.contentId &&
                    requirement.candidateReleaseId !== null,
            )
            .map((requirement) => ({
                provider: requirement.dependencyProvider,
                projectId: requirement.dependencyProjectId,
                versionId: requirement.candidateReleaseId!,
            }))
        const byIdentity = new Map(
            [...current.resolution.confirmedPrereleaseDependencies, ...confirmations].map(
                (confirmation) => [
                    `${confirmation.provider}:${confirmation.projectId}:${confirmation.versionId}`,
                    confirmation,
                ],
            ),
        )
        return {
            ...current.resolution,
            action: 'upgrade',
            allowPrerelease: true,
            confirmedPrereleaseDependencies: [...byIdentity.values()],
        }
    })
}

async function goBack() {
    await router.push(`/instance/${encodeURIComponent(flow.instanceId.value)}/upgrade`)
}

async function continueUpgrade() {
    if (!canContinue.value) return
    await router.push(`/instance/${encodeURIComponent(flow.instanceId.value)}/upgrade/customize`)
}
</script>

<style scoped>
.upgrade-card-grid {
    container-type: inline-size;
}

@container (min-width: 50rem) {
    .upgrade-card-list {
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
}

@container (min-width: 81.25rem) {
    .upgrade-card-list {
        grid-template-columns: repeat(3, minmax(0, 1fr));
    }
}
</style>
