<script setup lang="ts">
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import UndoIcon from '@modrinth/assets/icons/undo.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import { backupSettingsMessages as messages } from '../messages'
import BackupExclusionSelector from './BackupExclusionSelector.vue'
import {
    type BackupExclusion,
    type BackupOperation,
    type BackupOperationState,
    type BackupProgressEvent,
    type BackupRestorePreview,
    type BackupSnapshot,
    type InstanceBackupConfig,
    type InstanceBackupEligibility,
    useInstanceBackups,
} from './backups'
import { injectInstanceSettings } from './context'
import { useSimulatedInvoke } from './state'

/**
 * Port of `apps/app-frontend/src/components/ui/instance_settings/BackupSettings.vue`
 * over the preview's backup service (`./backups`), which reports progress the way
 * the `instance_backup_progress` event does.
 */
const { instance } = injectInstanceSettings()
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const formatDate = useFormatDateTime({ dateStyle: 'medium', timeStyle: 'short' })
const { handleError } = injectNotificationManager()
const backups = useInstanceBackups()
const simulate = useSimulatedInvoke()

const config = ref<InstanceBackupConfig | null>(null)
const snapshots = ref<BackupSnapshot[]>([])
const instanceRoot = ref('')
const loading = ref(true)
const editingExclusions = ref(false)
const excludedPaths = ref<BackupExclusion[]>([])
const action = ref<string | null>(null)
const operation = ref<BackupOperation | null>(null)
const selectedSnapshot = ref<BackupSnapshot | null>(null)
const restorePreview = ref<BackupRestorePreview | null>(null)
const deleteModal = ref<InstanceType<typeof PreviewModal>>()
const restoreModal = ref<InstanceType<typeof PreviewModal>>()
let unlisten: (() => void) | null = null

const eligible = computed(() => config.value?.eligibility === 'eligible')
const terminalStates: BackupOperationState[] = ['completed', 'cancelled', 'failed', 'interrupted']
const isRunning = computed(
    () => operation.value !== null && !terminalStates.includes(operation.value.state),
)
const canCancel = computed(() => operation.value?.cancellable === true)
const operationLabel = computed(() => {
    if (operation.value?.operation_type === 'restore_preview') {
        return formatMessage(messages.stagePreview)
    }
    switch (operation.value?.state) {
        case 'queued':
            return formatMessage(messages.stageQueued)
        case 'hashing':
            return formatMessage(messages.stageHashing)
        case 'saving':
            return formatMessage(messages.stageSaving)
        case 'validating':
            return formatMessage(messages.stageValidating)
        case 'materializing':
            return formatMessage(messages.stageMaterializing)
        case 'applying':
            return formatMessage(messages.stageApplying)
        default:
            return formatMessage(messages.stageScanning)
    }
})

function operationFromEvent(event: BackupProgressEvent): BackupOperation {
    const now = Date.now()
    const state: BackupOperationState =
        event.stage === 'restoring' || event.stage === 'copying'
            ? 'materializing'
            : event.stage === 'deleting'
              ? 'queued'
              : event.stage
    return {
        id: event.operationId,
        operation_type: event.operationType,
        instance_id: event.instanceId ?? instance.value.id,
        snapshot_id: event.snapshotId,
        state,
        processed_bytes: event.processedBytes,
        total_bytes: event.totalBytes,
        cancellable: event.stage === 'scanning' || event.stage === 'hashing',
        cancel_requested: false,
        error: event.message,
        created_at: operation.value?.id === event.operationId ? operation.value.created_at : now,
        updated_at: now,
        finished_at: ['completed', 'cancelled', 'failed'].includes(event.stage) ? now : undefined,
    }
}

async function listBackupOperations(instanceId: string) {
    await simulate(60)
    return backups.listOperations(instanceId)
}

async function refreshOperation() {
    const operations = await listBackupOperations(instance.value.id)
    const latest = operations[0] ?? null
    operation.value =
        latest && (isRunningState(latest.state) || ['failed', 'interrupted'].includes(latest.state))
            ? latest
            : null
    if (operation.value?.operation_type === 'restore_preview' && operation.value.snapshot_id) {
        selectedSnapshot.value =
            snapshots.value.find((snapshot) => snapshot.id === operation.value?.snapshot_id) ?? null
    }
}

function isRunningState(state: BackupOperationState) {
    return !terminalStates.includes(state)
}

