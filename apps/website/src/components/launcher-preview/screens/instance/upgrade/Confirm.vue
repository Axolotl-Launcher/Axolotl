<template>
    <section class="flex flex-col gap-6 py-2">
        <header>
            <h2 class="m-0 text-xl font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h2>
            <p class="mb-0 mt-1 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
        </header>

        <Admonition
            v-if="executionError"
            type="critical"
            :header="formatMessage(messages.startError)"
        >
            {{ executionError }}
        </Admonition>

        <Admonition type="warning" :header="formatMessage(messages.worldTitle)">
            <div class="flex flex-col gap-2">
                <span>{{ formatMessage(messages.worldBody) }}</span>
                <span>{{ formatMessage(messages.datapackNote, { path: datapackPath }) }}</span>
            </div>
        </Admonition>

        <div class="grid gap-3 md:grid-cols-2">
            <Card class="!m-0 p-4">
                <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.environment) }}
                </h3>
                <div class="mt-3 grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
                    <span class="text-[var(--color-text-tertiary)]">Minecraft</span>
                    <strong
                        >{{ plan.sourceEnvironment.gameVersion }} <span aria-hidden="true">→</span>
                        {{ plan.targetEnvironment.gameVersion }}</strong
                    >
                    <span class="text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.loader)
                    }}</span>
                    <strong
                        >{{ sourceLoader }} <span aria-hidden="true">→</span>
                        {{ targetLoader }}</strong
                    >
                </div>
            </Card>
            <Card class="!m-0 p-4">
                <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.strategy) }}
                </h3>
                <p class="mb-0 mt-3 text-lg font-semibold text-brand">{{ strategyLabel }}</p>
            </Card>
        </div>

        <div
            class="grid grid-cols-2 gap-px overflow-hidden rounded-md bg-divider sm:grid-cols-3 lg:grid-cols-6"
        >
            <div v-for="metric in metrics" :key="metric.label" class="bg-surface-2 p-3">
                <div class="text-xl font-semibold text-[var(--color-text-primary)]">
                    {{ metric.value }}
                </div>
                <div class="text-sm text-[var(--color-text-tertiary)]">{{ metric.label }}</div>
            </div>
        </div>

        <section v-if="sharedInstance" class="order-first flex flex-col gap-3">
            <div>
                <h3 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.sharedTitle) }}
                </h3>
                <p class="mb-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.sharedDescription) }}
                </p>
            </div>
            <div class="grid gap-3 md:grid-cols-2">
                <button
                    type="button"
                    class="rounded-md border border-solid p-4 text-left"
                    :class="modeClass('direct')"
                    @click="selectMode('direct')"
                >
                    <strong class="text-[var(--color-text-primary)]">{{
                        formatMessage(messages.direct)
                    }}</strong>
                    <p class="mb-0 mt-2 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.directDescription) }}
                    </p>
                </button>
                <button
                    type="button"
                    class="rounded-md border border-solid p-4 text-left"
                    :class="modeClass('copy_and_upgrade')"
                    @click="selectMode('copy_and_upgrade')"
                >
                    <strong class="text-[var(--color-text-primary)]">{{
                        formatMessage(messages.copy)
                    }}</strong>
                    <p class="mb-0 mt-2 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.copyDescription) }}
                    </p>
                </button>
            </div>
        </section>

        <section class="rounded-md border border-solid border-surface-4 bg-surface-2 p-4">
            <h3 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.backupTitle) }}
            </h3>
            <template v-if="effectiveMode === 'copy_and_upgrade'">
                <p class="mb-0 mt-2 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.copyNoBackup) }}
                </p>
            </template>
            <template v-else>
                <Checkbox
                    v-model="flow.createFullBackup.value"
                    class="mt-3"
                    :label="formatMessage(messages.backupToggle)"
                    @update:model-value="rememberBackupPreference"
                />
                <p class="mb-0 mt-2 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.backupDescription) }}
                </p>
                <Admonition
                    v-if="!flow.createFullBackup.value"
                    class="mt-3"
                    type="warning"
                    :header="formatMessage(messages.backupOffTitle)"
                >
                    {{ formatMessage(messages.backupOffBody) }}
                </Admonition>
            </template>
            <div
                class="mt-4 border-0 border-t border-solid border-divider pt-3 text-sm text-[var(--color-text-tertiary)]"
            >
                <strong class="text-[var(--color-text-primary)]">{{
                    formatMessage(messages.rollbackTitle)
                }}</strong>
                <p class="mb-0 mt-1">{{ formatMessage(messages.rollbackDescription) }}</p>
            </div>
        </section>

        <section>
            <h3 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.preservedTitle) }}
            </h3>
            <p class="mb-0 mt-2 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.preservedBody) }}
            </p>
        </section>
        <section
            v-if="detailGroups.some((group) => group.items.length)"
            class="flex flex-col gap-2"
        >
            <h3 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.details) }}
            </h3>
            <Accordion
                v-for="group in detailGroups.filter((entry) => entry.items.length)"
                :key="group.label"
                button-class="flex w-full items-center gap-2 border-0 bg-transparent p-0 text-left text-[var(--color-text-primary)]"
                content-class="pt-2"
            >
                <template #title>
                    <strong>{{ group.label }}</strong>
                    <span class="text-sm text-[var(--color-text-tertiary)]">{{
                        group.items.length
                    }}</span>
                </template>
                <div class="grid gap-2 sm:grid-cols-2">
                    <div
                        v-for="item in group.items"
                        :key="item.key"
                        class="flex min-w-0 items-center gap-3 rounded-md bg-surface-2 p-3"
                    >
                        <Avatar :src="item.icon" :tint-by="item.title" size="2.5rem" no-shadow />
                        <div class="min-w-0 flex-1">
                            <RouterLink
                                v-if="item.projectPath"
                                :to="item.projectPath"
                                class="inline-flex max-w-full cursor-pointer items-center gap-1 font-semibold text-[var(--color-text-primary)] hover:text-brand hover:underline focus-visible:underline"
                                @click="parkProjectReturn"
                                ><span class="truncate">{{ item.title }}</span
                                ><ExternalIcon class="size-3 shrink-0" aria-hidden="true"
                            /></RouterLink>
                            <div
                                v-else
                                class="truncate font-semibold text-[var(--color-text-primary)]"
                            >
                                {{ item.title }}
                            </div>
                            <div
                                class="flex min-w-0 flex-wrap items-center gap-x-2 text-sm text-[var(--color-text-tertiary)]"
                            >
                                <span>{{ item.providerLabel }}</span>
                                <UpgradeVersionChangelogPopout
                                    v-if="
                                        item.projectPath &&
                                        item.currentReleaseId &&
                                        item.currentLabel
                                    "
                                    :label="item.currentLabel"
                                    :provider="item.provider"
                                    :project-id="item.projectId"
                                    :release-id="item.currentReleaseId"
                                />
                                <span v-else-if="item.currentLabel">{{ item.currentLabel }}</span>
                                <UpgradeVersionChangelogPopout
                                    v-if="
                                        item.projectPath && item.targetReleaseId && item.targetLabel
                                    "
                                    :label="item.targetLabel"
                                    :provider="item.provider"
                                    :project-id="item.projectId"
                                    :release-id="item.targetReleaseId"
                                />
                                <span v-else-if="item.targetLabel">{{ item.targetLabel }}</span>
                                <span v-if="item.stateLabel">{{ item.stateLabel }}</span>
                            </div>
                        </div>
                    </div>
                </div>
            </Accordion>
        </section>
    </section>
