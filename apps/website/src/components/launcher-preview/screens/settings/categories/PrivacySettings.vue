<script setup lang="ts">
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref } from 'vue'

import SettingsRow from '../components/SettingsRow.vue'
import SettingsSaveStatus from '../components/SettingsSaveStatus.vue'
import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { useSettingsPage } from '../state'
import PrivilegedConsentModal from './privacy/PrivilegedConsentModal.vue'

/**
 * Privacy & data sharing (apps/app-frontend/src/components/ui/settings/PrivacySettings.vue).
 * Every toggle saves on its own and reports through the section's save status;
 * enabling privileged link actions first asks for the forced-reading consent.
 */
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { settings, simulate } = useSettingsPage()

/** `privacy_get`: the privacy part of the settings row. */
const privacy = ref({
    telemetry: settings.telemetry,
    discord_rpc: settings.discord_rpc,
    consent_version: settings.telemetry_consent_version,
})
const telemetrySaving = ref(false)
const discordSaving = ref(false)
const schemeSaving = ref(false)
const allowExternalScheme = ref(settings.allow_external_scheme)
const privilegedSaving = ref(false)
const allowPrivilegedScheme = ref(settings.allow_privileged_scheme)
const privilegedConsentModal = ref<InstanceType<typeof PrivilegedConsentModal> | null>(null)
const lastSaveState = ref<'idle' | 'saved' | 'error'>('idle')
const retrySave = ref<(() => void) | undefined>()

/** The app's current telemetry consent version, recorded when telemetry is enabled. */
const TELEMETRY_CONSENT_VERSION = 1

async function setTelemetryEnabled(enabled: boolean) {
    await simulate()
    settings.telemetry = enabled
    if (enabled) settings.telemetry_consent_version = TELEMETRY_CONSENT_VERSION
    return {
        telemetry: settings.telemetry,
        discord_rpc: settings.discord_rpc,
        consent_version: settings.telemetry_consent_version,
    }
}

async function setDiscordRpcEnabled(enabled: boolean) {
    await simulate()
    settings.discord_rpc = enabled
    return {
        telemetry: settings.telemetry,
        discord_rpc: settings.discord_rpc,
        consent_version: settings.telemetry_consent_version,
    }
}

async function updateSettings(patch: {
    allow_external_scheme?: boolean
    allow_privileged_scheme?: boolean
}) {
    await simulate()
    Object.assign(settings, patch)
}

const saveStatus = computed(() => {
    if (
        telemetrySaving.value ||
        discordSaving.value ||
        schemeSaving.value ||
        privilegedSaving.value
    )
        return 'saving'
    return lastSaveState.value
})

async function updateTelemetry(value: boolean) {
    if (telemetrySaving.value) return
    const previous = privacy.value.telemetry
    privacy.value.telemetry = value
    telemetrySaving.value = true
    lastSaveState.value = 'idle'
    retrySave.value = undefined
    try {
        const saved = await setTelemetryEnabled(value)
        privacy.value.telemetry = saved.telemetry
        privacy.value.consent_version = saved.consent_version
        lastSaveState.value = 'saved'
    } catch (error) {
        privacy.value.telemetry = previous
        retrySave.value = () => void updateTelemetry(value)
        lastSaveState.value = 'error'
        handleError(error as Error)
    } finally {
        telemetrySaving.value = false
    }
}

async function updateExternalScheme(value: boolean) {
    if (schemeSaving.value) return
    const previous = allowExternalScheme.value
    allowExternalScheme.value = value
    schemeSaving.value = true
    lastSaveState.value = 'idle'
    retrySave.value = undefined
    try {
        await updateSettings({ allow_external_scheme: value })
        lastSaveState.value = 'saved'
    } catch (error) {
        allowExternalScheme.value = previous
        retrySave.value = () => void updateExternalScheme(value)
        lastSaveState.value = 'error'
        handleError(error as Error)
    } finally {
        schemeSaving.value = false
    }
}

