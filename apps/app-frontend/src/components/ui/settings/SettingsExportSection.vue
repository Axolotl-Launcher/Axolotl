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
    type DangerousImport,
    exportSettings,
    get as getSettings,
    type ImportDanger,
    importSettings,
    sanitiseSettings,
    scanSettingsImport,
    type SettingsExportKind,
} from '@/helpers/settings'
import { highlightInFolder } from '@/helpers/utils'

import { reloadSettingsLayers } from './pinned-override'
import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const { formatMessage, locale } = useVIntl()
const { handleError } = injectNotificationManager()
const popupNotificationManager = injectPopupNotificationManager()
const queryClient = useQueryClient()

/** Lets the mounted settings sections of this category read the stored values again. */
const emit = defineEmits<{ changed: [] }>()

const EXPORT_FILE_NAMES: Record<SettingsExportKind, string> = {
    effective: 'Axolotl Launcher settings',
    patch: 'Axolotl Launcher settings patch',
}

type SettingsOperation = SettingsExportKind | 'cleanup' | 'import'

const busy = ref<SettingsOperation | null>(null)
const cleanupConfirm = ref<InstanceType<typeof ConfirmModal>>()
const importConfirm = ref<InstanceType<typeof ConfirmModal>>()
const dangerConfirm = ref<InstanceType<typeof NewModal>>()
const pendingImport = ref<{ path: string } | null>(null)
/** What the open confirmation shows, kept while it fades out. */
const dangerRows = ref<DangerousImport[]>([])

