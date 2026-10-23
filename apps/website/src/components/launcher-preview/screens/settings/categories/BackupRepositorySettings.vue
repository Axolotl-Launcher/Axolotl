<script setup lang="ts">
import FolderIcon from '@modrinth/assets/icons/folder.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { onMounted, onUnmounted, ref } from 'vue'

import FolderPanel from '../../create/components/FolderPanel.vue'
import SettingsRow from '../components/SettingsRow.vue'
import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { useStorageBackend } from './storage/storageBackend'
import type { BackupRepositoryStatus } from './storage/storageData'

/**
 * Port of the app's BackupRepositorySettings (apps/app-frontend/src/components/
 * ui/settings/BackupRepositorySettings.vue). The native folder dialog is the
 * preview's macOS open panel; the move runs on the simulated launcher disk.
 */
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const { handleError } = injectNotificationManager()
const {
    getBackupRepositoryStatus,
    listBackupOperations,
    listenBackupProgress,
    moveBackupRepository,
} = useStorageBackend()
const folderPanel = ref<InstanceType<typeof FolderPanel> | null>(null)
const status = ref<BackupRepositoryStatus | null>(null)
const loading = ref(true)
const moving = ref(false)
const movedBytes = ref(0)
const moveTotalBytes = ref(0)
const movingOperationId = ref<string | null>(null)
let unlisten: (() => void) | null = null

async function refresh() {
    loading.value = true
    try {
        status.value = await getBackupRepositoryStatus()
    } catch (error) {
        handleError(error as Error)
    } finally {
        loading.value = false
    }
}

async function changeLocation() {
    const selection = await folderPanel.value?.pick()
    if (!selection) return
    moving.value = true
    movedBytes.value = 0
    moveTotalBytes.value = 0
    try {
        movingOperationId.value = await moveBackupRepository(selection)
    } catch (error) {
        handleError(error as Error)
        moving.value = false
    }
}

onMounted(async () => {
    unlisten = await listenBackupProgress((event) => {
        if (event.operationType !== 'repository_move') return
        if (movingOperationId.value && event.operationId !== movingOperationId.value) return
        movingOperationId.value = event.operationId
        moving.value = !event.finalState
        movedBytes.value = event.processedBytes
        moveTotalBytes.value = event.totalBytes
        if (event.finalState) void refresh()
    })
    const activeMove = (await listBackupOperations(undefined, true)).find(
        (operation) => operation.operation_type === 'repository_move',
    )
    if (activeMove) {
        movingOperationId.value = activeMove.id
        moving.value = true
        movedBytes.value = activeMove.processed_bytes
        moveTotalBytes.value = activeMove.total_bytes
    }
    await refresh()
})

onUnmounted(() => unlisten?.())
</script>

<template>
    <SettingsSection
        :title="formatMessage(messages.settingsBackupsRepositoryTitle)"
        :description="formatMessage(messages.settingsBackupsRepositoryDescription)"
    >
        <SettingsRow>
            <template #label>{{
                formatMessage(messages.settingsBackupsRepositoryLocation)
            }}</template>
            <template #description>
                <span class="break-all">{{ status?.path ?? '...' }}</span>
                <span class="mt-1 block">{{
                    formatMessage(messages.settingsBackupsRepositoryLocationDescription)
                }}</span>
            </template>
            <template #control>
                <Button type="outlined" :disabled="loading || moving" @click="changeLocation"
                    ><FolderIcon />
                    {{
                        formatMessage(
                            moving && moveTotalBytes > 0
                                ? messages.settingsBackupsRepositoryMovingProgress
                                : moving
                                  ? messages.settingsBackupsRepositoryMoving
                                  : messages.settingsBackupsRepositoryChange,
                            {
                                processed: formatBytes(movedBytes),
                                total: formatBytes(moveTotalBytes),
                            },
                        )
                    }}
                </Button>
            </template>
        </SettingsRow>
        <SettingsRow v-if="status">
            <template #label>{{ formatMessage(messages.settingsBackupsRepositoryUsage) }}</template>
            <template #description>
                {{
                    formatMessage(messages.settingsBackupsRepositoryUsageDescription, {
                        stored: formatBytes(status.stored_size),
                        logical: formatBytes(status.logical_size),
                        count: status.snapshot_count,
                    })
                }}
            </template>
        </SettingsRow>
        <Admonition
            v-if="status && !status.available"
            type="critical"
            :header="formatMessage(messages.settingsBackupsRepositoryUnavailable)"
        >
            {{ status.error }}
        </Admonition>
    </SettingsSection>
    <FolderPanel ref="folderPanel" />
</template>