async function updatePrivilegedScheme(value: boolean) {
    if (privilegedSaving.value) return
    // Enabling requires reading the risk warning first; cancelling keeps it off.
    if (value) {
        const consented = await privilegedConsentModal.value?.request()
        if (!consented) return
    }
    const previous = allowPrivilegedScheme.value
    allowPrivilegedScheme.value = value
    privilegedSaving.value = true
    lastSaveState.value = 'idle'
    retrySave.value = undefined
    try {
        await updateSettings({ allow_privileged_scheme: value })
        lastSaveState.value = 'saved'
    } catch (error) {
        allowPrivilegedScheme.value = previous
        retrySave.value = () => void updatePrivilegedScheme(value)
        lastSaveState.value = 'error'
        handleError(error as Error)
    } finally {
        privilegedSaving.value = false
    }
}

async function updateDiscordRpc(value: boolean) {
    if (discordSaving.value) return
    const previous = privacy.value.discord_rpc
    privacy.value.discord_rpc = value
    discordSaving.value = true
    lastSaveState.value = 'idle'
    retrySave.value = undefined
    try {
        const saved = await setDiscordRpcEnabled(value)
        privacy.value.discord_rpc = saved.discord_rpc
        lastSaveState.value = 'saved'
    } catch (error) {
        privacy.value.discord_rpc = previous
        retrySave.value = () => void updateDiscordRpc(value)
        lastSaveState.value = 'error'
        handleError(error as Error)
    } finally {
        discordSaving.value = false
    }
}
</script>

<template>
    <div class="flex w-full flex-col gap-6">
        <SettingsSection
            :title="formatMessage(messages.appSettingsPrivacySectionTitle)"
            title-id="settings-target-privacy"
        >
            <template #extra>
                <SettingsSaveStatus :status="saveStatus" :retry="retrySave" />
            </template>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-privacy-telemetry" tabindex="-1">
                        {{ formatMessage(messages.appSettingsPrivacyTelemetry) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsPrivacyTelemetryDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="privacy-telemetry"
                        :model-value="privacy.telemetry"
                        :disabled="telemetrySaving"
                        @update:model-value="(value) => updateTelemetry(!!value)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-privacy-discord-rpc" tabindex="-1">
                        {{ formatMessage(messages.appSettingsPrivacyDiscordRpc) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsPrivacyDiscordRpcDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="privacy-discord-rpc"
                        :model-value="privacy.discord_rpc"
                        :disabled="discordSaving"
                        @update:model-value="(value) => updateDiscordRpc(!!value)"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>
        <p class="settings-page-note">
            {{ formatMessage(messages.appSettingsPrivacyDataHandling) }}
        </p>
        <SettingsSection
            :title="formatMessage(messages.appSettingsPrivacySecurityTitle)"
            title-id="settings-target-privacy"
        >
            <template #extra>
                <SettingsSaveStatus :status="saveStatus" :retry="retrySave" />
            </template>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-privacy-external-scheme" tabindex="-1">
                        {{ formatMessage(messages.appSettingsPrivacyExternalScheme) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsPrivacyExternalSchemeDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="privacy-external-scheme"
                        :model-value="allowExternalScheme"
                        :disabled="schemeSaving"
                        @update:model-value="(value) => updateExternalScheme(!!value)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-privacy-privileged-scheme" tabindex="-1">
                        {{ formatMessage(messages.appSettingsPrivacyPrivilegedScheme) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsPrivacyPrivilegedSchemeDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="privacy-privileged-scheme"
                        :model-value="allowPrivilegedScheme"
                        :disabled="privilegedSaving"
                        @update:model-value="(value) => updatePrivilegedScheme(!!value)"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>
    </div>
    <PrivilegedConsentModal ref="privilegedConsentModal" />
</template>

<style scoped>
.settings-page-note {
    margin: 0;
    color: var(--color-text-tertiary);
    font-size: 0.8125rem;
    line-height: 1.5;
}
</style>
