<template>
    <section class="flex flex-col gap-6 py-2">
        <header class="flex items-start gap-3">
            <CheckCircleIcon class="mt-0.5 size-8 shrink-0 text-green" aria-hidden="true" />
            <div>
                <h2 class="m-0 text-xl font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.title) }}
                </h2>
                <p class="mb-0 mt-1 text-[var(--color-text-tertiary)]">
                    {{
                        formatMessage(
                            mode === 'copy_and_upgrade'
                                ? messages.copyDescription
                                : messages.directDescription,
                        )
                    }}
                </p>
            </div>
        </header>

        <div class="grid gap-3 md:grid-cols-2">
            <Card class="!m-0 p-4">
                <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.environment) }}
                </h3>
                <div class="mt-3 grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
                    <span class="text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.minecraft)
                    }}</span>
                    <strong
                        >{{ sourceEnvironment?.gameVersion ?? formatMessage(messages.unknown) }}
                        <span aria-hidden="true">→</span>
                        {{
                            targetEnvironment?.gameVersion ?? formatMessage(messages.unknown)
                        }}</strong
                    >
                    <span class="text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.loader)
                    }}</span>
                    <strong
                        >{{ loaderLabel(sourceEnvironment) }} <span aria-hidden="true">→</span>
                        {{ loaderLabel(actualTargetEnvironment) }}</strong
                    >
                </div>
            </Card>
            <Card class="!m-0 p-4">
                <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.metrics) }}
                </h3>
                <div class="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-3">
                    <div v-for="metric in metrics" :key="metric.label">
                        <div class="text-xl font-semibold text-[var(--color-text-primary)]">
                            {{ metric.value }}
                        </div>
                        <div class="text-xs text-[var(--color-text-tertiary)]">
                            {{ metric.label }}
                        </div>
                    </div>
                </div>
            </Card>
        </div>

        <div class="flex flex-wrap gap-2">
            <Button
                type="colored"
                color="brand"
                @click="router.push(`/instance/${encodeURIComponent(result.targetInstanceId)}`)"
                ><ExternalIcon />{{ formatMessage(messages.openUpgraded) }}
            </Button>
            <Button
                v-if="mode === 'copy_and_upgrade'"
                type="outlined"
                @click="router.push(`/instance/${encodeURIComponent(result.sourceInstanceId)}`)"
                ><ExternalIcon />{{ formatMessage(messages.openOriginal) }}
            </Button>
            <Button
                v-if="result.backupInstanceId"
                type="outlined"
                @click="router.push(`/instance/${encodeURIComponent(result.backupInstanceId!)}`)"
                ><FolderOpenIcon />{{ formatMessage(messages.openBackup) }}
            </Button>
        </div>

        <Card v-if="result.backupInstanceId" class="!m-0 p-4">
            <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.backupTitle) }}
            </h3>
            <p class="mb-3 mt-1 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.backupDescription) }}
            </p>
        </Card>
        <Admonition
            v-else-if="mode === 'direct'"
            type="info"
            :header="formatMessage(messages.noBackupTitle)"
            >{{ formatMessage(messages.noBackupDescription) }}</Admonition
        >

        <Admonition
            v-if="result.externalChanges.length"
            type="warning"
            :header="formatMessage(messages.externalTitle)"
        >
            <p class="mb-2">{{ formatMessage(messages.externalDescription) }}</p>
            <ul class="m-0 list-disc pl-5">
                <li
                    v-for="change in result.externalChanges"
                    :key="`${change.kind}:${change.relativePath}`"
                >
                    <code>{{ change.relativePath }}</code> · {{ externalChangeLabel(change.kind) }}
                </li>
            </ul>
        </Admonition>
        <Admonition
            v-if="result.skippedDueToExternalConflict.length"
            type="warning"
            :header="formatMessage(messages.skippedTitle)"
        >
            <p class="mb-2">{{ formatMessage(messages.skippedDescription) }}</p>
            <ul class="m-0 list-disc pl-5">
                <li v-for="path in result.skippedDueToExternalConflict" :key="path">
                    <code>{{ path }}</code>
                </li>
            </ul>
        </Admonition>
        <UpgradeResultCollections
            :result="result"
            :target-version="targetEnvironment?.gameVersion ?? null"
        />
    </section>
</template>

<script setup lang="ts">
import './messages'

import CheckCircleIcon from '@modrinth/assets/icons/check-circle.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Card from '@modrinth/ui/src/components/base/Card.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { formatLoaderLabel } from '@modrinth/ui/src/utils/loaders.ts'
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import type { InstanceUpgradeExternalChangeKind, InstanceUpgradeTargetEnvironment } from './backend'
import { get_many as getInstances } from './backend'
import { useQuery } from './query'
import { summarizeUpgradeResult, upgradeResultMode } from './result'
import UpgradeResultCollections from './UpgradeResultCollections.vue'

