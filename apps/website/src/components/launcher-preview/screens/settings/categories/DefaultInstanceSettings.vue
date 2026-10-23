<script setup lang="ts">
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref, watch } from 'vue'

import SettingsRow from '../components/SettingsRow.vue'
import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { PREVIEW_PLATFORM, useSettingsPage } from '../state'
import EnvironmentVariablesInput from './launch/EnvironmentVariablesInput.vue'
import LogShareSettings from './launch/LogShareSettings.vue'
import { createSettingsPatchSaver } from './launch/settings-patch'
import SharedLogsSettings from './launch/SharedLogsSettings.vue'

/**
 * Launch defaults (apps/app-frontend/src/components/ui/settings/DefaultInstanceSettings.vue):
 * the game window, environment variables, launch hooks and what the launcher
 * does once the game starts, followed by crash analysis and shared logs.
 * As in the app, the page edits a copy of the settings and saves only the
 * fields that changed; the stored values are the window's settings.
 */
const { formatMessage } = useVIntl()
const { settings: stored, simulate } = useSettingsPage()

/** The `AppSettings` fields this page edits. */
interface LaunchDefaults {
    force_fullscreen: boolean
    maximize_window: boolean
    game_resolution: [number, number]
    custom_window_title_enabled: boolean
    default_window_title: string
    custom_env_vars: [string, string][]
    hooks: { pre_launch: string | null; wrapper: string | null; post_exit: string | null }
    hide_on_process_start: boolean
    enter_lightweight_mode_on_game_launch: boolean
    custom_dir: string | null
}

/** `get()` (plugin:settings|settings_get): a copy of the stored settings. */
async function get(): Promise<LaunchDefaults> {
    await simulate(40)
    return JSON.parse(
        JSON.stringify({
            force_fullscreen: stored.force_fullscreen,
            maximize_window: stored.maximize_window,
            game_resolution: stored.game_resolution,
            custom_window_title_enabled: stored.custom_window_title_enabled,
            default_window_title: stored.default_window_title,
            custom_env_vars: stored.custom_env_vars,
            hooks: stored.hooks,
            hide_on_process_start: stored.hide_on_process_start,
            enter_lightweight_mode_on_game_launch: stored.enter_lightweight_mode_on_game_launch,
            custom_dir: stored.custom_dir,
        }),
    ) as LaunchDefaults
}

function isWindowSize(value: unknown): value is [number, number] {
    return (
        Array.isArray(value) &&
        value.length === 2 &&
        value.every((side) => Number.isInteger(side) && side >= 0 && side <= 65535)
    )
}

/**
 * `update(patch)` (plugin:settings|settings_patch): merges the edited fields
 * into the stored settings. A size that is not two `u16` values fails to
 * deserialize, so nothing of that patch is saved; the title is stored trimmed.
 */
async function update(patch: Record<string, unknown>): Promise<void> {
    await simulate(120)
    if ('game_resolution' in patch && !isWindowSize(patch.game_resolution)) {
        throw new Error('invalid type: null, expected u16')
    }
    const {
        hooks,
        default_window_title: title,
        game_resolution: size,
        ...rest
    } = patch as Partial<Record<keyof LaunchDefaults, unknown>>
    Object.assign(stored, rest)
    if (size) stored.game_resolution = [...(size as [number, number])]
    if (typeof title === 'string') stored.default_window_title = title.trim()
    if (hooks) Object.assign(stored.hooks, hooks)
}

const fetchSettings = await get()
const saveDraft = createSettingsPatchSaver(fetchSettings, update)
const supportsMaximizeWindow = (PREVIEW_PLATFORM as string) === 'windows'
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

        // A rejected save leaves the stored value as it was, without a message.
        await saveDraft(setSettings).catch(() => {})
    },
    { deep: true },
)
</script>

