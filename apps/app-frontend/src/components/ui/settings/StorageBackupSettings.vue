<script setup lang="ts">
import { useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import BackupRepositorySettings from './BackupRepositorySettings.vue'
import ResourceManagementSettings from './ResourceManagementSettings.vue'
import SettingsExportSection from './SettingsExportSection.vue'
import SettingsSection from './SettingsSection.vue'
import { storageMessages } from './storage/storageMessages'
import StorageSettings from './StorageSettings.vue'

const { formatMessage } = useVIntl()

const resourceSettings = ref<InstanceType<typeof ResourceManagementSettings>>()

function reloadSettings() {
    void resourceSettings.value?.reloadSettings()
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <ResourceManagementSettings ref="resourceSettings" scope="storage-backups" />
        <BackupRepositorySettings />
        <SettingsSection>
            <template #header>{{ formatMessage(storageMessages.analysisTitle) }}</template>
            <div class="p-4"><StorageSettings /></div>
        </SettingsSection>
        <SettingsExportSection @changed="reloadSettings" />
    </div>
</template>
