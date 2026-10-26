<script setup lang="ts">
import { WrenchIcon } from '@modrinth/assets'
import { Button, defineMessages, injectNotificationManager, Toggle, useVIntl } from '@modrinth/ui'
import { inject, ref, watch } from 'vue'

import { useAntiPiracyStatus } from '@/composables/useAntiPiracyStatus'
import McpDebugSettings from '@/features/mcp/McpDebugSettings.vue'
import { get as getSettings, update as updateSettings } from '@/helpers/settings.ts'
import { createSettingsPatchSaver } from '@/helpers/settings-patch'
import { isDev } from '@/helpers/utils'
import { handleSevereError } from '@/store/error.js'
import { useTheming } from '@/store/state'
import { DEFAULT_FEATURE_FLAGS, type FeatureFlag } from '@/store/theme.ts'

import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const themeStore = useTheming()
const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const isDevEnvironment = await isDev()
const previewMinecraftCrashModal = inject<() => void>('previewMinecraftCrashModal')
const previewPrivacyConsentModal = inject<() => Promise<void>>('previewPrivacyConsentModal')
const previewRemoteAnnouncement = inject<
    (type: 'modal' | 'notification', withAction: boolean) => void
>('previewRemoteAnnouncement')
const previewWithAction = ref(false)
const { clear: clearOfficialLoginMarker } = useAntiPiracyStatus()
const clearingOfficialLoginMarker = ref(false)
const messages = defineMessages({
    featureFlagsSectionTitle: {
        id: 'app.settings.developer.feature-flags-section-title',
        defaultMessage: 'Feature flags',
    },
    projectBackground: {
        id: 'app.settings.feature-flags.project-background',
        defaultMessage: 'Project backgrounds',
    },
    pagePath: { id: 'app.settings.feature-flags.page-path', defaultMessage: 'Page path' },
    worldsTab: { id: 'app.settings.feature-flags.worlds-tab', defaultMessage: 'Worlds tab' },
    worldsInHome: {
        id: 'app.settings.feature-flags.worlds-in-home',
        defaultMessage: 'Worlds on home page',
    },
    serverProjectQa: {
        id: 'app.settings.feature-flags.server-project-qa',
        defaultMessage: 'Server project QA',
    },
    showVersionEnvironmentColumn: {
        id: 'app.settings.feature-flags.show-version-environment-column',
        defaultMessage: 'Show version environment column',
    },
    serverRamAsBytesAlwaysOn: {
        id: 'app.settings.feature-flags.server-ram-as-bytes-always-on',
        defaultMessage: 'Always show server RAM as bytes',
    },
    alwaysShowAppControls: {
        id: 'app.settings.feature-flags.always-show-app-controls',
        defaultMessage: 'Always show app controls',
    },
    skipNonEssentialWarnings: {
        id: 'app.settings.feature-flags.skip-non-essential-warnings',
        defaultMessage: 'Skip non-essential warnings',
    },
    skipUnknownPackWarning: {
        id: 'app.settings.feature-flags.skip-unknown-pack-warning',
        defaultMessage: 'Skip unknown pack warnings',
    },
    i18nDebug: {
        id: 'app.settings.feature-flags.i18n-debug',
        defaultMessage: 'Translation debugging',
    },
    showInstancePlayTime: {
        id: 'app.settings.feature-flags.show-instance-play-time',
        defaultMessage: 'Show instance play time',
    },
    pageTransitions: {
        id: 'app.settings.feature-flags.page-transitions',
        defaultMessage: 'Page transitions',
    },
    advancedFiltersCollapsed: {
        id: 'app.settings.feature-flags.advanced-filters-collapsed',
        defaultMessage: 'Collapse advanced filters by default',
    },
    autoInstallDependencies: {
        id: 'app.settings.feature-flags.auto-install-dependencies',
        defaultMessage: 'Automatically install dependencies',
    },
    announcementPreview: {
        id: 'app.settings.developer.announcement-preview',
        defaultMessage: 'Announcement preview',
    },
    announcementPreviewDescription: {
        id: 'app.settings.developer.announcement-preview-description',
        defaultMessage:
            'Preview the real announcement components using local sample content, without fetching or marking real announcements as read.',
    },
    previewWithAction: {
        id: 'app.settings.developer.preview-with-action',
        defaultMessage: 'Include an optional external-link button',
    },
    previewModal: {
        id: 'app.settings.developer.preview-announcement-modal',
        defaultMessage: 'Preview startup modal',
    },
    previewPopup: {
        id: 'app.settings.developer.preview-announcement-popup',
        defaultMessage: 'Preview Popup notification',
    },
    resetToDefault: {
        id: 'app.settings.feature-flags.reset-to-default',
        defaultMessage: 'Reset to default',
    },
    developerTools: {
        id: 'app.settings.about.developer-tools',
        defaultMessage: 'Developer tools',
    },
    testError: {
        id: 'app.settings.about.test-error',
        defaultMessage: 'Trigger test error',
    },
    testErrorMessage: {
        id: 'app.settings.about.test-error-message',
        defaultMessage: 'Test error triggered from the development settings.',
    },
    testNotificationError: {
        id: 'app.settings.about.test-notification-error',
        defaultMessage: 'Trigger notification test error',
    },
    testNotificationErrorTitle: {
        id: 'app.settings.about.test-notification-error-title',
        defaultMessage: 'Test notification error',
    },
    previewMinecraftCrashModal: {
        id: 'app.settings.about.preview-minecraft-crash-modal',
        defaultMessage: 'Preview Minecraft crash window',
    },
    previewPrivacyConsentModal: {
        id: 'app.settings.about.preview-privacy-consent-modal',
        defaultMessage: 'Preview privacy & security modal',
    },
    officialLoginSection: {
        id: 'app.settings.developer.official-login-section',
        defaultMessage: 'Official Minecraft login record',
    },
    officialLoginDescription: {
        id: 'app.settings.developer.official-login-description',
        defaultMessage:
            'Clear the official Minecraft sign-in record. You may need to sign in again before creating or using offline accounts.',
    },
    clearOfficialLoginMarker: {
        id: 'app.settings.developer.clear-official-login-marker',
        defaultMessage: 'Clear login record',
    },
    officialLoginMarkerCleared: {
        id: 'app.settings.developer.official-login-marker-cleared',
        defaultMessage: 'Official Minecraft login record cleared',
    },
})

