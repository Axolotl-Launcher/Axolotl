<script setup lang="ts">
import { defineMessages, StyledInput, Toggle, useVIntl } from '@modrinth/ui'
import { platform } from '@tauri-apps/plugin-os'
import { ref, watch } from 'vue'

import EnvironmentVariablesInput from '@/components/ui/EnvironmentVariablesInput.vue'
import { get, update } from '@/helpers/settings.ts'
import { createSettingsPatchSaver } from '@/helpers/settings-patch'

import LogShareSettings from './LogShareSettings.vue'
import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'
import SharedLogsSettings from './SharedLogsSettings.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    windowBehaviorSectionTitle: {
        id: 'app.settings.defaults.sections.window-behavior',
        defaultMessage: 'Window behavior',
    },
    environmentSectionTitle: {
        id: 'app.settings.defaults.sections.environment',
        defaultMessage: 'Environment variables',
    },
    launchHooksSectionTitle: {
        id: 'app.settings.defaults.sections.launch-hooks',
        defaultMessage: 'Launch hooks',
    },
    afterLaunchSectionTitle: {
        id: 'app.settings.defaults.sections.after-launch',
        defaultMessage: 'After launch behavior',
    },
    fullscreen: { id: 'app.settings.defaults.fullscreen', defaultMessage: 'Fullscreen' },
    fullscreenDescription: {
        id: 'app.settings.defaults.fullscreen-description',
        defaultMessage: 'Overwrites the options.txt file to start in full screen when launched.',
    },
    maximizeWindow: {
        id: 'app.settings.defaults.maximize-window',
        defaultMessage: 'Maximize window',
    },
    maximizeWindowDescription: {
        id: 'app.settings.defaults.maximize-window-description',
        defaultMessage: 'Maximize the Minecraft window when it starts.',
    },
    maximizeWindowUnsupported: {
        id: 'app.settings.defaults.maximize-window.unsupported',
        defaultMessage: 'Not supported on this operating system.',
    },
    windowTitleEnabled: {
        id: 'app.settings.defaults.window-title.enabled',
        defaultMessage: 'Custom Minecraft window title',
    },
    windowTitleEnabledDescription: {
        id: 'app.settings.defaults.window-title.enabled-description',
        defaultMessage: 'Allow instances to customize the Minecraft window title.',
    },
    defaultWindowTitle: {
        id: 'app.settings.defaults.window-title.default',
        defaultMessage: 'Default window title',
    },
    defaultWindowTitleDescription: {
        id: 'app.settings.defaults.window-title.default-description',
        defaultMessage: 'Used when an instance does not provide its own title.',
    },
    defaultWindowTitlePlaceholder: {
        id: 'app.settings.defaults.window-title.default-placeholder',
        defaultMessage: 'Enter the default window title...',
    },
    width: { id: 'app.settings.defaults.width', defaultMessage: 'Width' },
    widthDescription: {
        id: 'app.settings.defaults.width-description',
        defaultMessage: 'The width of the game window when launched.',
    },
    widthPlaceholder: {
        id: 'app.settings.defaults.width-placeholder',
        defaultMessage: 'Enter width...',
    },
    height: { id: 'app.settings.defaults.height', defaultMessage: 'Height' },
    heightDescription: {
        id: 'app.settings.defaults.height-description',
        defaultMessage: 'The height of the game window when launched.',
    },
    heightPlaceholder: {
        id: 'app.settings.defaults.height-placeholder',
        defaultMessage: 'Enter height...',
    },
    environmentVariables: {
        id: 'app.settings.defaults.environment-variables',
        defaultMessage: 'Environment variables',
    },
    environmentVariablesPlaceholder: {
        id: 'app.settings.defaults.environment-variables-placeholder',
        defaultMessage: 'Enter environment variables...',
    },
    preLaunchHook: {
        id: 'app.settings.defaults.pre-launch-hook',
        defaultMessage: 'Pre-launch hook',
    },
    preLaunchPlaceholder: {
        id: 'app.settings.defaults.pre-launch-placeholder',
        defaultMessage: 'Enter pre-launch command...',
    },
    preLaunchDescription: {
        id: 'app.settings.defaults.pre-launch-description',
        defaultMessage: 'Run before the instance is launched.',
    },
    wrapperHook: { id: 'app.settings.defaults.wrapper-hook', defaultMessage: 'Wrapper hook' },
    wrapperPlaceholder: {
        id: 'app.settings.defaults.wrapper-placeholder',
        defaultMessage: 'Enter wrapper command...',
    },
    wrapperDescription: {
        id: 'app.settings.defaults.wrapper-description',
        defaultMessage: 'Wrapper command for launching Minecraft.',
    },
    postExitHook: { id: 'app.settings.defaults.post-exit-hook', defaultMessage: 'Post-exit hook' },
    postExitPlaceholder: {
        id: 'app.settings.defaults.post-exit-placeholder',
        defaultMessage: 'Enter post-exit command...',
    },
    postExitDescription: {
        id: 'app.settings.defaults.post-exit-description',
        defaultMessage: 'Run after the game closes.',
    },
    lightweightMode: {
        id: 'app.appearance-settings.lightweight-mode.title',
        defaultMessage: 'Enter lightweight mode after launching a game',
    },
    lightweightModeDescription: {
        id: 'app.appearance-settings.lightweight-mode.description',
        defaultMessage:
            'Closes the launcher webview after Minecraft starts to reduce memory use. Restore it from the system tray.',
    },
    minimizeLauncher: {
        id: 'app.appearance-settings.minimize-launcher.title',
        defaultMessage: 'Minimize launcher',
    },
    minimizeLauncherDescription: {
        id: 'app.appearance-settings.minimize-launcher.description',
        defaultMessage: 'Minimize the launcher when a Minecraft process starts.',
    },
})

