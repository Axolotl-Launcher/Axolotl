<script setup lang="ts">
import { EyeIcon, RefreshCwIcon, XIcon } from '@modrinth/assets'
import {
    Button,
    defineMessages,
    injectNotificationManager,
    NewModal,
    Toggle,
    useVIntl,
} from '@modrinth/ui'
import { getVersion } from '@tauri-apps/api/app'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { inject, nextTick, ref, watch } from 'vue'

import UpdateAnnouncementHistory from '@/components/ui/announcement/UpdateAnnouncementHistory.vue'
import {
    getUpdateChannel,
    getUpdatePreferences,
    setUpdateChannel,
    setUpdatePreferences,
    type UpdateChannel,
} from '@/helpers/settings.ts'
import { isDev, restartApp } from '@/helpers/utils.js'
import { type AppUpdateCheckResult, checkForAppUpdate } from '@/providers/app-update.ts'

import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const [activeChannel, initialUpdatePreferences, currentVersion, isDevEnvironment] =
    await Promise.all([getUpdateChannel(), getUpdatePreferences(), getVersion(), isDev()])
const selectedChannel = ref<UpdateChannel>(activeChannel)
const updatePreferences = ref(initialUpdatePreferences)
const checking = ref(false)
const checkResult = ref<AppUpdateCheckResult | 'failed' | null>(null)
const latestChannelVersions = ref<Partial<Record<UpdateChannel, string>>>({})
const latestChannelVersionsLoaded = ref(false)
const previewUpdateAnnouncement = inject<(version: string) => void>('previewUpdateAnnouncement')
const restartModal = ref<InstanceType<typeof NewModal>>()
let restoringChannelSelection = false

const messages = defineMessages({
    title: {
        id: 'app.settings.updates.channel.title',
        defaultMessage: 'Update channel',
    },
    preferencesTitle: {
        id: 'app.settings.updates.preferences.title',
        defaultMessage: 'Update behavior',
    },
    checkTitle: {
        id: 'app.settings.updates.check.title',
        defaultMessage: 'Check for updates',
    },
    description: {
        id: 'app.settings.updates.channel.description',
        defaultMessage: 'Choose which launcher versions Axolotl receives.',
    },
    channelLabel: {
        id: 'app.settings.updates.channel.label',
        defaultMessage: 'Channel',
    },
    release: {
        id: 'app.settings.updates.channel.release',
        defaultMessage: 'Release',
    },
    releaseDescription: {
        id: 'app.settings.updates.channel.release-description',
        defaultMessage: 'Stable, tested features and fixes. Updates arrive less often.',
    },
    beta: {
        id: 'app.settings.updates.channel.beta',
        defaultMessage: 'Beta',
    },
    betaDescription: {
        id: 'app.settings.updates.channel.beta-description',
        defaultMessage:
            'New, less stable updates with the latest features. Updates arrive more often.',
    },
    betaImmediateFetch: {
        id: 'app.settings.updates.immediate-fetch.beta-description',
        defaultMessage: 'Beta updates are always available immediately.',
    },
    check: {
        id: 'app.settings.updates.check',
        defaultMessage: 'Check for updates',
    },
    checking: {
        id: 'app.settings.updates.checking',
        defaultMessage: 'Checking for updates…',
    },
    available: {
        id: 'app.settings.updates.available',
        defaultMessage: 'An update is available.',
    },
    upToDate: {
        id: 'app.settings.updates.up-to-date',
        defaultMessage: 'Axolotl is up to date.',
    },
    disabled: {
        id: 'app.settings.updates.disabled',
        defaultMessage: 'Updates are disabled in this build.',
    },
    offline: {
        id: 'app.settings.updates.offline',
        defaultMessage: 'Connect to the internet to check for updates.',
    },
    failed: {
        id: 'app.settings.updates.failed',
        defaultMessage: 'Could not check for updates.',
    },
    security: {
        id: 'app.settings.updates.security',
        defaultMessage: 'Updates are installed only when their cryptographic signature is valid.',
    },
    currentVersion: {
        id: 'app.settings.updates.current-version',
        defaultMessage: 'Current version {version}',
    },
    latestVersion: {
        id: 'app.settings.updates.latest-version',
        defaultMessage: 'Latest {channel} version: {version}',
    },
    latestVersionUnavailable: {
        id: 'app.settings.updates.latest-version-unavailable',
        defaultMessage: 'Latest version unavailable',
    },
    latestVersionLoading: {
        id: 'app.settings.updates.latest-version-loading',
        defaultMessage: 'Fetching latest version…',
    },
    preview: {
        id: 'app.settings.updates.preview-announcement',
        defaultMessage: 'Preview update announcement',
    },
    restartTitle: {
        id: 'app.settings.updates.channel.restart-title',
        defaultMessage: 'Restart required',
    },
    restartDescription: {
        id: 'app.settings.updates.channel.restart-description',
        defaultMessage:
            'Restart Axolotl now to start using the new update channel, or restart manually later.',
    },
    restartDevelopmentDescription: {
        id: 'app.settings.updates.channel.restart-development-description',
        defaultMessage:
            'The new update channel will be used after you manually restart the development session.',
    },
    restartNow: {
        id: 'app.settings.updates.channel.restart-now',
        defaultMessage: 'Restart now',
    },
    restartLater: {
        id: 'app.settings.updates.channel.restart-later',
        defaultMessage: 'Restart manually later',
    },
    immediateFetch: {
        id: 'app.settings.updates.immediate-fetch',
        defaultMessage: 'Get updates as soon as they are available',
    },
    immediateFetchDescription: {
        id: 'app.settings.updates.immediate-fetch-description',
        defaultMessage:
            'After the latest stable changes and fixes are released, be among the first to receive them.',
    },
    pause: {
        id: 'app.settings.updates.pause',
        defaultMessage: 'Pause updates',
    },
    pauseDescription: {
        id: 'app.settings.updates.pause-description',
        defaultMessage:
            'Stop automatic update checks, downloads, and update notifications until you resume updates.',
    },
    paused: {
        id: 'app.settings.updates.paused',
        defaultMessage: 'Updates are paused.',
    },
})