const eligibilityMessages: Record<
    Exclude<InstanceBackupEligibility, 'eligible'>,
    keyof typeof messages
> = {
    not_installed: 'notInstalled',
    symlink_instance: 'symlinkInstance',
    direct_linked_instance: 'directLinked',
    external_game_directory: 'externalDirectory',
    root_missing: 'rootMissing',
    root_is_link: 'rootLink',
}

const unavailableReason = computed(() => {
    const eligibility = config.value?.eligibility
    if (!eligibility || eligibility === 'eligible') return ''
    return formatMessage(messages[eligibilityMessages[eligibility]])
})

async function refresh() {
    loading.value = true
    try {
        await simulate(140)
        const nextConfig = backups.configFor(instance.value.id)
        config.value = nextConfig
        excludedPaths.value = nextConfig.enabled ? [...nextConfig.excluded_paths] : []
        instanceRoot.value = nextConfig.eligibility === 'eligible' ? instance.value.path : ''
        snapshots.value = backups.list(instance.value.id)
        await refreshOperation()
    } catch (error) {
        handleError(error)
    } finally {
        loading.value = false
    }
}

async function runAction(name: string, task: () => Promise<void>) {
    action.value = name
    try {
        await task()
    } catch (error) {
        handleError(error)
    } finally {
        action.value = null
    }
}

function enable() {
    void runAction('enable', async () => {
        await simulate()
        config.value = backups.enable(instance.value.id, excludedPaths.value)
        editingExclusions.value = false
    })
}

function beginEditingExclusions() {
    excludedPaths.value = [...(config.value?.excluded_paths ?? [])]
    editingExclusions.value = true
}

function saveExclusions() {
    void runAction('exclusions', async () => {
        await simulate()
        config.value = backups.updateExclusions(instance.value.id, excludedPaths.value)
        editingExclusions.value = false
    })
}

function start() {
    void runAction('start', async () => {
        await simulate(120)
        const id = backups.start(instance.value.id)
        operation.value = {
            id,
            operation_type: 'create',
            instance_id: instance.value.id,
            state: 'queued',
            processed_bytes: 0,
            total_bytes: 0,
            cancellable: true,
            cancel_requested: false,
            created_at: Date.now(),
            updated_at: Date.now(),
        }
    })
}

function cancel() {
    if (!operation.value || !canCancel.value) return
    void runAction('cancel', async () => {
        await simulate(80)
        if (backups.cancel(operation.value!.id)) {
            operation.value = {
                ...operation.value!,
                cancellable: false,
                cancel_requested: true,
            }
        }
    })
}

function confirmDelete(snapshot: BackupSnapshot) {
    selectedSnapshot.value = snapshot
    deleteModal.value?.show()
}

function confirmRestore(snapshot: BackupSnapshot) {
    void runAction(`preview:${snapshot.id}`, async () => {
        selectedSnapshot.value = snapshot
        await simulate(120)
        const id = backups.startRestorePreview(snapshot.id)
        operation.value = {
            id,
            operation_type: 'restore_preview',
            instance_id: instance.value.id,
            snapshot_id: snapshot.id,
            state: 'queued',
            processed_bytes: 0,
            total_bytes: snapshot.logical_size,
            cancellable: true,
            cancel_requested: false,
            created_at: Date.now(),
            updated_at: Date.now(),
        }
    })
}

function removeSnapshot() {
    if (!selectedSnapshot.value) return
    void runAction(`delete:${selectedSnapshot.value.id}`, async () => {
        await simulate(420)
        backups.remove(selectedSnapshot.value!.id)
        await refresh()
        deleteModal.value?.hide()
        selectedSnapshot.value = null
    })
}

function restoreSnapshot() {
    if (!selectedSnapshot.value || !restorePreview.value) return
    void runAction(`restore:${selectedSnapshot.value.id}`, async () => {
        await simulate(120)
        const id = backups.restore(selectedSnapshot.value!.id, restorePreview.value!.plan_token)
        operation.value = {
            id,
            operation_type: 'restore',
            instance_id: instance.value.id,
            snapshot_id: selectedSnapshot.value!.id,
            state: 'queued',
            processed_bytes: 0,
            total_bytes: selectedSnapshot.value!.logical_size,
            cancellable: true,
            cancel_requested: false,
            created_at: Date.now(),
            updated_at: Date.now(),
        }
        restoreModal.value?.hide()
        selectedSnapshot.value = null
        restorePreview.value = null
    })
}