const fetchSettings = await get()
const saveDraft = createSettingsPatchSaver(fetchSettings, update)
const supportsMaximizeWindow = (await platform()) === 'windows'
const settings = ref(fetchSettings)

watch(
    settings,
    async () => {
        const setSettings = JSON.parse(JSON.stringify(settings.value))

        if (!setSettings.hooks.pre_launch) {
            setSettings.hooks.pre_launch = null
        }
        if (!setSettings.hooks.wrapper) {
            setSettings.hooks.wrapper = null
        }
        if (!setSettings.hooks.post_exit) {
            setSettings.hooks.post_exit = null
        }

        if (!setSettings.custom_dir) {
            setSettings.custom_dir = null
        }

        await saveDraft(setSettings)
    },
    { deep: true },
)
</script>

<template>
    <div class="flex flex-col gap-6">
        <SettingsSection
            :title="formatMessage(messages.windowBehaviorSectionTitle)"
            title-id="settings-target-defaults-window"
        >
            <SettingsRow>
                <template #label>{{ formatMessage(messages.fullscreen) }}</template>
                <template #description>{{
                    formatMessage(messages.fullscreenDescription)
                }}</template>
                <template #control
                    ><Toggle id="fullscreen" v-model="settings.force_fullscreen"
                /></template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.maximizeWindow) }}</template>
                <template #description>
                    <span :class="{ 'text-[var(--color-text-tertiary)]': !supportsMaximizeWindow }">
                        {{
                            formatMessage(
                                supportsMaximizeWindow
                                    ? messages.maximizeWindowDescription
                                    : messages.maximizeWindowUnsupported,
                            )
                        }}
                    </span>
                </template>
                <template #control>
                    <Toggle
                        id="maximize-window"
                        v-model="settings.maximize_window"
                        :disabled="settings.force_fullscreen || !supportsMaximizeWindow"
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.width) }}</template>
                <template #description>{{ formatMessage(messages.widthDescription) }}</template>
                <template #control>
                    <StyledInput
                        id="width"
                        v-model="settings.game_resolution[0]"
                        :disabled="settings.force_fullscreen"
                        autocomplete="off"
                        type="number"
                        :placeholder="formatMessage(messages.widthPlaceholder)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.height) }}</template>
                <template #description>{{ formatMessage(messages.heightDescription) }}</template>
                <template #control>
                    <StyledInput
                        id="height"
                        v-model="settings.game_resolution[1]"
                        :disabled="settings.force_fullscreen"
                        autocomplete="off"
                        type="number"
                        :placeholder="formatMessage(messages.heightPlaceholder)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-defaults-window-title" tabindex="-1">
                        {{ formatMessage(messages.windowTitleEnabled) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.windowTitleEnabledDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="custom-window-title-enabled"
                        v-model="settings.custom_window_title_enabled"
                    />
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>{{ formatMessage(messages.defaultWindowTitle) }}</template>
                <template #description>{{
                    formatMessage(messages.defaultWindowTitleDescription)
                }}</template>
                <template #control>
                    <StyledInput
                        id="default-window-title"
                        v-model="settings.default_window_title"
                        :disabled="!settings.custom_window_title_enabled"
                        autocomplete="off"
                        type="text"
                        :placeholder="formatMessage(messages.defaultWindowTitlePlaceholder)"
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            :title="formatMessage(messages.environmentSectionTitle)"
            title-id="settings-target-defaults-environment"
        >
            <SettingsRow stacked>
                <template #label>{{ formatMessage(messages.environmentVariables) }}</template>
                <template #control>
                    <EnvironmentVariablesInput
                        id="env-vars"
                        v-model="settings.custom_env_vars"
                        autocomplete="off"
                        type="text"
                        :placeholder="formatMessage(messages.environmentVariablesPlaceholder)"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            :title="formatMessage(messages.launchHooksSectionTitle)"
            title-id="settings-target-defaults-launch-hooks"
        >
            <SettingsRow stacked>
                <template #label>{{ formatMessage(messages.preLaunchHook) }}</template>
                <template #description>{{ formatMessage(messages.preLaunchDescription) }}</template>
                <template #control>
                    <StyledInput
                        id="pre-launch"
                        v-model="settings.hooks.pre_launch"
                        autocomplete="off"
                        type="text"
                        :placeholder="formatMessage(messages.preLaunchPlaceholder)"
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>{{ formatMessage(messages.wrapperHook) }}</template>
                <template #description>{{ formatMessage(messages.wrapperDescription) }}</template>
                <template #control>
                    <StyledInput
                        id="wrapper"
                        v-model="settings.hooks.wrapper"
                        autocomplete="off"
                        type="text"
                        :placeholder="formatMessage(messages.wrapperPlaceholder)"
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>{{ formatMessage(messages.postExitHook) }}</template>
                <template #description>{{ formatMessage(messages.postExitDescription) }}</template>
                <template #control>
                    <StyledInput
                        id="post-exit"
                        v-model="settings.hooks.post_exit"
                        autocomplete="off"
                        type="text"
                        :placeholder="formatMessage(messages.postExitPlaceholder)"
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            :title="formatMessage(messages.afterLaunchSectionTitle)"
            title-id="settings-target-defaults-after-launch"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-launch-lightweight-mode" tabindex="-1">
                        {{ formatMessage(messages.lightweightMode) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.lightweightModeDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="enter-lightweight-mode-on-game-launch"
                        :model-value="settings.enter_lightweight_mode_on_game_launch"
                        @update:model-value="
                            (value) => {
                                settings.enter_lightweight_mode_on_game_launch = !!value
                                if (value) settings.hide_on_process_start = false
                            }
                        "
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-launch-minimize" tabindex="-1">
                        {{ formatMessage(messages.minimizeLauncher) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.minimizeLauncherDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="minimize-launcher"
                        :model-value="settings.hide_on_process_start"
                        :disabled="settings.enter_lightweight_mode_on_game_launch"
                        @update:model-value="(value) => (settings.hide_on_process_start = !!value)"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <LogShareSettings />
        <SharedLogsSettings />
    </div>
</template>
