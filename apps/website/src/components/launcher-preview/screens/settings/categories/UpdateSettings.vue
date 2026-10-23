<script setup lang="ts">
import DatabaseIcon from '@modrinth/assets/icons/database.svg?component'
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { inject, nextTick, ref, watch } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import SettingsRow from '../components/SettingsRow.vue'
import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { PREVIEW_APP_VERSION, type UpdateChannel } from '../state'
import { type AppUpdateCheckResult, IS_DEV_BUILD, useUpdateBackend } from './updates/update-backend'
import UpdateAnnouncementHistory from './updates/UpdateAnnouncementHistory.vue'

/**
 * Settings > Updates (apps/app-frontend/src/components/ui/settings/UpdateSettings.vue):
 * the update channel with its separate databases, the update behaviour, the
 * manual check and the bundled release announcements. The channel commands,
 * the database copies, the Update Server and the restart are simulated by
 * `useUpdateBackend` over the app's own release catalog.
 */
const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const {
    getUpdateChannel,
    setUpdateChannel,
    getCurrentAppDatabasePath,
    betaDatabaseExists,
    copyReleaseDatabaseToBeta,
    copyDatabaseBetweenChannels,
    getUpdatePreferences,
    setUpdatePreferences,
    fetchLatestChannelVersions,
    checkForAppUpdate,
    restartApp,
} = useUpdateBackend()

const [activeChannel, initialUpdatePreferences, currentVersion, isDevEnvironment, databasePath] =
    await Promise.all([
        getUpdateChannel(),
        getUpdatePreferences(),
        PREVIEW_APP_VERSION,
        IS_DEV_BUILD,
        getCurrentAppDatabasePath().catch(() => ''),
    ])
const selectedChannel = ref<UpdateChannel>(activeChannel)
const updatePreferences = ref(initialUpdatePreferences)
const checking = ref(false)
const checkResult = ref<AppUpdateCheckResult | 'failed' | null>(null)
const currentDatabasePath = ref(databasePath)
const latestChannelVersions = ref<Partial<Record<UpdateChannel, string>>>({})
const latestChannelVersionsLoaded = ref(false)
const previewUpdateAnnouncement = inject<((version: string) => void) | undefined>(
    'previewUpdateAnnouncement',
    undefined,
)
const restartModal = ref<InstanceType<typeof PreviewModal>>()
const copyDatabaseModal = ref<InstanceType<typeof PreviewModal>>()
const databaseOperationModal = ref<InstanceType<typeof PreviewModal>>()
const pendingChannel = ref<UpdateChannel | null>(null)
const databaseOperation = ref<'release-to-beta' | 'beta-to-release' | ''>('')
const databaseOperationBusy = ref(false)
let restoringChannelSelection = false

async function loadLatestChannelVersions() {
    try {
        latestChannelVersions.value = await fetchLatestChannelVersions()
    } catch {
        latestChannelVersions.value = {}
    }
    latestChannelVersionsLoaded.value = true
}

void loadLatestChannelVersions()

const resultMessages = {
    available: messages.appSettingsUpdatesAvailable,
    'up-to-date': messages.appSettingsUpdatesUpToDate,
    disabled: messages.appSettingsUpdatesDisabled,
    offline: messages.appSettingsUpdatesOffline,
    failed: messages.appSettingsUpdatesFailed,
    paused: messages.appSettingsUpdatesPaused,
} as const

watch(selectedChannel, async (channel, previousChannel) => {
    if (restoringChannelSelection) return

    if (channel === 'beta') updatePreferences.value.immediateUpdateFetch = true
    if (channel === 'beta' && previousChannel === 'release') {
        try {
            if (!(await betaDatabaseExists())) {
                pendingChannel.value = channel
                restoringChannelSelection = true
                selectedChannel.value = previousChannel
                await nextTick()
                restoringChannelSelection = false
                copyDatabaseModal.value?.show()
                return
            }
        } catch (error) {
            restoringChannelSelection = true
            selectedChannel.value = previousChannel
            await nextTick()
            restoringChannelSelection = false
            handleError(error as Error)
            return
        }
    }

    await applyChannel(channel)
    checkResult.value = null
})

async function applyChannel(channel: UpdateChannel, copyDatabase = false) {
    try {
        if (copyDatabase) await copyReleaseDatabaseToBeta()
        await setUpdateChannel(channel)
        restartModal.value?.show()
        return true
    } catch (error) {
        handleError(error as Error)
        return false
    }
}

async function chooseBetaDatabase(copyDatabase: boolean) {
    const channel = pendingChannel.value
    pendingChannel.value = null
    copyDatabaseModal.value?.hide()
    if (channel && (await applyChannel(channel, copyDatabase))) {
        restoringChannelSelection = true
        selectedChannel.value = channel
        await nextTick()
        restoringChannelSelection = false
    }
}

