<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectPopupNotificationManager } from '@modrinth/ui/src/providers/popup-notifications.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, markRaw, ref } from 'vue'

import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { messages } from '../../messages'
import { useSettingsPage } from '../../state'
import { useFinder } from './finder'
import {
    defaultExportFileName,
    exportArchiveBytes,
    type LogExportLevel,
    type LogExportOptions,
    type LogExportRange,
} from './launcher-logs'
import MacSavePanel from './MacSavePanel.vue'

/**
 * Log export options (apps/app-frontend/src/components/ui/settings/LogExportModal.vue).
 * `exportLauncherLogs()` asks for the destination in the system save panel
 * (the app's proposed name, ZIP filter), then `export_launcher_logs` filters,
 * censors and zips the session logs into it; the archive then exists in that
 * folder for the Finder to reveal.
 */
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const popupNotificationManager = injectPopupNotificationManager()
const { settings, simulate } = useSettingsPage()
const finder = useFinder()

const modal = ref<InstanceType<typeof PreviewModal>>()
const savePanel = ref<InstanceType<typeof MacSavePanel>>()
const exporting = ref(false)
const selectedRange = ref<LogExportRange>('last2Hours')
const selectedLevel = ref<LogExportLevel>('all')
const includeSystemInfo = ref(true)
const includeInstanceLogs = ref(false)
const includeCrashAnalysis = ref(false)
const includeNetworkConnections = ref(false)

const rangeOptions = computed(() => [
    {
        value: 'last30Minutes',
        label: formatMessage(messages.appSettingsLogsExportModalRange30Minutes),
    },
    { value: 'last2Hours', label: formatMessage(messages.appSettingsLogsExportModalRange2Hours) },
    { value: 'all', label: formatMessage(messages.appSettingsLogsExportModalRangeAll) },
])

const levelOptions = computed(() => [
    { value: 'all', label: formatMessage(messages.appSettingsLogsExportModalLevelAll) },
    { value: 'debug', label: formatMessage(messages.appSettingsLogsExportModalLevelDebug) },
    { value: 'info', label: formatMessage(messages.appSettingsLogsExportModalLevelInfo) },
])

/** `exportLauncherLogs()` of helpers/utils: save panel, then `plugin:utils|export_launcher_logs`. */
async function exportLauncherLogs(options: LogExportOptions) {
    const outputPath = await savePanel.value?.request(defaultExportFileName())
    if (!outputPath) return null

    const bytes = exportArchiveBytes(options, Date.now(), settings.log_level)
    // Reading, filtering and censoring the session logs dominates the time.
    await simulate(280 + Math.min(1400, Math.round(bytes / 110)))
    finder.addFile(outputPath, { kind: 'zip', bytes, modified: Date.now() })
    return outputPath
}

/** `highlightInFolder()`: `plugin:utils|highlight_in_folder` reveals the file in the Finder. */
async function highlightInFolder(path: string) {
    await simulate(120)
    finder.revealItem(path)
}

async function exportLogs() {
    if (exporting.value) return
    exporting.value = true
    try {
        const path = await exportLauncherLogs({
            range: selectedRange.value,
            level: selectedLevel.value,
            includeSystemInfo: includeSystemInfo.value,
            includeInstanceLogs: includeInstanceLogs.value,
            includeCrashAnalysis: includeCrashAnalysis.value,
            includeNetworkConnections: includeNetworkConnections.value,
        })

        if (path) {
            modal.value?.hide()
            popupNotificationManager.addPopupNotification({
                title: formatMessage(messages.appSettingsLogsExportNotificationTitle),
                text: formatMessage(messages.appSettingsLogsExportNotificationText, { path }),
                type: 'success',
                buttons: [
                    {
                        label: formatMessage(commonMessages.openInFolderButton),
                        // The notification store is reactive; the icon must stay a plain component.
                        icon: markRaw(FolderOpenIcon),
                        action: () => highlightInFolder(path).catch(handleError),
                    },
                ],
            })
        }
    } catch (error) {
        handleError(error)
    } finally {
        exporting.value = false
    }
}

defineExpose({
    show: () => modal.value?.show(),
    hide: () => {
        if (!exporting.value) modal.value?.hide()
    },
})
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.appSettingsLogsExportModalTitle)"
        :closable="!exporting"
        :close-on-click-outside="!exporting"
        width="40rem"
        scrollable
    >
        <div class="flex flex-col gap-4">
            <p class="m-0">{{ formatMessage(messages.appSettingsLogsExportModalDescription) }}</p>

            <div class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.appSettingsLogsExportModalRange)
                }}</span>
                <Combobox
                    id="log-export-range"
                    v-model="selectedRange"
                    :disabled="exporting"
                    :name="formatMessage(messages.appSettingsLogsExportModalRange)"
                    :options="rangeOptions"
                />
            </div>

            <div class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.appSettingsLogsExportModalLevel)
                }}</span>
                <Combobox
                    id="log-export-level"
                    v-model="selectedLevel"
                    :disabled="exporting"
                    :name="formatMessage(messages.appSettingsLogsExportModalLevel)"
                    :options="levelOptions"
                />
            </div>

            <div class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.appSettingsLogsExportModalExtras)
                }}</span>
                <Checkbox
                    v-model="includeSystemInfo"
                    :disabled="exporting"
                    :label="formatMessage(messages.appSettingsLogsExportModalSystemInfo)"
                />
                <Checkbox
                    v-model="includeInstanceLogs"
                    :disabled="exporting"
                    :label="formatMessage(messages.appSettingsLogsExportModalInstanceLogs)"
                />
                <Checkbox
                    v-model="includeCrashAnalysis"
                    :disabled="exporting"
                    :label="formatMessage(messages.appSettingsLogsExportModalCrashAnalysis)"
                />
                <Checkbox
                    v-model="includeNetworkConnections"
                    :disabled="exporting"
                    :label="formatMessage(messages.appSettingsLogsExportModalNetworkConnections)"
                />
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appSettingsLogsExportModalNetworkConnectionsHint) }}
                </p>
                <Admonition
                    v-if="includeNetworkConnections"
                    type="info"
                    :body="formatMessage(messages.launcherPreviewSettingsLogsNetworkTests)"
                />
            </div>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" :disabled="exporting" @click="modal?.hide()"
                    ><XIcon />
                    {{ formatMessage(messages.appSettingsLogsExportModalCancel) }}
                </Button>
                <Button type="colored" color="brand" :loading="exporting" @click="exportLogs"
                    ><DownloadIcon />
                    {{
                        formatMessage(
                            exporting
                                ? messages.appSettingsLogsExportModalExporting
                                : messages.appSettingsLogsExportModalConfirm,
                        )
                    }}
                </Button>
            </div>
        </template>
    </PreviewModal>
    <MacSavePanel ref="savePanel" />
</template>