</template>

<script setup lang="ts">
import './messages'

import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import Accordion from '@modrinth/ui/src/components/base/Accordion.vue'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Card from '@modrinth/ui/src/components/base/Card.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { buildUpgradeDisplayNames, formatLoaderLabel } from '@modrinth/ui/src/utils/loaders.ts'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import {
    confirmSelectionReleaseSlots,
    confirmSolutionGroups,
    confirmTargetLoaderLabel,
    confirmUpgradeOptions,
    contentIdentityKeys,
    isSharedUpgradeInstance,
    normalizeUpgradePath,
    resolveConfirmDependencyReleases,
    solutionSummary,
    upgradeContentDisplayMetadata,
} from './analysis'
import type {
    ContentProvider,
    InstanceUpgradeDependencyChange,
    InstanceUpgradePlanItem,
    InstanceUpgradeSelection,
    SharedUpgradeMode,
} from './backend'
import { submitInstanceUpgrade } from './backend'
import {
    type InstanceContentData,
    type InstanceContentSnapshotItem,
    loadInstanceContentData,
    localContentIconUrl,
} from './content'
import { attachUpgradeJobToFlow, useInstanceUpgradeFlow } from './flow'
import {
    loadUpgradeProjectDisplayMetadata,
    loadUpgradeVersionDisplayMetadata,
    upgradeProjectDisplayCacheKey,
    type UpgradeProjectIdentity,
    type UpgradeReleaseIdentity,
    upgradeVersionDisplayLabel,
} from './metadata'
import { parkUpgradeFlow, upgradeProjectPath } from './navigation'
import { useQuery } from './query'
import UpgradeVersionChangelogPopout from './UpgradeVersionChangelogPopout.vue'