const settings = ref(await getSettings())
const saveDraft = createSettingsPatchSaver(settings.value, updateSettings)
const options = ref<FeatureFlag[]>(Object.keys(DEFAULT_FEATURE_FLAGS))
const featureFlagLabels: Record<FeatureFlag, keyof typeof messages> = {
    project_background: 'projectBackground',
    page_path: 'pagePath',
    worlds_tab: 'worldsTab',
    worlds_in_home: 'worldsInHome',
    server_project_qa: 'serverProjectQa',
    show_version_environment_column: 'showVersionEnvironmentColumn',
    server_ram_as_bytes_always_on: 'serverRamAsBytesAlwaysOn',
    always_show_app_controls: 'alwaysShowAppControls',
    skip_non_essential_warnings: 'skipNonEssentialWarnings',
    skip_unknown_pack_warning: 'skipUnknownPackWarning',
    i18n_debug: 'i18nDebug',
    show_instance_play_time: 'showInstancePlayTime',
    page_transitions: 'pageTransitions',
    advanced_filters_collapsed: 'advancedFiltersCollapsed',
    auto_install_dependencies: 'autoInstallDependencies',
}

function setFeatureFlag(key: string, value: boolean) {
    themeStore.featureFlags[key] = value
    settings.value.feature_flags[key] = value
}

function triggerTestError() {
    handleSevereError(new Error(formatMessage(messages.testErrorMessage)))
}