<template>
    <div class="flex flex-col gap-6">
        <SettingsSection
            :title="formatMessage(messages.appSettingsDefaultsSectionsWindowBehavior)"
            title-id="settings-target-defaults-window"
        >
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appSettingsDefaultsFullscreen)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsDefaultsFullscreenDescription)
                }}</template>
                <template #control
                    ><Toggle id="fullscreen" v-model="settings.force_fullscreen"
                /></template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appSettingsDefaultsMaximizeWindow)
                }}</template>
                <template #description>
                    <span :class="{ 'text-[var(--color-text-tertiary)]': !supportsMaximizeWindow }">
                        {{
                            formatMessage(
                                supportsMaximizeWindow
                                    ? messages.appSettingsDefaultsMaximizeWindowDescription
                                    : messages.appSettingsDefaultsMaximizeWindowUnsupported,
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
                <template #label>{{ formatMessage(messages.appSettingsDefaultsWidth) }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsDefaultsWidthDescription)
                }}</template>
                <template #control>
                    <StyledInput
                        id="width"
                        v-model="settings.game_resolution[0]"
                        :disabled="settings.force_fullscreen"
                        autocomplete="off"
                        type="number"
                        :placeholder="formatMessage(messages.appSettingsDefaultsWidthPlaceholder)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.appSettingsDefaultsHeight) }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsDefaultsHeightDescription)
                }}</template>
                <template #control>
                    <StyledInput
                        id="height"
                        v-model="settings.game_resolution[1]"
                        :disabled="settings.force_fullscreen"
                        autocomplete="off"
                        type="number"
                        :placeholder="formatMessage(messages.appSettingsDefaultsHeightPlaceholder)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-defaults-window-title" tabindex="-1">
                        {{ formatMessage(messages.appSettingsDefaultsWindowTitleEnabled) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsDefaultsWindowTitleEnabledDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="custom-window-title-enabled"
                        v-model="settings.custom_window_title_enabled"
                    />
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsDefaultsWindowTitleDefault)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsDefaultsWindowTitleDefaultDescription)
                }}</template>
                <template #control>
                    <StyledInput
                        id="default-window-title"
                        v-model="settings.default_window_title"
                        :disabled="!settings.custom_window_title_enabled"
                        autocomplete="off"
                        type="text"
                        :placeholder="
                            formatMessage(messages.appSettingsDefaultsWindowTitleDefaultPlaceholder)
                        "
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            :title="formatMessage(messages.appSettingsDefaultsSectionsEnvironment)"
            title-id="settings-target-defaults-environment"
        >
            <SettingsRow stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsDefaultsEnvironmentVariables)
                }}</template>
                <template #control>
                    <EnvironmentVariablesInput
                        id="env-vars"
                        v-model="settings.custom_env_vars"
                        autocomplete="off"
                        type="text"
                        :placeholder="
                            formatMessage(
                                messages.appSettingsDefaultsEnvironmentVariablesPlaceholder,
                            )
                        "
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            :title="formatMessage(messages.appSettingsDefaultsSectionsLaunchHooks)"
            title-id="settings-target-defaults-launch-hooks"
        >
            <SettingsRow stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsDefaultsPreLaunchHook)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsDefaultsPreLaunchDescription)
                }}</template>
                <template #control>
                    <StyledInput
                        id="pre-launch"
                        v-model="settings.hooks.pre_launch"
                        autocomplete="off"
                        type="text"
                        :placeholder="
                            formatMessage(messages.appSettingsDefaultsPreLaunchPlaceholder)
                        "
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsDefaultsWrapperHook)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsDefaultsWrapperDescription)
                }}</template>
                <template #control>
                    <StyledInput
                        id="wrapper"
                        v-model="settings.hooks.wrapper"
                        autocomplete="off"
                        type="text"
                        :placeholder="formatMessage(messages.appSettingsDefaultsWrapperPlaceholder)"
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsDefaultsPostExitHook)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsDefaultsPostExitDescription)
                }}</template>
                <template #control>
                    <StyledInput
                        id="post-exit"
                        v-model="settings.hooks.post_exit"
                        autocomplete="off"
                        type="text"
                        :placeholder="
                            formatMessage(messages.appSettingsDefaultsPostExitPlaceholder)
                        "
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            :title="formatMessage(messages.appSettingsDefaultsSectionsAfterLaunch)"
            title-id="settings-target-defaults-after-launch"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-launch-lightweight-mode" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsLightweightModeTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsLightweightModeDescription)
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
                        {{ formatMessage(messages.appAppearanceSettingsMinimizeLauncherTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsMinimizeLauncherDescription)
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
