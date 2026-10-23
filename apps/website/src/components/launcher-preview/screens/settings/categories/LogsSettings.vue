<script setup lang="ts">
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import UploadIcon from '@modrinth/assets/icons/upload.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref, watch } from 'vue'

import SettingsRow from '../components/SettingsRow.vue'
import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { type LogLevel, PREVIEW_APP_VERSION, type UpdateChannel, useSettingsPage } from '../state'
import { useFinder } from './logs/finder'
import { LAUNCHER_LOGS_DIR } from './logs/launcher-logs'
import LogExportModal from './logs/LogExportModal.vue'

/**
 * Logs (apps/app-frontend/src/components/ui/settings/LogsSettings.vue): the
 * level written to the launcher's session logs, the retention note, and the
 * export and open-folder actions. The level is saved straight into the
 * settings, normalized for the update channel as `settings_patch` does.
 */
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { settings, simulate } = useSettingsPage()
const finder = useFinder()

/**
 * `getUpdateChannel()`: the channel the Updates page chose (`set_update_channel`
 * writes it at once), else this build's own channel (`default_update_channel`,
 * a pre-release version means Beta). Read once, as the app awaits it on load.
 */
function getUpdateChannel(): UpdateChannel {
    const chosen = (settings as unknown as { update_channel?: unknown }).update_channel
    if (chosen === 'release' || chosen === 'beta') return chosen
    return PREVIEW_APP_VERSION.includes('-') ? 'beta' : 'release'
}

const updateChannel = getUpdateChannel()
const isBeta = updateChannel === 'beta'
const exportModal = ref<InstanceType<typeof LogExportModal>>()

const logLevelOptions = computed(() => {
    const verboseOptions = [
        { value: 'debug', label: formatMessage(messages.appSettingsLogsLevelDebug) },
        { value: 'trace', label: formatMessage(messages.appSettingsLogsLevelTrace) },
    ]
    if (isBeta) return verboseOptions

    return [
        { value: 'error', label: formatMessage(messages.appSettingsLogsLevelError) },
        { value: 'warn', label: formatMessage(messages.appSettingsLogsLevelWarn) },
        { value: 'info', label: formatMessage(messages.appSettingsLogsLevelInfo) },
        ...verboseOptions,
    ]
})

const levelHint = computed(() =>
    formatMessage(
        isBeta ? messages.appSettingsLogsLevelHintBeta : messages.appSettingsLogsLevelHint,
    ),
)

/** `log_level_for_channel`: Beta keeps diagnostics detailed by allowing only Debug or Trace. */
watch(
    () => settings.log_level,
    (level) => {
        if (isBeta && level !== 'debug' && level !== 'trace')
            settings.log_level = 'debug' as LogLevel
    },
)

/** `showLauncherLogsFolder()`: `plugin:utils|show_launcher_logs_folder` opens it in the Finder. */
async function showLauncherLogsFolder() {
    await simulate(120)
    finder.openFolder(LAUNCHER_LOGS_DIR)
}

async function openLogsFolder() {
    try {
        await showLauncherLogsFolder()
    } catch (error) {
        handleError(error as Error)
    }
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <SettingsSection
            :title="formatMessage(messages.appSettingsLogsLevelTitle)"
            :description="formatMessage(messages.appSettingsLogsLevelDescription)"
            title-id="settings-target-logs-level"
        >
            <SettingsRow stacked>
                <template #label>
                    <span>{{ formatMessage(messages.appSettingsLogsLevelTitle) }}</span>
                </template>
                <template #description>{{ levelHint }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox
                            id="log-level"
                            v-model="settings.log_level"
                            :name="formatMessage(messages.appSettingsLogsLevelTitle)"
                            :options="logLevelOptions"
                        />
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>

        <!-- Informational only: no controls, so it reads as a note rather than an
             empty card. -->
        <div class="flex flex-col gap-1">
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.appSettingsLogsRetentionTitle) }}
            </h2>
            <p class="m-0 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.appSettingsLogsRetentionDescription) }}
            </p>
        </div>

        <SettingsSection
            :title="formatMessage(messages.appSettingsLogsExportTitle)"
            :description="formatMessage(messages.appSettingsLogsExportDescription)"
            title-id="settings-target-logs-export"
        >
            <SettingsRow stacked>
                <template #label>
                    <span>{{ formatMessage(messages.appSettingsLogsExportTitle) }}</span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsLogsExportDescription)
                }}</template>
                <template #control>
                    <div class="flex flex-wrap gap-2">
                        <Button type="colored" color="brand" @click="exportModal?.show()"
                            ><UploadIcon />
                            {{ formatMessage(messages.appSettingsLogsExportButton) }}
                        </Button>
                        <Button @click="openLogsFolder"
                            ><FolderOpenIcon />
                            {{ formatMessage(messages.appSettingsLogsOpenFolder) }}
                        </Button>
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>

        <LogExportModal ref="exportModal" />
    </div>
</template>