async function loadLatestChannelVersions() {
    const versions = await Promise.all(
        (['release', 'beta'] as const).map(async (channel) => {
            try {
                const response = await tauriFetch(
                    `https://update.axlmc.org/latest?channel=${channel}`,
                )
                if (!response.ok) return [channel, undefined] as const
                const payload = (await response.json()) as { version?: string }
                return [channel, payload.version] as const
            } catch {
                return [channel, undefined] as const
            }
        }),
    )
    latestChannelVersions.value = Object.fromEntries(versions.filter(([, version]) => version))
    latestChannelVersionsLoaded.value = true
}

void loadLatestChannelVersions()

const resultMessages: Record<AppUpdateCheckResult | 'failed', keyof typeof messages> = {
    available: 'available',
    'up-to-date': 'upToDate',
    disabled: 'disabled',
    offline: 'offline',
    failed: 'failed',
    paused: 'paused',
}

watch(selectedChannel, async (channel, previousChannel) => {
    if (restoringChannelSelection) return

    if (channel === 'beta') updatePreferences.value.immediateUpdateFetch = true
    if (!(await applyChannel(channel))) {
        restoringChannelSelection = true
        selectedChannel.value = previousChannel
        await nextTick()
        restoringChannelSelection = false
    }
    checkResult.value = null
})

async function applyChannel(channel: UpdateChannel) {
    try {
        await setUpdateChannel(channel)
        restartModal.value?.show()
        return true
    } catch (error) {
        handleError(error)
        return false
    }
}

async function restartForChannelChange() {
    restartModal.value?.hide()
    try {
        await restartApp()
    } catch (error) {
        handleError(error)
    }
}

async function saveUpdatePreferences() {
    try {
        await setUpdatePreferences(updatePreferences.value)
    } catch (error) {
        handleError(error)
    }
}

