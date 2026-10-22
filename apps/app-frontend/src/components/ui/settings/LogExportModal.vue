<script setup lang="ts">
import { DownloadIcon, FolderOpenIcon, XIcon } from '@modrinth/assets'
import {
    Button,
    Checkbox,
    Combobox,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    injectPopupNotificationManager,
    NewModal,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import { exportLauncherLogs, highlightInFolder } from '@/helpers/utils'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const popupNotificationManager = injectPopupNotificationManager()

const modal = ref<InstanceType<typeof NewModal>>()
const exporting = ref(false)
const selectedRange = ref<'last30Minutes' | 'last2Hours' | 'all'>('last2Hours')
const selectedLevel = ref<'all' | 'debug' | 'info'>('all')
const includeSystemInfo = ref(true)
const includeInstanceLogs = ref(false)
const includeCrashAnalysis = ref(false)
const includeNetworkConnections = ref(false)

const messages = defineMessages({
    title: { id: 'app.settings.logs.export.modal.title', defaultMessage: 'Export logs' },
    description: {
        id: 'app.settings.logs.export.modal.description',
        defaultMessage:
            'Choose how much detail to include. Credentials are replaced with placeholders. IP addresses are hidden in logs and preserved in network connection reports.',
    },
    rangeLabel: { id: 'app.settings.logs.export.modal.range', defaultMessage: 'Time range' },
    rangeLast30Minutes: {
        id: 'app.settings.logs.export.modal.range.30-minutes',
        defaultMessage: 'Last 30 minutes',
    },
    rangeLast2Hours: {
        id: 'app.settings.logs.export.modal.range.2-hours',
        defaultMessage: 'Last 2 hours',
    },
    rangeAll: { id: 'app.settings.logs.export.modal.range.all', defaultMessage: 'All sessions' },
    levelLabel: { id: 'app.settings.logs.export.modal.level', defaultMessage: 'Log level' },
    levelAll: { id: 'app.settings.logs.export.modal.level.all', defaultMessage: 'All levels' },
    levelDebug: {
        id: 'app.settings.logs.export.modal.level.debug',
        defaultMessage: 'Debug and above',
    },
    levelInfo: {
        id: 'app.settings.logs.export.modal.level.info',
        defaultMessage: 'Info and above',
    },
    extrasLabel: {
        id: 'app.settings.logs.export.modal.extras',
        defaultMessage: 'Additional information',
    },
    systemInfo: {
        id: 'app.settings.logs.export.modal.system-info',
        defaultMessage: 'System and environment summary',
    },
    instanceLogs: {
        id: 'app.settings.logs.export.modal.instance-logs',
        defaultMessage: 'Most recent instance log',
    },
    crashAnalysis: {
        id: 'app.settings.logs.export.modal.crash-analysis',
        defaultMessage: 'Crash analysis (when a crash is detected)',
    },
    networkConnections: {
        id: 'app.settings.logs.export.modal.network-connections',
        defaultMessage: 'Network connections',
    },
    networkConnectionsHint: {
        id: 'app.settings.logs.export.modal.network-connections-hint',
        defaultMessage:
            'Run fresh tests for all services using launcher network settings when exporting, including a short download sample.',
    },
    exporting: {
        id: 'app.settings.logs.export.modal.exporting',
        defaultMessage: 'Exporting…',
    },
    cancel: { id: 'app.settings.logs.export.modal.cancel', defaultMessage: 'Cancel' },
    confirm: { id: 'app.settings.logs.export.modal.confirm', defaultMessage: 'Export' },
    exportedTitle: {
        id: 'app.settings.logs.export.notification-title',
        defaultMessage: 'Logs exported',
    },
    exportedText: {
        id: 'app.settings.logs.export.notification-text',
        defaultMessage: 'The log archive was saved to {path}',
    },
})

const rangeOptions = computed(() => [
    { value: 'last30Minutes', label: formatMessage(messages.rangeLast30Minutes) },
    { value: 'last2Hours', label: formatMessage(messages.rangeLast2Hours) },
    { value: 'all', label: formatMessage(messages.rangeAll) },
])

const levelOptions = computed(() => [
    { value: 'all', label: formatMessage(messages.levelAll) },
    { value: 'debug', label: formatMessage(messages.levelDebug) },
    { value: 'info', label: formatMessage(messages.levelInfo) },
])

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
                title: formatMessage(messages.exportedTitle),
                text: formatMessage(messages.exportedText, { path }),
                type: 'success',
                buttons: [
                    {
                        label: formatMessage(commonMessages.openInFolderButton),
                        icon: FolderOpenIcon,
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
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        :closable="!exporting"
        :close-on-click-outside="!exporting"
        width="40rem"
        scrollable
    >
        <div class="flex flex-col gap-4">
            <p class="m-0">{{ formatMessage(messages.description) }}</p>

            <div class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.rangeLabel)
                }}</span>
                <Combobox
                    id="log-export-range"
                    v-model="selectedRange"
                    :disabled="exporting"
                    :name="formatMessage(messages.rangeLabel)"
                    :options="rangeOptions"
                />
            </div>

            <div class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.levelLabel)
                }}</span>
                <Combobox
                    id="log-export-level"
                    v-model="selectedLevel"
                    :disabled="exporting"
                    :name="formatMessage(messages.levelLabel)"
                    :options="levelOptions"
                />
            </div>

            <div class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.extrasLabel)
                }}</span>
                <Checkbox
                    v-model="includeSystemInfo"
                    :disabled="exporting"
                    :label="formatMessage(messages.systemInfo)"
                />
                <Checkbox
                    v-model="includeInstanceLogs"
                    :disabled="exporting"
                    :label="formatMessage(messages.instanceLogs)"
                />
                <Checkbox
                    v-model="includeCrashAnalysis"
                    :disabled="exporting"
                    :label="formatMessage(messages.crashAnalysis)"
                />
                <Checkbox
                    v-model="includeNetworkConnections"
                    :disabled="exporting"
                    :label="formatMessage(messages.networkConnections)"
                />
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.networkConnectionsHint) }}
                </p>
            </div>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" :disabled="exporting" @click="modal?.hide()"
                    ><XIcon />
                    {{ formatMessage(messages.cancel) }}
                </Button>
                <Button type="colored" color="brand" :loading="exporting" @click="exportLogs"
                    ><DownloadIcon />
                    {{ formatMessage(exporting ? messages.exporting : messages.confirm) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