const messages = defineMessages({
    title: { id: 'instance.upgrade.confirm.title', defaultMessage: 'Confirm upgrade' },
    description: {
        id: 'instance.upgrade.confirm.description',
        defaultMessage: 'Review the changes and backup options before starting the upgrade.',
    },
    environment: { id: 'instance.upgrade.confirm.environment', defaultMessage: 'Environment' },
    loader: { id: 'instance.upgrade.confirm.loader', defaultMessage: 'Loader' },
    automatic: { id: 'instance.upgrade.confirm.loader.automatic', defaultMessage: 'automatic' },
    strategy: { id: 'instance.upgrade.confirm.strategy', defaultMessage: 'Upgrade strategy' },
    newest: {
        id: 'instance.upgrade.confirm.strategy.newest',
        defaultMessage: 'Update as much as possible',
    },
    minimal: {
        id: 'instance.upgrade.confirm.strategy.minimal',
        defaultMessage: 'Change as little as possible',
    },
    custom: { id: 'instance.upgrade.confirm.strategy.custom', defaultMessage: 'Custom' },
    updated: { id: 'instance.upgrade.confirm.updated', defaultMessage: 'Will update' },
    kept: { id: 'instance.upgrade.confirm.kept', defaultMessage: 'Will keep' },
    disabled: { id: 'instance.upgrade.confirm.disabled', defaultMessage: 'Will disable' },
    added: {
        id: 'instance.upgrade.confirm.dependencies-added',
        defaultMessage: 'Dependencies added',
    },
    dependencyUpdated: {
        id: 'instance.upgrade.confirm.dependencies-updated',
        defaultMessage: 'Dependencies updated',
    },
    removed: {
        id: 'instance.upgrade.confirm.dependencies-removed',
        defaultMessage: 'Dependencies removed',
    },
    sharedTitle: {
        id: 'instance.upgrade.confirm.shared.title',
        defaultMessage: 'Shared instance handling',
    },
    sharedDescription: {
        id: 'instance.upgrade.confirm.shared.description',
        defaultMessage:
            'Choose whether to modify the external target or upgrade an independent local copy.',
    },
    direct: { id: 'instance.upgrade.confirm.shared.direct', defaultMessage: 'Direct upgrade' },
    directDescription: {
        id: 'instance.upgrade.confirm.shared.direct-description',
        defaultMessage:
            'Modify the real external target folder. The existing link remains in place.',
    },
    copy: { id: 'instance.upgrade.confirm.shared.copy', defaultMessage: 'Copy and upgrade' },
    copyDescription: {
        id: 'instance.upgrade.confirm.shared.copy-description',
        defaultMessage:
            'Create and upgrade a new local instance. The original link and external target remain untouched.',
    },
    backupTitle: { id: 'instance.upgrade.confirm.backup.title', defaultMessage: 'Full backup' },
    backupToggle: {
        id: 'instance.upgrade.confirm.backup.toggle',
        defaultMessage: 'Create a full backup before upgrading',
    },
    backupDescription: {
        id: 'instance.upgrade.confirm.backup.description',
        defaultMessage:
            'A separate instance copy preserves worlds and configuration for a later return.',
    },
    copyNoBackup: {
        id: 'instance.upgrade.confirm.backup.copy-mode',
        defaultMessage:
            'The original shared instance will not be modified, so no additional full backup will be created.',
    },
    backupOffTitle: {
        id: 'instance.upgrade.confirm.backup.off-title',
        defaultMessage: 'No full backup will be created',
    },
    backupOffBody: {
        id: 'instance.upgrade.confirm.backup.off-body',
        defaultMessage: 'World changes after launching the upgraded game may not be reversible.',
    },
    rollbackTitle: {
        id: 'instance.upgrade.confirm.rollback.title',
        defaultMessage: 'Automatic technical rollback',
    },
    rollbackDescription: {
        id: 'instance.upgrade.confirm.rollback.description',
        defaultMessage:
            'If file changes fail during the upgrade, the launcher automatically rolls back the operation. This is separate from a full user backup.',
    },
    worldTitle: {
        id: 'instance.upgrade.confirm.world.title',
        defaultMessage: 'World saves may be migrated irreversibly',
    },
    worldBody: {
        id: 'instance.upgrade.confirm.world.body',
        defaultMessage:
            'Launching the upgraded instance may migrate worlds to a newer save format. Opening migrated worlds with an older Minecraft version may be unsafe or unsupported. A full backup is strongly recommended.',
    },
    datapackNote: {
        id: 'instance.upgrade.confirm.world.datapacks',
        defaultMessage: 'Datapacks inside {path} are preserved but are not automatically upgraded.',
    },
    preservedTitle: {
        id: 'instance.upgrade.confirm.preserved.title',
        defaultMessage: 'Preserved data',
    },
    preservedBody: {
        id: 'instance.upgrade.confirm.preserved.body',
        defaultMessage:
            'Worlds, options, servers, and configuration files are preserved. Mods, resource packs, and shaders follow the selected upgrade solution.',
    },
    details: { id: 'instance.upgrade.confirm.details', defaultMessage: 'Change details' },
    updatedContent: {
        id: 'instance.upgrade.confirm.details.updated',
        defaultMessage: 'Updated content',
    },
    keptContent: {
        id: 'instance.upgrade.confirm.details.kept',
        defaultMessage: 'Kept content',
    },
    disabledContent: {
        id: 'instance.upgrade.confirm.details.disabled',
        defaultMessage: 'Disabled content',
    },
    dependencyChanges: {
        id: 'instance.upgrade.confirm.details.dependencies',
        defaultMessage: 'Dependency changes',
    },
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
    dependencyFallback: {
        id: 'instance.upgrade.confirm.details.new-dependency',
        defaultMessage: 'New {provider} dependency',
    },
    contentFallback: {
        id: 'instance.upgrade.confirm.details.content-item',
        defaultMessage: 'Content item',
    },
    versionTarget: {
        id: 'instance.upgrade.confirm.details.version-target',
        defaultMessage: 'Target {version}',
    },
    versionCurrent: {
        id: 'instance.upgrade.confirm.details.version-current',
        defaultMessage: 'Current {version}',
    },
    disabledState: {
        id: 'instance.upgrade.confirm.details.disabled-state',
        defaultMessage: 'Disabled',
    },
    back: { id: 'instance.upgrade.confirm.back', defaultMessage: 'Back' },
    start: { id: 'instance.upgrade.confirm.start', defaultMessage: 'Start upgrade' },
    starting: { id: 'instance.upgrade.confirm.starting', defaultMessage: 'Starting upgrade…' },
    startError: {
        id: 'instance.upgrade.confirm.start-error',
        defaultMessage: 'Unable to start upgrade',
    },
    activeJob: {
        id: 'instance.upgrade.confirm.active-job',
        defaultMessage: 'An upgrade is already in progress',
    },
    backupInstanceName: {
        id: 'instance.upgrade.confirm.backup.instance-name',
        defaultMessage: '{name} (Pre-upgrade backup)',
    },
    copyInstanceName: {
        id: 'instance.upgrade.confirm.copy.instance-name',
        defaultMessage: '{name} (Upgraded copy)',
    },
})