function triggerTestNotificationError() {
    addNotification({
        title: formatMessage(messages.testNotificationErrorTitle),
        text: formatMessage(messages.testErrorMessage),
        type: 'error',
    })
}

async function clearLoginMarker() {
    if (clearingOfficialLoginMarker.value) return
    clearingOfficialLoginMarker.value = true
    try {
        await clearOfficialLoginMarker()
        addNotification({
            title: formatMessage(messages.officialLoginMarkerCleared),
            type: 'success',
        })
    } catch (error) {
        handleSevereError(error)
    } finally {
        clearingOfficialLoginMarker.value = false
    }
}

watch(
    settings,
    async () => {
        await saveDraft(settings.value)
    },
    { deep: true },
)
</script>
<template>
    <SettingsSection
        :title="formatMessage(messages.featureFlagsSectionTitle)"
        title-id="settings-target-feature-flags"
    >
        <SettingsRow v-for="option in options" :key="option">
            <template #label>{{ formatMessage(messages[featureFlagLabels[option]]) }}</template>
            <template #control>
                <div class="flex items-center gap-2">
                    <Button
                        type="quiet"
                        :disabled="
                            themeStore.getFeatureFlag(option) === DEFAULT_FEATURE_FLAGS[option]
                        "
                        @click="setFeatureFlag(option, DEFAULT_FEATURE_FLAGS[option])"
                    >
                        {{ formatMessage(messages.resetToDefault) }}
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

    <SettingsSection :title="formatMessage(messages.officialLoginSection)">
        <div class="flex flex-wrap items-center justify-between gap-3 p-4">
            <p class="m-0 max-w-xl text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.officialLoginDescription) }}
            </p>
            <Button type="base" :disabled="clearingOfficialLoginMarker" @click="clearLoginMarker">
                {{ formatMessage(messages.clearOfficialLoginMarker) }}
            </Button>
        </div>
    </SettingsSection>

    <McpDebugSettings />

    <SettingsSection v-if="(themeStore.devMode || isDevEnvironment) && previewRemoteAnnouncement">
        <template #header>
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.announcementPreview) }}
            </h2>
        </template>
        <div class="flex flex-col gap-4 p-4">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.announcementPreviewDescription) }}
            </p>
            <div class="flex items-center gap-2">
                <Toggle id="announcement-preview-action" v-model="previewWithAction" />
                <label for="announcement-preview-action">{{
                    formatMessage(messages.previewWithAction)
                }}</label>
            </div>
            <div class="flex flex-wrap gap-2">
                <Button type="base" @click="previewRemoteAnnouncement('modal', previewWithAction)">
                    {{ formatMessage(messages.previewModal) }}
                </Button>
                <Button
                    type="base"
                    @click="previewRemoteAnnouncement('notification', previewWithAction)"
                >
                    {{ formatMessage(messages.previewPopup) }}
                </Button>
            </div>
        </div>
    </SettingsSection>

    <SettingsSection v-if="isDevEnvironment">
        <template #header>
            <h2
                class="m-0 flex items-center gap-2 text-lg font-semibold text-[var(--color-text-primary)]"
            >
                <WrenchIcon class="size-5 text-[var(--color-text-tertiary)]" />
                {{ formatMessage(messages.developerTools) }}
            </h2>
        </template>
        <div class="flex flex-wrap gap-2 p-4">
            <Button type="base" @click="triggerTestError">
                <WrenchIcon /> {{ formatMessage(messages.testError) }}
            </Button>
            <Button type="base" @click="triggerTestNotificationError">
                <WrenchIcon /> {{ formatMessage(messages.testNotificationError) }}
            </Button>
            <Button type="base" @click="previewMinecraftCrashModal?.()">
                <WrenchIcon /> {{ formatMessage(messages.previewMinecraftCrashModal) }}
            </Button>
            <Button type="base" @click="previewPrivacyConsentModal?.()">
                <WrenchIcon /> {{ formatMessage(messages.previewPrivacyConsentModal) }}
            </Button>
        </div>
    </SettingsSection>
</template>
