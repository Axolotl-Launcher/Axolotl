<script setup lang="ts">
import { DownloadIcon, EraserIcon, FolderOpenIcon, UploadIcon } from '@modrinth/assets'
import {
    Button,
    commonMessages,
    ConfirmModal,
    defineMessages,
    injectNotificationManager,
    injectPopupNotificationManager,
    useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { open, save } from '@tauri-apps/plugin-dialog'
import { computed, ref } from 'vue'

import {
    appSettingsKeys,
    exportSettings,
    get as getSettings,
    importSettings,
    sanitiseSettings,
    type SettingsExportKind,
} from '@/helpers/settings'
import { highlightInFolder } from '@/helpers/utils'

import { reloadSettingsLayers } from './pinned-override'
import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const popupNotificationManager = injectPopupNotificationManager()
const queryClient = useQueryClient()

const EXPORT_FILE_NAMES: Record<SettingsExportKind, string> = {
    effective: 'Axolotl Launcher settings',
    patch: 'Axolotl Launcher settings patch',
}

type SettingsOperation = SettingsExportKind | 'cleanup' | 'import'

const busy = ref<SettingsOperation | null>(null)
const cleanupConfirm = ref<InstanceType<typeof ConfirmModal>>()
const importConfirm = ref<InstanceType<typeof ConfirmModal>>()

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
    cleanupLabel: {
        id: 'app.settings.export.cleanup-label',
        defaultMessage: 'Local settings',
    },
    cleanupButton: {
        id: 'app.settings.export.cleanup-button',
        defaultMessage: 'Clean up…',
    },
    cleanupRunning: {
        id: 'app.settings.export.cleanup-running',
        defaultMessage: 'Cleaning up…',
    },
    cleanupConfirmTitle: {
        id: 'app.settings.export.cleanup-confirm-title',
        defaultMessage: 'Clean up local settings?',
    },
    cleanupConfirmDescription: {
        id: 'app.settings.export.cleanup-confirm-description',
        defaultMessage:
            'Rewrites every local settings document, discarding unknown and redundant keys. This cannot be undone.',
    },
    cleanupConfirmProceed: {
        id: 'app.settings.export.cleanup-confirm-proceed',
        defaultMessage: 'Clean up',
    },
    cleanupDoneTitle: {
        id: 'app.settings.export.cleanup-done-title',
        defaultMessage: 'Local settings cleaned up',
    },
    cleanupDoneText: {
        id: 'app.settings.export.cleanup-done-text',
        defaultMessage:
            '{count, plural, =0 {No local settings document needed rewriting.} one {Rewrote # local settings document.} other {Rewrote # local settings documents.}}',
    },
    importLabel: {
        id: 'app.settings.export.import-label',
        defaultMessage: 'Settings file',
    },
    importButton: {
        id: 'app.settings.export.import-button',
        defaultMessage: 'Import…',
    },
    importRunning: {
        id: 'app.settings.export.import-running',
        defaultMessage: 'Importing…',
    },
    importConfirmTitle: {
        id: 'app.settings.export.import-confirm-title',
        defaultMessage: 'Import settings?',
    },
    importConfirmDescription: {
        id: 'app.settings.export.import-confirm-description',
        defaultMessage:
            'Merges the file into your local settings. Where both set the same key, the imported value wins. This cannot be undone.',
    },
    importConfirmProceed: {
        id: 'app.settings.export.import-confirm-proceed',
        defaultMessage: 'Import',
    },
    importedTitle: {
        id: 'app.settings.export.imported-title',
        defaultMessage: 'Settings imported',
    },
    importedText: {
        id: 'app.settings.export.imported-text',
        defaultMessage:
            '{count, plural, =0 {No local settings document changed.} one {Updated # local settings document.} other {Updated # local settings documents.}}',
    },
})

const cleanupButtonMessage = computed(() =>
    busy.value === 'cleanup' ? messages.cleanupRunning : messages.cleanupButton,
)
const importButtonMessage = computed(() =>
    busy.value === 'import' ? messages.importRunning : messages.importButton,
)

/** Both operations rewrite the documents behind the running app. */
async function refreshSettings() {
    queryClient.setQueryData(appSettingsKeys.all, await getSettings())
    await reloadSettingsLayers(handleError)
}

async function exportToFile(kind: SettingsExportKind) {
    if (busy.value) return

    busy.value = kind
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
        busy.value = null
    }
}

async function cleanUpLocalSettings() {
    if (busy.value) return

    busy.value = 'cleanup'
    try {
        const rewritten = await sanitiseSettings()
        popupNotificationManager.addPopupNotification({
            title: formatMessage(messages.cleanupDoneTitle),
            text: formatMessage(messages.cleanupDoneText, { count: rewritten }),
            type: 'success',
        })
        await refreshSettings()
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = null
    }
}

async function importFromFile() {
    if (busy.value) return

    busy.value = 'import'
    try {
        const inputPath = await open({
            filters: [{ name: 'JSON', extensions: ['json'] }],
            multiple: false,
        })
        if (!inputPath) return

        const rewritten = await importSettings(inputPath)
        popupNotificationManager.addPopupNotification({
            title: formatMessage(messages.importedTitle),
            text: formatMessage(messages.importedText, { count: rewritten }),
            type: 'success',
        })
        await refreshSettings()
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = null
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
                    :loading="busy === 'effective'"
                    :disabled="busy !== null"
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
                    :loading="busy === 'patch'"
                    :disabled="busy !== null"
                    @click="exportToFile('patch')"
                >
                    <DownloadIcon aria-hidden="true" />
                    {{ formatMessage(messages.patchButton) }}
                </Button>
            </template>
        </SettingsRow>
        <SettingsRow>
            <template #label>{{ formatMessage(messages.cleanupLabel) }}</template>
            <template #control>
                <Button
                    type="outlined"
                    color="red"
                    :loading="busy === 'cleanup'"
                    :disabled="busy !== null"
                    @click="cleanupConfirm?.show()"
                >
                    <EraserIcon aria-hidden="true" />
                    {{ formatMessage(cleanupButtonMessage) }}
                </Button>
            </template>
        </SettingsRow>
        <SettingsRow>
            <template #label>{{ formatMessage(messages.importLabel) }}</template>
            <template #control>
                <Button
                    type="outlined"
                    color="red"
                    :loading="busy === 'import'"
                    :disabled="busy !== null"
                    @click="importConfirm?.show()"
                >
                    <UploadIcon aria-hidden="true" />
                    {{ formatMessage(importButtonMessage) }}
                </Button>
            </template>
        </SettingsRow>
    </SettingsSection>

    <ConfirmModal
        ref="cleanupConfirm"
        :danger="true"
        :markdown="false"
        :title="formatMessage(messages.cleanupConfirmTitle)"
        :description="formatMessage(messages.cleanupConfirmDescription)"
        :proceed-icon="EraserIcon"
        :proceed-label="formatMessage(messages.cleanupConfirmProceed)"
        @proceed="cleanUpLocalSettings"
    />
    <ConfirmModal
        ref="importConfirm"
        :danger="true"
        :markdown="false"
        :title="formatMessage(messages.importConfirmTitle)"
        :description="formatMessage(messages.importConfirmDescription)"
        :proceed-icon="UploadIcon"
        :proceed-label="formatMessage(messages.importConfirmProceed)"
        @proceed="importFromFile"
    />
</template>