const flow = useInstanceUpgradeFlow()
const route = useRoute()
const router = useRouter()
const { formatMessage } = useVIntl()
const datapackPath = 'saves/<world>/datapacks'
const submissionLock = ref(false)
const executionError = ref<string | null>(null)
const plan = computed(() => flow.plan.value!)
const solution = computed(() => plan.value.selectedSolution!)
const sharedInstance = computed(() => isSharedUpgradeInstance(flow.instance.value))
const confirmOptions = computed(() =>
    confirmUpgradeOptions(
        sharedInstance.value,
        flow.sharedUpgradeMode.value,
        flow.directFullBackupPreference.value,
    ),
)
const effectiveMode = computed(() => confirmOptions.value.effectiveMode)
const routeInstanceId = computed(() =>
    Array.isArray(route.params.id) ? route.params.id[0] : route.params.id,
)
const submissionBusy = computed(() => flow.busy.value || submissionLock.value)
const canStartUpgrade = computed(
    () =>
        flow.plan.value !== null &&
        flow.plan.value.blockingIssues.length === 0 &&
        flow.plan.value.selectedSolution !== null &&
        !flow.busy.value &&
        !submissionLock.value &&
        flow.activeJobId.value === null &&
        routeInstanceId.value === flow.instanceId.value &&
        confirmOptions.value.canStart,
)
const summary = computed(() => solutionSummary(solution.value))
const metrics = computed(() => [
    { label: formatMessage(messages.updated), value: summary.value.upgraded },
    { label: formatMessage(messages.kept), value: summary.value.kept },
    { label: formatMessage(messages.disabled), value: summary.value.disabled },
    { label: formatMessage(messages.added), value: summary.value.dependencyAdditions },
    { label: formatMessage(messages.dependencyUpdated), value: summary.value.dependencyUpdates },
    { label: formatMessage(messages.removed), value: summary.value.dependencyRemovals },
])
const sourceLoader = computed(() =>
    loaderLabel(
        plan.value.sourceEnvironment.modLoader,
        plan.value.sourceEnvironment.modLoaderVersion,
    ),
)
const targetLoader = computed(() =>
    confirmTargetLoaderLabel(
        formatLoaderLabel(plan.value.targetEnvironment.modLoader),
        plan.value.targetEnvironment.modLoader,
        plan.value.targetEnvironment.modLoaderVersion,
        formatMessage(messages.automatic),
    ),
)
const displayNames = computed(() => {
    const instance = flow.instance.value
    if (!instance)
        return { backup: null, copy: null, upgradedTarget: null, shouldAutoRename: false }
    return buildUpgradeDisplayNames({
        sourceName: instance.name,
        sourceLoader: instance.loader,
        sourceGameVersion: instance.game_version,
        sourceLoaderVersion: instance.loader_version ?? null,
        targetLoader: plan.value.targetEnvironment.modLoader,
        targetGameVersion: plan.value.targetEnvironment.gameVersion,
        targetLoaderVersion: plan.value.targetEnvironment.modLoaderVersion,
        backupName: formatMessage(messages.backupInstanceName, { name: instance.name }),
        customCopyName: formatMessage(messages.copyInstanceName, { name: instance.name }),
    })
})
const strategyLabel = computed(
    () =>
        ({
            newest: formatMessage(messages.newest),
            minimal_change: formatMessage(messages.minimal),
            custom: formatMessage(messages.custom),
        })[solution.value.kind],
)

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
const itemByContentId = computed(
    () => new Map(plan.value.items.map((item) => [item.contentId, item])),
)
const itemByProviderProject = computed(
    () =>
        new Map(
            plan.value.items.flatMap((item) =>
                item.provider && item.projectId
                    ? [[`${item.provider}:${item.projectId}`, item] as const]
                    : [],
            ),
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
    for (const selection of solution.value.selections) {
        add(selection.provider, selection.projectId, selection.currentReleaseId)
        add(selection.provider, selection.projectId, selection.targetReleaseId)
    }
    for (const change of solution.value.dependencyChanges) {
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
const projectIdentities = computed(() => {
    const identities: UpgradeProjectIdentity[] = []
    const add = (provider: ContentProvider | null, projectId: string | null) => {
        if ((provider === 'modrinth' || provider === 'curseforge') && projectId) {
            identities.push({ provider, projectId })
        }
    }
    for (const selection of solution.value.selections) add(selection.provider, selection.projectId)
    for (const change of solution.value.dependencyChanges) add(change.provider, change.projectId)
    return identities
})
const projectMetadataQuery = useQuery({
    queryKey: computed(() => [
        'instance-upgrade',
        'project-display',
        ...projectIdentities.value.map((identity) => `${identity.provider}:${identity.projectId}`),
    ]),
    queryFn: () => loadUpgradeProjectDisplayMetadata(projectIdentities.value),
    staleTime: Number.POSITIVE_INFINITY,
})

interface ConfirmDetailRow {
    key: string
    title: string
    icon: string
    provider: ContentProvider | null
    providerLabel: string
    projectId: string | null
    projectPath: string | null
    currentReleaseId: string | null
    currentLabel: string | null
    targetReleaseId: string | null
    targetLabel: string | null
    stateLabel: string | null
}

const groupedChanges = computed(() => confirmSolutionGroups(solution.value))
const detailGroups = computed(() => [
    {
        label: formatMessage(messages.updatedContent),
        items: groupedChanges.value.updated.map(selectionDetail),
    },
    {
        label: formatMessage(messages.keptContent),
        items: groupedChanges.value.kept.map(selectionDetail),
    },
    {
        label: formatMessage(messages.disabledContent),
        items: groupedChanges.value.disabled.map(selectionDetail),
    },
    {
        label: formatMessage(messages.dependencyChanges),
        items: groupedChanges.value.dependencyChanges.map(dependencyDetail),
    },
])

function loaderLabel(
    loader: typeof plan.value.sourceEnvironment.modLoader,
    version: string | null,
) {
    const label = formatLoaderLabel(loader)
    return version ? `${label} ${version}` : label
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
): string | null {
    if (!releaseId) return fallback ?? null
    if (!provider || !projectId) return fallback ?? releaseId
    const resolved = upgradeVersionDisplayLabel(versionMetadataQuery.data.value, {
        provider,
        projectId,
        releaseId,
    })
    return resolved === releaseId && fallback ? fallback : resolved
}

function projectMetadata(provider: ContentProvider | null, projectId: string | null) {
    if (!provider || !projectId) return null
    return (
        projectMetadataQuery.data.value?.get(upgradeProjectDisplayCacheKey(provider, projectId)) ??
        null
    )
}

function selectionDetail(selection: InstanceUpgradeSelection): ConfirmDetailRow {
    const item = itemByContentId.value.get(selection.contentId)
    const metadata = item ? contentMetadata(item) : null
    const providerMetadata = projectMetadata(selection.provider, selection.projectId)
    const releases = confirmSelectionReleaseSlots(selection)
    const current = releaseLabel(
        selection.provider,
        selection.projectId,
        releases.currentReleaseId,
        metadata?.currentVersion,
    )
    const target = releaseLabel(selection.provider, selection.projectId, releases.targetReleaseId)
    return {
        key: selection.contentId,
        title:
            metadata?.title ?? providerMetadata?.title ?? formatMessage(messages.contentFallback),
        icon: localContentIconUrl(metadata?.iconUrl ?? providerMetadata?.iconUrl),
        provider: selection.provider,
        providerLabel: providerLabel(selection.provider),
        projectId: selection.projectId,
        projectPath: upgradeProjectPath(selection.provider, selection.projectId),
        currentReleaseId: releases.currentReleaseId,
        currentLabel: current ? formatMessage(messages.versionCurrent, { version: current }) : null,
        targetReleaseId: releases.targetReleaseId,
        targetLabel: target ? formatMessage(messages.versionTarget, { version: target }) : null,
        stateLabel: selection.action === 'disable' ? formatMessage(messages.disabledState) : null,
    }
}

function dependencyDetail(change: InstanceUpgradeDependencyChange): ConfirmDetailRow {
    const item =
        (change.existingContentId ? itemByContentId.value.get(change.existingContentId) : null) ??
        itemByProviderProject.value.get(`${change.provider}:${change.projectId}`)
    const metadata = item ? contentMetadata(item) : null
    const providerMetadata = projectMetadata(change.provider, change.projectId)
    const releases = resolveConfirmDependencyReleases(change, (releaseId, slot) =>
        releaseLabel(
            change.provider,
            change.projectId,
            releaseId,
            slot === 'current' ? metadata?.currentVersion : undefined,
        ),
    )
    return {
        key: `${change.provider}:${change.projectId}:${change.existingContentId ?? 'new'}:${change.kind}`,
        title:
            metadata?.title ??
            providerMetadata?.title ??
            formatMessage(messages.dependencyFallback, {
                provider: providerLabel(change.provider),
            }),
        icon: localContentIconUrl(metadata?.iconUrl ?? providerMetadata?.iconUrl),
        provider: change.provider,
        providerLabel: providerLabel(change.provider),
        projectId: change.projectId,
        projectPath: upgradeProjectPath(change.provider, change.projectId),
        currentReleaseId: releases.currentReleaseId,
        currentLabel: releases.current
            ? formatMessage(messages.versionCurrent, { version: releases.current })
            : null,
        targetReleaseId: releases.targetReleaseId,
        targetLabel: releases.target
            ? formatMessage(messages.versionTarget, { version: releases.target })
            : null,
        stateLabel: null,
    }
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

function selectMode(mode: SharedUpgradeMode) {
    const previousMode = flow.sharedUpgradeMode.value
    flow.sharedUpgradeMode.value = mode
    if (mode === 'copy_and_upgrade') {
        if (previousMode !== 'copy_and_upgrade') {
            flow.directFullBackupPreference.value = flow.createFullBackup.value
        }
        flow.createFullBackup.value = false
    } else {
        flow.createFullBackup.value = flow.directFullBackupPreference.value
    }
}

function rememberBackupPreference(value: boolean) {
    flow.directFullBackupPreference.value = value
}

function modeClass(mode: SharedUpgradeMode) {
    return flow.sharedUpgradeMode.value === mode
        ? 'border-brand bg-surface-2 ring-1 ring-brand'
        : 'border-surface-4 bg-surface-2 hover:bg-surface-3'
}

function errorMessage(error: unknown): string {
    if (error instanceof Error) return error.message
    if (typeof error === 'string') return error
    if (typeof error === 'object' && error && 'message' in error) return String(error.message)
    return String(error)
}

async function startUpgrade() {
    if (!canStartUpgrade.value || !flow.plan.value || !effectiveMode.value) return
    flow.busy.value = true
    executionError.value = null
    try {
        const submitted = await submitInstanceUpgrade(
            {
                instanceId: flow.instanceId.value,
                planId: flow.plan.value.id,
                createFullBackup: confirmOptions.value.createFullBackup,
                sharedUpgradeMode: effectiveMode.value,
                displayNames: displayNames.value,
            },
            submissionLock,
        )
        if (!submitted) return
        await router.replace(attachUpgradeJobToFlow(flow, submitted.job))
    } catch (error) {
        executionError.value = errorMessage(error)
    } finally {
        flow.busy.value = false
    }
}

function registerControls() {
    flow.registerStepControls({
        canNext: canStartUpgrade,
        busy: submissionBusy,
        nextLabel: formatMessage(submissionBusy.value ? messages.starting : messages.start),
        onNext: startUpgrade,
        onBack: () =>
            router.push(`/instance/${encodeURIComponent(flow.instanceId.value)}/upgrade/customize`),
    })
}

onMounted(() => {
    if (!sharedInstance.value) {
        flow.sharedUpgradeMode.value = 'direct'
        flow.createFullBackup.value = confirmOptions.value.createFullBackup
    }
    registerControls()
})
watch([canStartUpgrade, submissionBusy], registerControls)
onBeforeUnmount(() => flow.registerStepControls(null))
</script>