async function checkForUpdates() {
    checking.value = true
    checkResult.value = null

    try {
        checkResult.value = await checkForAppUpdate()
    } catch (error) {
        checkResult.value = 'failed'
        handleError(error)
    } finally {
        checking.value = false
    }
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <SettingsSection :title="formatMessage(messages.title)">
            <div class="update-channel-panel">
                <p
                    id="settings-target-updates-channel"
                    class="m-0 text-sm leading-[1.45] text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.description) }}
                </p>
                <div
                    class="update-channel-options"
                    role="radiogroup"
                    :aria-label="formatMessage(messages.channelLabel)"
                >
                    <button
                        type="button"
                        class="update-channel-card"
                        :class="{ 'update-channel-card-selected': selectedChannel === 'release' }"
                        role="radio"
                        :aria-checked="selectedChannel === 'release'"
                        @click="selectedChannel = 'release'"
                    >
                        <span class="update-channel-card-title">{{
                            formatMessage(messages.release)
                        }}</span>
                        <span class="update-channel-card-description">
                            {{ formatMessage(messages.releaseDescription) }}
                        </span>
                        <span class="update-channel-card-version">
                            {{
                                !latestChannelVersionsLoaded
                                    ? formatMessage(messages.latestVersionLoading)
                                    : latestChannelVersions.release
                                      ? formatMessage(messages.latestVersion, {
                                            channel: formatMessage(messages.release),
                                            version: latestChannelVersions.release,
                                        })
                                      : formatMessage(messages.latestVersionUnavailable)
                            }}
                        </span>
                    </button>
                    <button
                        type="button"
                        class="update-channel-card"
                        :class="{ 'update-channel-card-selected': selectedChannel === 'beta' }"
                        role="radio"
                        :aria-checked="selectedChannel === 'beta'"
                        @click="selectedChannel = 'beta'"
                    >
                        <span class="update-channel-card-title">{{
                            formatMessage(messages.beta)
                        }}</span>
                        <span class="update-channel-card-description">
                            {{ formatMessage(messages.betaDescription) }}
                        </span>
                        <span class="update-channel-card-version">
                            {{
                                !latestChannelVersionsLoaded
                                    ? formatMessage(messages.latestVersionLoading)
                                    : latestChannelVersions.beta
                                      ? formatMessage(messages.latestVersion, {
                                            channel: formatMessage(messages.beta),
                                            version: latestChannelVersions.beta,
                                        })
                                      : formatMessage(messages.latestVersionUnavailable)
                            }}
                        </span>
                    </button>
                </div>
            </div>
        </SettingsSection>

        <SettingsSection :title="formatMessage(messages.preferencesTitle)">
            <SettingsRow>
                <template #label>{{ formatMessage(messages.immediateFetch) }}</template>
                <template #description>
                    {{
                        selectedChannel === 'beta'
                            ? formatMessage(messages.betaImmediateFetch)
                            : formatMessage(messages.immediateFetchDescription)
                    }}
                </template>
                <template #control>
                    <Toggle
                        id="immediate-update-fetch"
                        v-model="updatePreferences.immediateUpdateFetch"
                        :disabled="selectedChannel === 'beta'"
                        @update:model-value="saveUpdatePreferences"
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.pause) }}</template>
                <template #description>{{ formatMessage(messages.pauseDescription) }}</template>
                <template #control>
                    <Toggle
                        id="pause-updates"
                        v-model="updatePreferences.updatesPaused"
                        @update:model-value="saveUpdatePreferences"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection :title="formatMessage(messages.checkTitle)">
            <div class="update-check-panel">
                <div class="update-check-heading">
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.currentVersion, { version: currentVersion }) }}
                    </p>
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.security) }}
                    </p>
                </div>
                <div class="flex flex-wrap gap-2">
                    <Button
                        type="colored"
                        color="brand"
                        :disabled="checking"
                        @click="checkForUpdates"
                    >
                        <RefreshCwIcon :class="{ 'animate-spin': checking }" />
                        {{ formatMessage(checking ? messages.checking : messages.check) }}
                    </Button>
                    <Button
                        v-if="isDevEnvironment && previewUpdateAnnouncement"
                        type="outlined"
                        native-type="button"
                        @click="previewUpdateAnnouncement(currentVersion)"
                    >
                        <EyeIcon />
                        {{ formatMessage(messages.preview) }}
                    </Button>
                </div>
                <p
                    v-if="checkResult"
                    class="update-check-result"
                    :class="`update-check-result-${checkResult}`"
                    role="status"
                >
                    {{ formatMessage(messages[resultMessages[checkResult]]) }}
                </p>
            </div>
        </SettingsSection>

        <UpdateAnnouncementHistory :current-version="currentVersion" />
    </div>

    <NewModal
        ref="restartModal"
        :header="formatMessage(messages.restartTitle)"
        :closable="true"
        :close-on-click-outside="true"
    >
        <p class="m-0">
            {{
                formatMessage(
                    isDevEnvironment
                        ? messages.restartDevelopmentDescription
                        : messages.restartDescription,
                )
            }}
        </p>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="restartModal?.hide()"
                    ><XIcon />
                    {{ formatMessage(messages.restartLater) }}
                </Button>
                <Button
                    v-if="!isDevEnvironment"
                    type="colored"
                    color="brand"
                    @click="restartForChannelChange"
                    ><RefreshCwIcon />
                    {{ formatMessage(messages.restartNow) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<style scoped>
.update-check-panel {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--gap-md);
    padding: var(--gap-lg);
}

.update-channel-panel {
    display: flex;
    flex-direction: column;
    gap: var(--gap-md);
    padding: var(--gap-lg);
}

.update-channel-options {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--gap-md);
    width: 100%;
}