function onCopyDatabaseModalHide() {
    // Dismissing this choice cancels the pending channel switch. The selected
    // channel was already restored before the modal was shown.
    pendingChannel.value = null
}

async function restartForChannelChange() {
    restartModal.value?.hide()
    try {
        await restartApp()
    } catch (error) {
        handleError(error as Error)
    }
}

async function saveUpdatePreferences() {
    try {
        await setUpdatePreferences(updatePreferences.value)
    } catch (error) {
        handleError(error as Error)
    }
}

async function checkForUpdates() {
    checking.value = true
    checkResult.value = null

    try {
        checkResult.value = await checkForAppUpdate()
    } catch (error) {
        checkResult.value = 'failed'
        handleError(error as Error)
    } finally {
        checking.value = false
    }
}

function requestDatabaseOperation() {
    if (!databaseOperation.value) return
    databaseOperationModal.value?.show()
}

function selectDatabaseOperation(operation: 'release-to-beta' | 'beta-to-release') {
    databaseOperation.value = operation
    requestDatabaseOperation()
}

async function confirmDatabaseOperation() {
    if (!databaseOperation.value || databaseOperationBusy.value) return
    const [sourceChannel, targetChannel] =
        databaseOperation.value === 'release-to-beta'
            ? (['release', 'beta'] as const)
            : (['beta', 'release'] as const)
    databaseOperationBusy.value = true
    databaseOperationModal.value?.hide()
    try {
        await copyDatabaseBetweenChannels(sourceChannel, targetChannel)
        addNotification({
            type: 'success',
            title: formatMessage(messages.appSettingsUpdatesDatabaseOperationSuccess, {
                source:
                    sourceChannel === 'release'
                        ? formatMessage(messages.appSettingsUpdatesDatabaseIsolationRelease)
                        : formatMessage(messages.appSettingsUpdatesDatabaseIsolationBeta),
                target:
                    targetChannel === 'release'
                        ? formatMessage(messages.appSettingsUpdatesDatabaseIsolationRelease)
                        : formatMessage(messages.appSettingsUpdatesDatabaseIsolationBeta),
            }),
        })
    } catch {
        handleError(
            new Error(
                targetChannel === activeChannel
                    ? formatMessage(messages.appSettingsUpdatesDatabaseOperationActiveTarget)
                    : formatMessage(messages.appSettingsUpdatesDatabaseOperationFailed),
            ),
        )
    } finally {
        databaseOperationBusy.value = false
    }
}

function cancelDatabaseOperation() {
    databaseOperationModal.value?.hide()
    databaseOperation.value = ''
}

