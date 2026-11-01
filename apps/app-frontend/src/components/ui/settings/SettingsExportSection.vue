<script setup lang="ts">
import { DownloadIcon, EraserIcon, FolderOpenIcon, UploadIcon } from '@modrinth/assets'
import {
    Button,
    commonMessages,
    ConfirmModal,
    defineMessages,
    injectNotificationManager,
    injectPopupNotificationManager,
    NewModal,
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
    scanSettingsImport,
    type ImportDanger,
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
const dangerConfirm = ref<InstanceType<typeof NewModal>>()
const pendingImport = ref<{ path: string; dangerous: string[] } | null>(null)

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
            'Only keys that differ from the defaults. The archive holds one document per domain; unzip it and use it directly as a deployment config.',
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
        defaultMessage: 'The settings archive was saved to {path}',
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
        defaultMessage: 'Settings archive',
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
            'Merges the archive into your local settings. Where both set the same key, the imported value wins. This cannot be undone.',
    },
    importConfirmProceed: {
        id: 'app.settings.export.import-confirm-proceed',
        defaultMessage: 'Import',
    },
    importDangerTitle: {
        id: 'app.settings.export.import-danger-title',
        defaultMessage: 'Dangerous settings in this archive',
    },
    importDangerDescription: {
        id: 'app.settings.export.import-danger-description',
        defaultMessage:
            'The archive sets settings that can make the launcher run commands or change the game process environment.',
    },
    importDangerKeysLabel: {
        id: 'app.settings.export.import-danger-keys-label',
        defaultMessage: 'It sets:',
    },
    importDangerSkip: {
        id: 'app.settings.export.import-danger-skip',
        defaultMessage: 'Import everything else',
    },
    importDangerAllow: {
        id: 'app.settings.export.import-danger-allow',
        defaultMessage: 'Import everything',
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
    importedDroppedText: {
        id: 'app.settings.export.imported-dropped-text',
        defaultMessage:
            '{count, plural, =0 {No local settings document changed.} one {Updated # local settings document.} other {Updated # local settings documents.}} {dropped, plural, one {# value was ignored because its type is invalid.} other {# values were ignored because their types are invalid.}}',
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
            defaultPath: `${EXPORT_FILE_NAMES[kind]} ${timestamp}.zip`,
            filters: [{ name: 'ZIP', extensions: ['zip'] }],
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

async function chooseImportArchive() {
    if (busy.value) return

    pendingImport.value = null
    busy.value = 'import'
    try {
        const inputPath = await open({
            filters: [{ name: 'ZIP', extensions: ['zip'] }],
            multiple: false,
        })
        if (!inputPath) return

        const scan = await scanSettingsImport(inputPath)
        pendingImport.value = { path: inputPath, dangerous: scan.dangerous }
        if (scan.dangerous.length > 0) {
            dangerConfirm.value?.show()
        } else {
            importConfirm.value?.show()
        }
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = null
    }
}

function cancelPendingImport() {
    pendingImport.value = null
    dangerConfirm.value?.hide()
}

async function importPendingArchive(danger: ImportDanger) {
    const pending = pendingImport.value
    if (!pending || busy.value) return

    dangerConfirm.value?.hide()
    importConfirm.value?.hide()
    pendingImport.value = null
    busy.value = 'import'
    try {
        const result = await importSettings(pending.path, danger)
        popupNotificationManager.addPopupNotification({
            title: formatMessage(messages.importedTitle),
            text: formatMessage(
                result.dropped > 0 ? messages.importedDroppedText : messages.importedText,
                { count: result.documents, dropped: result.dropped },
            ),
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
                    @click="chooseImportArchive"
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
        @proceed="importPendingArchive('allow')"
    />
    <NewModal
        ref="dangerConfirm"
        :header="formatMessage(messages.importDangerTitle)"
        fade="danger"
        max-width="520px"
        :closable="false"
    >
        <p class="m-0">{{ formatMessage(messages.importDangerDescription) }}</p>
        <p class="mb-1 mt-4 text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.importDangerKeysLabel) }}
        </p>
        <ul class="m-0 flex list-disc flex-col gap-1 pl-5 font-mono text-sm">
            <li v-for="dangerousKey in pendingImport?.dangerous ?? []" :key="dangerousKey">
                {{ dangerousKey }}
            </li>
        </ul>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button type="colored" color="brand" @click="importPendingArchive('skip')">
                    {{ formatMessage(messages.importDangerSkip) }}
                </Button>
                <Button @click="cancelPendingImport">
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="outlined" color="red" @click="importPendingArchive('allow')">
                    {{ formatMessage(messages.importDangerAllow) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