function disable() {
    void runAction('disable', async () => {
        await simulate()
        backups.disable(instance.value.id)
        await refresh()
    })
}

onMounted(async () => {
    unlisten = backups.listen(async (event) => {
        if (
            !['create', 'restore', 'restore_preview'].includes(event.operationType) ||
            event.instanceId !== instance.value.id
        )
            return
        operation.value = operationFromEvent(event)
        if (event.operationType === 'restore_preview' && event.stage === 'completed') {
            const completed = (await listBackupOperations(instance.value.id)).find(
                (candidate) => candidate.id === event.operationId,
            )
            const snapshot = snapshots.value.find((candidate) => candidate.id === event.snapshotId)
            if (completed?.restore_preview && snapshot) {
                selectedSnapshot.value = snapshot
                restorePreview.value = completed.restore_preview
                operation.value = null
                restoreModal.value?.show()
                return
            }
        }
        if (['completed', 'cancelled', 'failed'].includes(event.stage)) {
            void refresh()
        }
    })
    await refresh()
})

onUnmounted(() => unlisten?.())
watch(
    () => instance.value.id,
    () => void refresh(),
)
</script>

<template>
    <div class="flex flex-col gap-5">
        <header>
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h2>
            <p class="m-0 mt-1 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
        </header>

        <p v-if="loading" class="m-0 text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.loading) }}
        </p>
        <Admonition
            v-else-if="!eligible"
            type="warning"
            :header="formatMessage(messages.unavailable)"
        >
            {{ unavailableReason }}
        </Admonition>
        <template v-else-if="config">
            <div v-if="!config.enabled || editingExclusions" class="flex flex-col gap-4">
                <p class="m-0 text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.choose) }}
                </p>
                <BackupExclusionSelector
                    v-model="excludedPaths"
                    :instance-id="instance.id"
                    :instance-root="instanceRoot"
                    :disabled="action !== null"
                />
                <div class="flex flex-wrap gap-2">
                    <Button
                        :disabled="action !== null"
                        @click="config.enabled ? saveExclusions() : enable()"
                        ><SaveIcon />
                        {{
                            formatMessage(
                                config.enabled ? messages.saveExclusions : messages.enable,
                            )
                        }}
                    </Button>
                    <Button
                        v-if="config.enabled"
                        type="outlined"
                        :disabled="action !== null"
                        @click="editingExclusions = false"
                        ><XIcon />
                        {{ formatMessage(commonMessages.cancelButton) }}
                    </Button>
                </div>
            </div>

            <template v-else>
                <div
                    class="flex flex-col gap-2 rounded-lg border border-solid border-surface-4 p-3"
                >
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{
                            config.excluded_paths.length === 0
                                ? formatMessage(messages.allIncluded)
                                : formatMessage(messages.excludedPaths, {
                                      paths: config.excluded_paths
                                          .map((entry) => entry.path)
                                          .join(', '),
                                  })
                        }}
                    </p>
                    <div class="flex flex-wrap gap-2">
                        <Button :disabled="isRunning || action !== null" @click="start"
                            ><FileArchiveIcon />
                            {{ formatMessage(messages.backupNow) }}
                        </Button>
                        <Button
                            v-if="canCancel"
                            type="outlined"
                            :disabled="action !== null"
                            @click="cancel"
                            ><XIcon />
                            {{ formatMessage(messages.cancelBackup) }}
                        </Button>
                        <Button
                            type="outlined"
                            :disabled="isRunning || action !== null"
                            @click="beginEditingExclusions"
                            >{{ formatMessage(messages.editExclusions) }}
                        </Button>
                    </div>
                    <p
                        v-if="isRunning"
                        class="m-0 text-sm font-medium text-[var(--color-text-primary)]"
                    >
                        {{ operationLabel }}
                        <span
                            v-if="(operation?.total_bytes ?? 0) > 0"
                            class="ml-2 text-[var(--color-text-tertiary)]"
                        >
                            {{
                                formatMessage(messages.progressBytes, {
                                    processed: formatBytes(operation?.processed_bytes ?? 0),
                                    total: formatBytes(operation?.total_bytes ?? 0),
                                })
                            }}
                        </span>
                    </p>
                    <p
                        v-else-if="
                            operation?.state === 'failed' || operation?.state === 'interrupted'
                        "
                        class="m-0 text-sm text-red"
                    >
                        {{
                            formatMessage(
                                operation.state === 'interrupted'
                                    ? messages.stageInterrupted
                                    : messages.stageFailed,
                            )
                        }}<span v-if="operation.error">: {{ operation.error }}</span>
                    </p>
                </div>

                <section class="flex flex-col gap-3">
                    <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.snapshots) }}
                    </h3>
                    <p v-if="snapshots.length === 0" class="m-0 text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.empty) }}
                    </p>
                    <div
                        v-else
                        class="divide-y divide-solid divide-surface-4 rounded-lg border border-solid border-surface-4"
                    >
                        <div
                            v-for="snapshot in snapshots"
                            :key="snapshot.id"
                            class="flex items-center gap-3 px-3 py-3"
                        >
                            <FileArchiveIcon
                                class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                            />
                            <div class="min-w-0 flex-1">
                                <p class="m-0 font-semibold text-[var(--color-text-primary)]">
                                    {{ formatDate(new Date(snapshot.created_at)) }}
                                </p>
                                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                                    {{
                                        formatMessage(messages.snapshotMeta, {
                                            files: snapshot.file_count,
                                            size: formatBytes(snapshot.logical_size),
                                        })
                                    }}
                                    ·
                                    {{
                                        formatMessage(messages.addedSize, {
                                            size: formatBytes(snapshot.added_size),
                                        })
                                    }}
                                </p>
                            </div>
                            <Button
                                type="quiet"
                                size="2xs"
                                circular
                                icon-only
                                :disabled="action !== null || isRunning"
                                :aria-label="formatMessage(messages.restore)"
                                @click="confirmRestore(snapshot)"
                                ><UndoIcon />
                            </Button>
                            <Button
                                type="quiet"
                                color="red"
                                size="2xs"
                                circular
                                icon-only
                                :disabled="action !== null || isRunning"
                                :aria-label="formatMessage(messages.delete)"
                                @click="confirmDelete(snapshot)"
                                ><TrashIcon />
                            </Button>
                        </div>
                    </div>
                </section>

                <div class="flex flex-col items-start gap-2">
                    <Button
                        type="outlined"
                        color="red"
                        :disabled="snapshots.length > 0 || action !== null || isRunning"
                        @click="disable"
                        >{{ formatMessage(messages.disable) }}
                    </Button>
                    <p
                        v-if="snapshots.length > 0"
                        class="m-0 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.disableBlocked) }}
                    </p>
                </div>
            </template>
        </template>

        <PreviewModal
            ref="deleteModal"
            :header="formatMessage(messages.deleteTitle)"
            fade="danger"
            max-width="500px"
        >
            <Admonition type="critical">{{ formatMessage(messages.deleteBody) }}</Admonition>
            <template #actions>
                <div class="flex items-center justify-end gap-2">
                    <Button
                        type="outlined"
                        :disabled="action?.startsWith('delete:')"
                        @click="deleteModal?.hide()"
                        >{{ formatMessage(commonMessages.cancelButton) }}
                    </Button>
                    <Button
                        type="colored"
                        color="red"
                        :disabled="action?.startsWith('delete:')"
                        @click="removeSnapshot"
                        ><TrashIcon />{{ formatMessage(messages.delete) }}
                    </Button>
                </div>
            </template>
        </PreviewModal>
        <PreviewModal
            ref="restoreModal"
            :header="formatMessage(messages.restoreTitle)"
            max-width="500px"
        >
            <Admonition v-if="restorePreview" type="warning">
                {{
                    formatMessage(messages.restoreBody, {
                        added: restorePreview.added_files,
                        modified: restorePreview.modified_files,
                        deleted: restorePreview.deleted_files,
                    })
                }}
            </Admonition>
            <template #actions>
                <div class="flex items-center justify-end gap-2">
                    <Button
                        type="outlined"
                        :disabled="action?.startsWith('restore:')"
                        @click="restoreModal?.hide()"
                        >{{ formatMessage(commonMessages.cancelButton) }}
                    </Button>
                    <Button :disabled="action?.startsWith('restore:')" @click="restoreSnapshot"
                        ><UndoIcon />{{ formatMessage(messages.restore) }}
                    </Button>
                </div>
            </template>
        </PreviewModal>
    </div>
</template>