const messages = defineMessages({
    title: { id: 'instance.upgrade.result.title', defaultMessage: 'Upgrade complete' },
    directDescription: {
        id: 'instance.upgrade.result.direct-description',
        defaultMessage: 'This instance was upgraded successfully.',
    },
    copyDescription: {
        id: 'instance.upgrade.result.copy-description',
        defaultMessage:
            'An upgraded copy was created successfully. The original shared instance was left unchanged.',
    },
    environment: { id: 'instance.upgrade.result.environment', defaultMessage: 'Environment' },
    minecraft: { id: 'instance.upgrade.result.minecraft', defaultMessage: 'Minecraft' },
    loader: { id: 'instance.upgrade.result.loader', defaultMessage: 'Loader' },
    unknown: { id: 'instance.upgrade.result.unknown', defaultMessage: 'Unavailable' },
    automatic: { id: 'instance.upgrade.result.automatic', defaultMessage: 'Automatic' },
    metrics: { id: 'instance.upgrade.result.metrics', defaultMessage: 'Outcome' },
    updated: { id: 'instance.upgrade.result.updated', defaultMessage: 'Updated' },
    kept: { id: 'instance.upgrade.result.kept', defaultMessage: 'Kept' },
    disabled: { id: 'instance.upgrade.result.disabled', defaultMessage: 'Disabled' },
    added: {
        id: 'instance.upgrade.result.dependencies-added',
        defaultMessage: 'Dependencies added',
    },
    dependencyUpdated: {
        id: 'instance.upgrade.result.dependencies-updated',
        defaultMessage: 'Dependencies updated',
    },
    removed: {
        id: 'instance.upgrade.result.dependencies-removed',
        defaultMessage: 'Dependencies removed',
    },
    openUpgraded: {
        id: 'instance.upgrade.result.open-upgraded',
        defaultMessage: 'Open upgraded instance',
    },
    openOriginal: {
        id: 'instance.upgrade.result.open-original',
        defaultMessage: 'Open original instance',
    },
    backupTitle: { id: 'instance.upgrade.result.backup-title', defaultMessage: 'Backup created' },
    backupDescription: {
        id: 'instance.upgrade.result.backup-description',
        defaultMessage:
            'A complete pre-upgrade copy was created separately from automatic technical rollback.',
    },
    openBackup: { id: 'instance.upgrade.result.open-backup', defaultMessage: 'Open backup' },
    noBackupTitle: {
        id: 'instance.upgrade.result.no-backup-title',
        defaultMessage: 'No complete backup was created',
    },
    noBackupDescription: {
        id: 'instance.upgrade.result.no-backup-description',
        defaultMessage:
            'Automatic technical rollback protected this operation while it was running; it is not a permanent backup.',
    },
    externalTitle: {
        id: 'instance.upgrade.result.external-title',
        defaultMessage: 'Changes detected while upgrading',
    },
    externalDescription: {
        id: 'instance.upgrade.result.external-description',
        defaultMessage:
            'Files changed outside the launcher were detected, and user changes were given priority where applicable.',
    },
    skippedTitle: {
        id: 'instance.upgrade.result.skipped-title',
        defaultMessage: 'Some planned changes were skipped',
    },
    skippedDescription: {
        id: 'instance.upgrade.result.skipped-description',
        defaultMessage: 'These files changed while upgrading, so the user changes were preserved.',
    },
    warningsTitle: {
        id: 'instance.upgrade.result.warnings-title',
        defaultMessage: 'Compatibility warnings',
    },
    warningPrereleaseOnly: {
        id: 'instance.upgrade.warning.prerelease-only',
        defaultMessage: '{path} only has prerelease builds for the target environment.',
    },
    warningUnidentified: {
        id: 'instance.upgrade.warning.unidentified',
        defaultMessage: '{path} could not be identified and was preserved unchanged.',
    },
    warningDependencyConflict: {
        id: 'instance.upgrade.warning.dependency-conflict',
        defaultMessage: '{path} has conflicting dependency requirements.',
    },
    warningMissingDependency: {
        id: 'instance.upgrade.warning.missing-required-dependency',
        defaultMessage: '{path} requires a dependency that could not be resolved.',
    },
    warningIncompatibleDependency: {
        id: 'instance.upgrade.warning.incompatible-dependency',
        defaultMessage: '{path} has an incompatible dependency.',
    },
    warningUnsupportedType: {
        id: 'instance.upgrade.warning.unsupported-content-type',
        defaultMessage: '{path} uses a content type that cannot be upgraded automatically.',
    },
    warningNoRelease: {
        id: 'instance.upgrade.warning.no-compatible-release',
        defaultMessage: '{path} has no compatible release for the target environment.',
    },
    warningNoShaderRuntime: {
        id: 'instance.upgrade.warning.no-compatible-shader-runtime',
        defaultMessage: '{path} has no release compatible with the target shader runtime.',
    },
    warningShaderMissing: {
        id: 'instance.upgrade.warning.shader-runtime-missing',
        defaultMessage: '{path} was preserved because no target shader runtime is configured.',
    },
    warningShaderUnknown: {
        id: 'instance.upgrade.warning.shader-runtime-unknown',
        defaultMessage: '{path} was preserved because the target shader runtime is unknown.',
    },
    warningSearchLimit: {
        id: 'instance.upgrade.warning.search-limit-reached',
        defaultMessage: '{path} could not be resolved within the bounded compatibility search.',
    },
    warningKeepIncompatible: {
        id: 'instance.upgrade.warning.keep-incompatible',
        defaultMessage:
            '{path} was kept unchanged and may be incompatible with the upgraded instance.',
    },
    detailsTitle: {
        id: 'instance.upgrade.result.details-title',
        defaultMessage: 'Upgrade details',
    },
    add: { id: 'instance.upgrade.result.action-add', defaultMessage: 'Added' },
    upgrade: { id: 'instance.upgrade.result.action-upgrade', defaultMessage: 'Updated' },
    keep: { id: 'instance.upgrade.result.action-keep', defaultMessage: 'Kept' },
    disable: { id: 'instance.upgrade.result.action-disable', defaultMessage: 'Disabled' },
    remove: { id: 'instance.upgrade.result.action-remove', defaultMessage: 'Removed' },
    changeAdded: { id: 'instance.upgrade.result.change-added', defaultMessage: 'Added' },
    changeRemoved: { id: 'instance.upgrade.result.change-removed', defaultMessage: 'Removed' },
    changeModified: { id: 'instance.upgrade.result.change-modified', defaultMessage: 'Modified' },
})