function onDatabaseOperationModalHide() {
    databaseOperation.value = ''
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <SettingsSection :title="formatMessage(messages.appSettingsUpdatesChannelTitle)">
            <div class="update-channel-panel">
                <p
                    id="settings-target-updates-channel"
                    class="m-0 text-sm leading-[1.45] text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.appSettingsUpdatesChannelDescription) }}
                </p>
                <div
                    class="update-channel-options"
                    role="radiogroup"
                    :aria-label="formatMessage(messages.appSettingsUpdatesChannelLabel)"
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
                            formatMessage(messages.appSettingsUpdatesChannelRelease)
                        }}</span>
                        <span class="update-channel-card-description">
                            {{
                                formatMessage(messages.appSettingsUpdatesChannelReleaseDescription)
                            }}
                        </span>
                        <span class="update-channel-card-version">
                            {{
                                !latestChannelVersionsLoaded
                                    ? formatMessage(messages.appSettingsUpdatesLatestVersionLoading)
                                    : latestChannelVersions.release
                                      ? formatMessage(messages.appSettingsUpdatesLatestVersion, {
                                            channel: formatMessage(
                                                messages.appSettingsUpdatesChannelRelease,
                                            ),
                                            version: latestChannelVersions.release,
                                        })
                                      : formatMessage(
                                            messages.appSettingsUpdatesLatestVersionUnavailable,
                                        )
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
                            formatMessage(messages.appSettingsUpdatesChannelBeta)
                        }}</span>
                        <span class="update-channel-card-description">
                            {{ formatMessage(messages.appSettingsUpdatesChannelBetaDescription) }}
                        </span>
                        <span class="update-channel-card-version">
                            {{
                                !latestChannelVersionsLoaded
                                    ? formatMessage(messages.appSettingsUpdatesLatestVersionLoading)
                                    : latestChannelVersions.beta
                                      ? formatMessage(messages.appSettingsUpdatesLatestVersion, {
                                            channel: formatMessage(
                                                messages.appSettingsUpdatesChannelBeta,
                                            ),
                                            version: latestChannelVersions.beta,
                                        })
                                      : formatMessage(
                                            messages.appSettingsUpdatesLatestVersionUnavailable,
                                        )
                            }}
                        </span>
                    </button>
                </div>
            </div>
            <div class="database-isolation">
                <div class="database-isolation-copy">
                    <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.appSettingsUpdatesDatabaseIsolationTitle) }}
                    </h3>
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.appSettingsUpdatesDatabaseIsolationDescription) }}
                    </p>
                    <div class="database-path">
                        <span>{{
                            formatMessage(
                                messages.appSettingsUpdatesDatabaseIsolationCurrentDatabase,
                            )
                        }}</span>
                        <code>{{ currentDatabasePath || '—' }}</code>
                    </div>
                    <div class="database-operation">
                        <span>{{
                            formatMessage(messages.appSettingsUpdatesDatabaseOperationLabel)
                        }}</span>
                        <div class="database-operation-buttons">
                            <Button
                                type="outlined"
                                :disabled="activeChannel === 'beta'"
                                @click="selectDatabaseOperation('release-to-beta')"
                                ><DatabaseIcon />
                                {{
                                    formatMessage(
                                        messages.appSettingsUpdatesDatabaseOperationReleaseToBeta,
                                    )
                                }}
                            </Button>
                            <Button
                                type="outlined"
                                :disabled="activeChannel === 'release'"
                                @click="selectDatabaseOperation('beta-to-release')"
                                ><DatabaseIcon />
                                {{
                                    formatMessage(
                                        messages.appSettingsUpdatesDatabaseOperationBetaToRelease,
                                    )
                                }}
                            </Button>
                        </div>
                    </div>
                </div>
                <div
                    class="database-diagram"
                    :aria-label="
                        formatMessage(messages.appSettingsUpdatesDatabaseIsolationDescription)
                    "
                >
                    <div class="database-channel database-channel-release">
                        <span>R</span>
                        <strong>{{
                            formatMessage(messages.appSettingsUpdatesDatabaseIsolationRelease)
                        }}</strong>
                        <small>{{
                            activeChannel === 'release'
                                ? formatMessage(messages.appSettingsUpdatesDatabaseIsolationActive)
                                : '\u00a0'
                        }}</small>
                    </div>
                    <div class="database-branch" aria-hidden="true"></div>
                    <div class="database-channel database-channel-beta">
                        <span>B</span>
                        <strong>{{
                            formatMessage(messages.appSettingsUpdatesDatabaseIsolationBeta)
                        }}</strong>
                        <small>{{
                            activeChannel === 'beta'
                                ? formatMessage(messages.appSettingsUpdatesDatabaseIsolationActive)
                                : '\u00a0'
                        }}</small>
                    </div>
                </div>
            </div>
        </SettingsSection>

        <SettingsSection :title="formatMessage(messages.appSettingsUpdatesPreferencesTitle)">
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appSettingsUpdatesImmediateFetch)
                }}</template>
                <template #description>
                    {{
                        selectedChannel === 'beta'
                            ? formatMessage(
                                  messages.appSettingsUpdatesImmediateFetchBetaDescription,
                              )
                            : formatMessage(messages.appSettingsUpdatesImmediateFetchDescription)
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
                <template #label>{{ formatMessage(messages.appSettingsUpdatesPause) }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsUpdatesPauseDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="pause-updates"
                        v-model="updatePreferences.updatesPaused"
                        @update:model-value="saveUpdatePreferences"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection :title="formatMessage(messages.appSettingsUpdatesCheckTitle)">
            <div class="update-check-panel">
                <div class="update-check-heading">
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(messages.appSettingsUpdatesCurrentVersion, {
                                version: currentVersion,
                            })
                        }}
                    </p>
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.appSettingsUpdatesSecurity) }}
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
                        {{
                            formatMessage(
                                checking
                                    ? messages.appSettingsUpdatesChecking
                                    : messages.appSettingsUpdatesCheck,
                            )
                        }}
                    </Button>
                    <Button
                        v-if="isDevEnvironment && previewUpdateAnnouncement"
                        type="outlined"
                        native-type="button"
                        @click="previewUpdateAnnouncement(currentVersion)"
                    >
                        <EyeIcon />
                        {{ formatMessage(messages.appSettingsUpdatesPreviewAnnouncement) }}
                    </Button>
                </div>
                <p
                    v-if="checkResult"
                    class="update-check-result"
                    :class="`update-check-result-${checkResult}`"
                    role="status"
                >
                    {{ formatMessage(resultMessages[checkResult]) }}
                </p>
            </div>
        </SettingsSection>

        <UpdateAnnouncementHistory :current-version="currentVersion" />
    </div>

    <PreviewModal
        ref="restartModal"
        :header="formatMessage(messages.appSettingsUpdatesChannelRestartTitle)"
        :closable="true"
        :close-on-click-outside="true"
    >
        <p class="m-0">
            {{
                formatMessage(
                    isDevEnvironment
                        ? messages.appSettingsUpdatesChannelRestartDevelopmentDescription
                        : messages.appSettingsUpdatesChannelRestartDescription,
                )
            }}
        </p>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="restartModal?.hide()"
                    ><XIcon />
                    {{ formatMessage(messages.appSettingsUpdatesChannelRestartLater) }}
                </Button>
                <Button
                    v-if="!isDevEnvironment"
                    type="colored"
                    color="brand"
                    @click="restartForChannelChange"
                    ><RefreshCwIcon />
                    {{ formatMessage(messages.appSettingsUpdatesChannelRestartNow) }}
                </Button>
            </div>
        </template>
    </PreviewModal>

    <PreviewModal
        ref="copyDatabaseModal"
        :header="formatMessage(messages.appSettingsUpdatesChannelCopyDatabaseTitle)"
        :closable="true"
        :close-on-click-outside="true"
        :on-hide="onCopyDatabaseModalHide"
    >
        <p class="m-0">
            {{ formatMessage(messages.appSettingsUpdatesChannelCopyDatabaseDescription) }}
        </p>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="chooseBetaDatabase(false)"
                    ><XIcon />
                    {{ formatMessage(messages.appSettingsUpdatesChannelStartEmpty) }}
                </Button>
                <Button type="colored" color="brand" @click="chooseBetaDatabase(true)"
                    ><RefreshCwIcon />
                    {{ formatMessage(messages.appSettingsUpdatesChannelCopyDatabase) }}
                </Button>
            </div>
        </template>
    </PreviewModal>

    <PreviewModal
        ref="databaseOperationModal"
        :header="formatMessage(messages.appSettingsUpdatesDatabaseOperationConfirmTitle)"
        :closable="true"
        :close-on-click-outside="true"
        :on-hide="onDatabaseOperationModalHide"
    >
        <p class="m-0">
            {{
                formatMessage(messages.appSettingsUpdatesDatabaseOperationConfirmDescription, {
                    source:
                        databaseOperation === 'release-to-beta'
                            ? formatMessage(messages.appSettingsUpdatesDatabaseIsolationRelease)
                            : formatMessage(messages.appSettingsUpdatesDatabaseIsolationBeta),
                    target:
                        databaseOperation === 'release-to-beta'
                            ? formatMessage(messages.appSettingsUpdatesDatabaseIsolationBeta)
                            : formatMessage(messages.appSettingsUpdatesDatabaseIsolationRelease),
                })
            }}
        </p>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button
                    type="outlined"
                    :disabled="databaseOperationBusy"
                    @click="cancelDatabaseOperation"
                    ><XIcon />
                    {{ formatMessage(messages.appSettingsUpdatesDatabaseOperationCancel) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    :disabled="databaseOperationBusy"
                    @click="confirmDatabaseOperation"
                    ><RefreshCwIcon :class="{ 'animate-spin': databaseOperationBusy }" />
                    {{ formatMessage(messages.appSettingsUpdatesDatabaseOperationConfirm) }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>

<style scoped>
.update-check-panel {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--gap-md);
    padding: var(--gap-lg);
}

.database-isolation {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 20rem), 1fr));
    gap: var(--gap-lg);
    margin: 0 var(--gap-lg) var(--gap-lg);
    padding: var(--gap-md);
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-md);
    background: var(--surface-1);
}