const messages = defineMessages({
    title: { id: 'app.settings.export.title', defaultMessage: 'Manage settings files' },
    allLabel: {
        id: 'app.settings.export.all-label',
        defaultMessage: 'All settings files',
    },
    allButton: {
        id: 'app.settings.export.all-button',
        defaultMessage: 'Export all settings files…',
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
        defaultMessage: 'Local settings files',
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
        defaultMessage: 'Clean up local settings files?',
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
        defaultMessage: 'The settings you are about to import may be dangerous',
    },
    importDangerDescription: {
        id: 'app.settings.export.import-danger-description',
        defaultMessage: 'The settings you are about to import will change {keys}.',
    },
    importDangerKeyColumn: {
        id: 'app.settings.export.import-danger-key-column',
        defaultMessage: 'Dangerous setting',
    },
    importDangerBeforeColumn: {
        id: 'app.settings.export.import-danger-before-column',
        defaultMessage: 'Before import',
    },
    importDangerAfterColumn: {
        id: 'app.settings.export.import-danger-after-column',
        defaultMessage: 'After import',
    },
    importDangerNote: {
        id: 'app.settings.export.import-danger-note',
        defaultMessage:
            'They change the arguments the game is started with or the commands that run before it, and may also change the environment of the game process, for example by injecting environment variables. If you do not trust where the file came from, or do not understand what they mean and do, you should cancel the import or skip these dangerous settings. Please continue only if you fully understand and trust them.',
    },
    importDangerCancel: {
        id: 'app.settings.export.import-danger-cancel',
        defaultMessage: 'Cancel import',
    },
    importDangerSkip: {
        id: 'app.settings.export.import-danger-skip',
        defaultMessage: 'Skip dangerous settings',
    },
    importDangerAllow: {
        id: 'app.settings.export.import-danger-allow',
        defaultMessage: 'Continue import',
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
const dangerousKeyList = computed(() =>
    new Intl.ListFormat(locale.value, { style: 'short', type: 'unit' }).format(
        dangerRows.value.map((entry) => entry.key),
    ),
)

/** One value as the compact JSON the import would write. */
function settingValue(value: unknown): string {
    return value === null || value === undefined ? '—' : JSON.stringify(value)
}

/** Both operations rewrite the documents behind the running app. */
async function refreshSettings() {
    queryClient.setQueryData(appSettingsKeys.all, await getSettings())
    await reloadSettingsLayers(handleError)
}

/**
 * The writes can land even when the call reports an error, so the refresh runs
 * either way, then the mounted sections read the stored values again and the
 * failure is reported against what the interface now shows.
 */
async function runSettingsFileOperation(operation: SettingsOperation, write: () => Promise<void>) {
    busy.value = operation
    let failure: unknown
    try {
        await write()
    } catch (error) {
        failure = error
    }
    try {
        await refreshSettings()
    } catch (error) {
        failure ??= error
    }
    busy.value = null
    emit('changed')
    if (failure) handleError(failure)
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

    await runSettingsFileOperation('cleanup', async () => {
        const rewritten = await sanitiseSettings()
        popupNotificationManager.addPopupNotification({
            title: formatMessage(messages.cleanupDoneTitle),
            text: formatMessage(messages.cleanupDoneText, { count: rewritten }),
            type: 'success',
        })
    })
}

/** Scans the archive afresh and shows the modal for what it holds at this moment. */
async function reviewImport(inputPath: string) {
    const scan = await scanSettingsImport(inputPath)
    pendingImport.value = { path: inputPath }
    dangerRows.value = scan.dangerous
    if (scan.dangerous.length > 0) {
        dangerConfirm.value?.show()
    } else {
        importConfirm.value?.show()
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

        await reviewImport(inputPath)
    } catch (error) {
        pendingImport.value = null
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
    await runSettingsFileOperation('import', async () => {
        const result = await importSettings(pending.path, danger)
        popupNotificationManager.addPopupNotification({
            title: formatMessage(messages.importedTitle),
            text: formatMessage(
                result.dropped > 0 ? messages.importedDroppedText : messages.importedText,
                { count: result.documents, dropped: result.dropped },
            ),
            type: 'success',
        })
    })
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
        max-width="640px"
        :closable="false"
    >
        <p class="m-0">
            {{ formatMessage(messages.importDangerDescription, { keys: dangerousKeyList }) }}
        </p>
        <table class="mt-4 w-full table-fixed border-collapse text-sm">
            <thead>
                <tr>
                    <th
                        scope="col"
                        class="w-[22%] border-0 border-b border-solid border-surface-4 pb-2 pr-3 text-left font-semibold text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.importDangerKeyColumn) }}
                    </th>
                    <th
                        scope="col"
                        class="w-[39%] border-0 border-b border-solid border-surface-4 pb-2 pr-3 text-left font-semibold text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.importDangerBeforeColumn) }}
                    </th>
                    <th
                        scope="col"
                        class="w-[39%] border-0 border-b border-solid border-surface-4 pb-2 text-left font-semibold text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.importDangerAfterColumn) }}
                    </th>
                </tr>
            </thead>
            <tbody>
                <tr v-for="entry in dangerRows" :key="entry.key">
                    <th
                        scope="row"
                        class="border-0 border-t border-solid border-surface-4 py-2 pr-3 text-left align-top font-normal text-[var(--color-text-primary)]"
                    >
                        <code class="break-all font-mono text-xs">{{ entry.key }}</code>
                    </th>
                    <td class="border-0 border-t border-solid border-surface-4 py-2 pr-3 align-top">
                        <span
                            class="line-clamp-4 break-all font-mono text-xs text-[var(--color-text-default)]"
                            :title="settingValue(entry.before)"
                        >
                            {{ settingValue(entry.before) }}
                        </span>
                    </td>
                    <td class="border-0 border-t border-solid border-surface-4 py-2 align-top">
                        <span
                            class="line-clamp-4 break-all font-mono text-xs text-[var(--color-text-default)]"
                            :title="settingValue(entry.after)"
                        >
                            {{ settingValue(entry.after) }}
                        </span>
                    </td>
                </tr>
            </tbody>
        </table>
        <p class="m-0 mt-4 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.importDangerNote) }}
        </p>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button @click="cancelPendingImport">
                    {{ formatMessage(messages.importDangerCancel) }}
                </Button>
                <Button @click="importPendingArchive('skip')">
                    {{ formatMessage(messages.importDangerSkip) }}
                </Button>
                <Button type="colored" color="red" @click="importPendingArchive('allow')">
                    {{ formatMessage(messages.importDangerAllow) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