const props = defineProps<{ result: import('./backend').InstanceUpgradeResult }>()
const router = useRouter()
const { formatMessage } = useVIntl()
const result = computed(() => props.result)
const mode = computed(() => upgradeResultMode(result.value))
const targetEnvironment = computed(() => result.value.targetEnvironment ?? null)
const sourceEnvironment = computed(() => result.value.sourceEnvironment ?? null)
const relatedInstancesQuery = useQuery({
    queryKey: computed(() => [
        'instance-upgrade',
        'result-instances',
        result.value.sourceInstanceId,
        result.value.targetInstanceId,
        result.value.backupInstanceId,
    ]),
    queryFn: () =>
        getInstances([
            result.value.sourceInstanceId,
            result.value.targetInstanceId,
            ...(result.value.backupInstanceId ? [result.value.backupInstanceId] : []),
        ]).catch(() => []),
    staleTime: Number.POSITIVE_INFINITY,
})
const targetInstance = computed(
    () =>
        relatedInstancesQuery.data.value?.find(
            (instance) => instance.id === result.value.targetInstanceId,
        ) ?? null,
)
const actualTargetEnvironment = computed<InstanceUpgradeTargetEnvironment | null>(() =>
    targetInstance.value
        ? {
              gameVersion: targetInstance.value.game_version,
              modLoader: targetInstance.value.loader,
              modLoaderVersion:
                  targetInstance.value.loader_version ??
                  targetEnvironment.value?.modLoaderVersion ??
                  null,
              shaderRuntime: targetEnvironment.value?.shaderRuntime ?? 'unknown',
          }
        : targetEnvironment.value,
)
const summary = computed(() => summarizeUpgradeResult(result.value.solution))
const metrics = computed(() => [
    { label: formatMessage(messages.updated), value: summary.value.updated },
    { label: formatMessage(messages.kept), value: summary.value.kept },
    { label: formatMessage(messages.disabled), value: summary.value.disabled },
    { label: formatMessage(messages.added), value: summary.value.dependencyAdded },
    { label: formatMessage(messages.dependencyUpdated), value: summary.value.dependencyUpdated },
    { label: formatMessage(messages.removed), value: summary.value.dependencyRemoved },
])

function loaderLabel(environment: InstanceUpgradeTargetEnvironment | null) {
    if (!environment) return formatMessage(messages.unknown)
    const label = formatLoaderLabel(environment.modLoader)
    return environment.modLoaderVersion
        ? `${label} ${environment.modLoaderVersion}`
        : `${label} (${formatMessage(messages.automatic)})`
}
function externalChangeLabel(kind: InstanceUpgradeExternalChangeKind) {
    return formatMessage(
        messages[
            kind === 'added'
                ? 'changeAdded'
                : kind === 'removed'
                  ? 'changeRemoved'
                  : 'changeModified'
        ],
    )
}
</script>