.update-channel-card {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    width: 100%;
    min-width: 0;
    gap: var(--gap-xs);
    padding: var(--gap-md);
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-md);
    background: var(--surface-1);
    color: var(--color-text-tertiary);
    text-align: left;
    cursor: pointer;
    transition:
        border-color 150ms ease,
        background-color 150ms ease,
        color 150ms ease,
        transform 150ms ease;
}

.update-channel-card:hover {
    border-color: color-mix(in srgb, var(--color-brand) 55%, var(--surface-4));
    background: var(--surface-3);
}

.update-channel-card:active {
    transform: scale(0.98);
}

.update-channel-card:focus-visible {
    outline: 2px solid var(--color-brand);
    outline-offset: 2px;
}

.update-channel-card-selected {
    border-color: var(--color-brand);
    background: color-mix(in srgb, var(--color-brand) 12%, var(--surface-1));
}

.update-channel-card-title {
    color: var(--color-text-primary);
    font-weight: 600;
}

.update-channel-card-description {
    font-size: 0.875rem;
    line-height: 1.4;
}

.update-channel-card-version {
    grid-column: 2;
    grid-row: 1 / span 2;
    align-self: center;
    color: var(--color-text-tertiary);
    font-size: 0.8125rem;
    font-weight: 600;
}

.update-check-heading {
    display: flex;
    flex-direction: column;
    gap: var(--gap-xs);
}

.update-check-result {
    margin: 0;
    padding: var(--gap-sm) var(--gap-md);
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-sm);
    background: var(--surface-1);
    color: var(--color-text-tertiary);
    font-size: 0.875rem;
    line-height: 1.4;
}

.update-check-result-available {
    border-color: color-mix(in srgb, var(--color-brand) 45%, var(--surface-4));
    color: var(--color-brand);
}

.update-check-result-failed,
.update-check-result-offline {
    border-color: color-mix(in srgb, var(--color-red) 45%, var(--surface-4));
    color: var(--color-red);
}

.update-check-result-paused,
.update-check-result-disabled {
    border-color: color-mix(in srgb, var(--color-yellow) 45%, var(--surface-4));
    color: var(--color-yellow);
}
</style>