.database-isolation-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: var(--gap-sm);
}

.database-path {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: var(--gap-xs);
    color: var(--color-text-tertiary);
    font-size: 0.9375rem;
}

.database-path code {
    overflow-wrap: anywhere;
    color: var(--color-text-primary);
    font-family: var(--mono-font, monospace);
}

.database-operation {
    display: flex;
    flex-direction: column;
    gap: var(--gap-xs);
    color: var(--color-text-tertiary);
    font-size: 0.9375rem;
}

.database-operation-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: var(--gap-sm);
}

.database-diagram {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: var(--gap-sm);
    min-width: 0;
}

.database-channel {
    display: flex;
    min-width: 0;
    flex-direction: column;
    align-items: center;
    gap: 0.2rem;
    padding: var(--gap-sm);
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    color: var(--color-text-primary);
    text-align: center;
}

.database-channel > span {
    display: grid;
    width: 2rem;
    height: 2rem;
    place-items: center;
    border-radius: 50%;
    background: var(--color-brand);
    color: var(--color-button-text);
    font-weight: 700;
}

.database-channel small {
    color: var(--color-brand);
    font-size: 0.7rem;
    font-weight: 600;
}

.database-channel-beta > span {
    background: var(--color-purple, var(--color-brand));
}

.database-branch {
    width: 2.5rem;
    height: 1px;
    background: var(--surface-5);
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
