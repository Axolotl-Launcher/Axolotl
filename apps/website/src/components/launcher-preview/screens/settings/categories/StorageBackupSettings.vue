<script setup lang="ts">
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

import SettingsSection from '../components/SettingsSection.vue'
import BackupRepositorySettings from './BackupRepositorySettings.vue'
import ResourceManagementSettings from './ResourceManagementSettings.vue'
import { storageMessages } from './storage/storageMessages'
import StorageSettings from './StorageSettings.vue'

/** Port of the app's StorageBackupSettings category. */
const { formatMessage } = useVIntl()
</script>

<template>
    <div class="flex flex-col gap-6">
        <ResourceManagementSettings scope="storage-backups" />
        <BackupRepositorySettings />
        <SettingsSection>
            <template #header>{{ formatMessage(storageMessages.analysisTitle) }}</template>
            <div class="p-4"><StorageSettings /></div>
        </SettingsSection>
    </div>
</template>
