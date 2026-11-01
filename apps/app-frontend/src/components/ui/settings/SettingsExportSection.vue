<script setup lang="ts">
import { DownloadIcon, FolderOpenIcon } from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    injectPopupNotificationManager,
    useVIntl,
} from '@modrinth/ui'
import { save } from '@tauri-apps/plugin-dialog'
import { ref } from 'vue'

import { exportSettings, type SettingsExportKind } from '@/helpers/settings'
import { highlightInFolder } from '@/helpers/utils'

import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const popupNotificationManager = injectPopupNotificationManager()

const EXPORT_FILE_NAMES: Record<SettingsExportKind, string> = {
    effective: 'Axolotl Launcher settings',
    patch: 'Axolotl Launcher settings patch',
}

const exporting = ref<SettingsExportKind | null>(null)

const messages = defineMessages({
    title: { id: 'app.settings.export.title', defaultMessage: 'Export settings' },
    allLabel: { id: 'app.settings.export.all-label', defaultMessage: 'All settings' },
    allButton: {
        id: 'app.settings.export.all-button',
        defaultMessage: 'Export all settings…',
    },
    patchLabel: {
        id: 'app.settings.export.patch-label',
        defaultMessage: 'Deployment patch',
    },
    patchDescription: {
        id: 'app.settings.export.patch-description',
        defaultMessage:
            'Only keys that differ from the defaults, ready to use directly as a deployment config.',
    },
    patchButton: {
        id: 'app.settings.export.patch-button',
        defaultMessage: 'Export patch…',
    },
    exportedTitle: {
        id: 'app.settings.export.exported-title',
        defaultMessage: 'Settings exported',
    },
    exportedText: {
        id: 'app.settings.export.exported-text',
        defaultMessage: 'The settings file was saved to {path}',
    },
})

async function exportToFile(kind: SettingsExportKind) {
    if (exporting.value) return

    exporting.value = kind
    try {
        const timestamp = new Date().toISOString().replace(/[:.]/g, '-')
        const outputPath = await save({
            defaultPath: `${EXPORT_FILE_NAMES[kind]} ${timestamp}.json`,
            filters: [{ name: 'JSON', extensions: ['json'] }],
        })
        if (!outputPath) return

        await exportSettings(outputPath, kind)
        popupNotificationManager.addPopupNotification({
            title: formatMessage(messages.exportedTitle),
            text: formatMessage(messages.exportedText, { path: outputPath }),
            type: 'success',
            buttons: [
                {
                    label: formatMessage(commonMessages.openInFolderButton),
                    icon: FolderOpenIcon,
                    action: () => highlightInFolder(outputPath).catch(handleError),
                },
            ],
        })
    } catch (error) {
        handleError(error)
    } finally {
        exporting.value = null
    }
}
</script>

<template>
    <SettingsSection
        :title="formatMessage(messages.title)"
        title-id="settings-target-settings-export"
    >
        <SettingsRow>
            <template #label>{{ formatMessage(messages.allLabel) }}</template>
            <template #control>
                <Button
                    type="colored"
                    color="brand"
                    :loading="exporting === 'effective'"
                    :disabled="exporting !== null"
                    @click="exportToFile('effective')"
                >
                    <DownloadIcon aria-hidden="true" />
                    {{ formatMessage(messages.allButton) }}
                </Button>
            </template>
        </SettingsRow>
        <SettingsRow>
            <template #label>{{ formatMessage(messages.patchLabel) }}</template>
            <template #description>{{ formatMessage(messages.patchDescription) }}</template>
            <template #control>
                <Button
                    :loading="exporting === 'patch'"
                    :disabled="exporting !== null"
                    @click="exportToFile('patch')"
                >
                    <DownloadIcon aria-hidden="true" />
                    {{ formatMessage(messages.patchButton) }}
                </Button>
            </template>
        </SettingsRow>
    </SettingsSection>
</template>
