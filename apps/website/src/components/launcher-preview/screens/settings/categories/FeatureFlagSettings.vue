<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { ref } from 'vue'

import SettingsRow from '../components/SettingsRow.vue'
import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { DEFAULT_FEATURE_FLAGS, type FeatureFlag, useSettingsPage, useThemeStore } from '../state'
import AnnouncementPreview from './flags/AnnouncementPreview.vue'

/**
 * Feature flags (apps/app-frontend/src/components/ui/settings/FeatureFlagSettings.vue),
 * the developer-only category. Every flag writes through to the window's
 * settings, so the shell reacts at once (page transitions, the worlds tab).
 * This is a release build (`is_dev` is false), so the debug-only developer
 * tools section stays hidden, as it does in the shipped app.
 */
const themeStore = useThemeStore()
const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const { simulate, setFeatureFlag: writeFeatureFlag } = useSettingsPage()

const announcementPreview = ref<InstanceType<typeof AnnouncementPreview>>()
const previewWithAction = ref(false)
const clearingOfficialLoginMarker = ref(false)

const options = ref<FeatureFlag[]>(Object.keys(DEFAULT_FEATURE_FLAGS) as FeatureFlag[])
const featureFlagLabels: Record<FeatureFlag, MessageDescriptor> = {
    project_background: messages.appSettingsFeatureFlagsProjectBackground,
    page_path: messages.appSettingsFeatureFlagsPagePath,
    worlds_tab: messages.appSettingsFeatureFlagsWorldsTab,
    worlds_in_home: messages.appSettingsFeatureFlagsWorldsInHome,
    server_project_qa: messages.appSettingsFeatureFlagsServerProjectQa,
    show_version_environment_column: messages.appSettingsFeatureFlagsShowVersionEnvironmentColumn,
    server_ram_as_bytes_always_on: messages.appSettingsFeatureFlagsServerRamAsBytesAlwaysOn,
    always_show_app_controls: messages.appSettingsFeatureFlagsAlwaysShowAppControls,
    skip_non_essential_warnings: messages.appSettingsFeatureFlagsSkipNonEssentialWarnings,
    skip_unknown_pack_warning: messages.appSettingsFeatureFlagsSkipUnknownPackWarning,
    i18n_debug: messages.appSettingsFeatureFlagsI18nDebug,
    show_instance_play_time: messages.appSettingsFeatureFlagsShowInstancePlayTime,
    page_transitions: messages.appSettingsFeatureFlagsPageTransitions,
    advanced_filters_collapsed: messages.appSettingsFeatureFlagsAdvancedFiltersCollapsed,
    auto_install_dependencies: messages.appSettingsFeatureFlagsAutoInstallDependencies,
}

/** The theme store and the saved settings share one `feature_flags` record here. */
function setFeatureFlag(key: FeatureFlag, value: boolean) {
    writeFeatureFlag(key, value)
}

/** `useAntiPiracyStatus().clear()`: `clear_official_login_marker`. */
async function clearOfficialLoginMarker() {
    await simulate(260)
}

async function clearLoginMarker() {
    if (clearingOfficialLoginMarker.value) return
    clearingOfficialLoginMarker.value = true
    try {
        await clearOfficialLoginMarker()
        addNotification({
            title: formatMessage(messages.appSettingsDeveloperOfficialLoginMarkerCleared),
            type: 'success',
        })
    } catch (error) {
        handleError(error as Error)
    } finally {
        clearingOfficialLoginMarker.value = false
    }
}

function previewRemoteAnnouncement(type: 'modal' | 'notification', withAction: boolean) {
    announcementPreview.value?.preview(type, withAction)
}
</script>
<template>
    <SettingsSection
        :title="formatMessage(messages.appSettingsDeveloperFeatureFlagsSectionTitle)"
        title-id="settings-target-feature-flags"
    >
        <SettingsRow v-for="option in options" :key="option">
            <template #label>{{ formatMessage(featureFlagLabels[option]) }}</template>
            <template #control>
                <div class="flex items-center gap-2">
                    <Button
                        type="quiet"
                        :disabled="
                            themeStore.getFeatureFlag(option) === DEFAULT_FEATURE_FLAGS[option]
                        "
                        @click="setFeatureFlag(option, DEFAULT_FEATURE_FLAGS[option])"
                    >
                        {{ formatMessage(messages.appSettingsFeatureFlagsResetToDefault) }}
                    </Button>
                    <Toggle
                        :id="`feature-flag-${option}`"
                        :model-value="themeStore.getFeatureFlag(option)"
                        @update:model-value="
                            () => setFeatureFlag(option, !themeStore.getFeatureFlag(option))
                        "
                    />
                </div>
            </template>
        </SettingsRow>
    </SettingsSection>

    <SettingsSection :title="formatMessage(messages.appSettingsDeveloperOfficialLoginSection)">
        <div class="flex flex-wrap items-center justify-between gap-3 p-4">
            <p class="m-0 max-w-xl text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.appSettingsDeveloperOfficialLoginDescription) }}
            </p>
            <Button type="base" :disabled="clearingOfficialLoginMarker" @click="clearLoginMarker">
                {{ formatMessage(messages.appSettingsDeveloperClearOfficialLoginMarker) }}
            </Button>
        </div>
    </SettingsSection>

    <SettingsSection v-if="themeStore.devMode">
        <template #header>
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.appSettingsDeveloperAnnouncementPreview) }}
            </h2>
        </template>
        <div class="flex flex-col gap-4 p-4">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.appSettingsDeveloperAnnouncementPreviewDescription) }}
            </p>
            <div class="flex items-center gap-2">
                <Toggle id="announcement-preview-action" v-model="previewWithAction" />
                <label for="announcement-preview-action">{{
                    formatMessage(messages.appSettingsDeveloperPreviewWithAction)
                }}</label>
            </div>
            <div class="flex flex-wrap gap-2">
                <Button type="base" @click="previewRemoteAnnouncement('modal', previewWithAction)">
                    {{ formatMessage(messages.appSettingsDeveloperPreviewAnnouncementModal) }}
                </Button>
                <Button
                    type="base"
                    @click="previewRemoteAnnouncement('notification', previewWithAction)"
                >
                    {{ formatMessage(messages.appSettingsDeveloperPreviewAnnouncementPopup) }}
                </Button>
            </div>
        </div>
    </SettingsSection>

    <AnnouncementPreview ref="announcementPreview" :ready="true" />
</template>
