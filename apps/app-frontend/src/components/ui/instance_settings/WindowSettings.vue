<script setup lang="ts">
import {
    Checkbox,
    defineMessages,
    injectNotificationManager,
    StyledInput,
    Toggle,
    useVIntl,
} from '@modrinth/ui'
import { platform } from '@tauri-apps/plugin-os'
import { type Ref, ref, watch } from 'vue'

import { edit } from '@/helpers/instance'
import { get } from '@/helpers/settings.ts'
import { injectInstanceSettings } from '@/providers/instance-settings'

import type { AppSettings, GameInstance } from '../../../helpers/types'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const { instance } = injectInstanceSettings()
const initialInstance = instance.value
const supportsMaximizeWindow = (await platform()) === 'windows'

const globalSettings = (await get().catch(handleError)) as AppSettings

const overrideWindowSettings = ref(
    initialInstance.game_resolution != null ||
        initialInstance.force_fullscreen != null ||
        initialInstance.maximize_window != null,
)
const resolution: Ref<[number, number]> = ref([
    ...(initialInstance.game_resolution ?? globalSettings.game_resolution),
] as [number, number])
const fullscreenSetting: Ref<boolean> = ref(
    initialInstance.force_fullscreen ?? globalSettings.force_fullscreen,
)
const maximizeWindowSetting = ref(initialInstance.maximize_window ?? globalSettings.maximize_window)
const windowTitle = ref(initialInstance.window_title ?? '')
let saveQueue = Promise.resolve()

watch(
    [
        overrideWindowSettings,
        () => resolution.value[0],
        () => resolution.value[1],
        fullscreenSetting,
        maximizeWindowSetting,
        windowTitle,
    ],
    (
        [override, width, height, fullscreen, maximize, title],
        [oldOverride, oldWidth, oldHeight, oldFullscreen, oldMaximize, oldTitle],
    ) => {
        const patch: Partial<GameInstance> = {}
        if (override !== oldOverride) {
            patch.force_fullscreen = override ? fullscreen : null
            patch.maximize_window = override ? maximize : null
            patch.game_resolution = override ? [width, height] : null
        } else if (override) {
            if (fullscreen !== oldFullscreen) patch.force_fullscreen = fullscreen
            if (maximize !== oldMaximize) patch.maximize_window = maximize
            if (width !== oldWidth || height !== oldHeight) patch.game_resolution = [width, height]
        }
        if (title !== oldTitle) patch.window_title = title.trim() || null
        if (Object.keys(patch).length) {
            saveQueue = saveQueue.then(() => edit(initialInstance.id, patch)).catch(handleError)
        }
    },
)

const messages = defineMessages({
    customWindowSettings: {
        id: 'instance.settings.tabs.window.custom-window-settings',
        defaultMessage: 'Custom window settings',
    },
    fullscreen: {
        id: 'instance.settings.tabs.window.fullscreen',
        defaultMessage: 'Fullscreen',
    },
    fullscreenDescription: {
        id: 'instance.settings.tabs.window.fullscreen.description',
        defaultMessage: 'Make the game start in full screen when launched (using options.txt).',
    },
    maximizeWindow: {
        id: 'instance.settings.tabs.window.maximize-window',
        defaultMessage: 'Maximize window',
    },
    maximizeWindowDescription: {
        id: 'instance.settings.tabs.window.maximize-window.description',
        defaultMessage: 'Maximize the Minecraft window when launched.',
    },
    maximizeWindowUnsupported: {
        id: 'instance.settings.tabs.window.maximize-window.unsupported',
        defaultMessage: 'Not supported on this operating system.',
    },
    width: {
        id: 'instance.settings.tabs.window.width',
        defaultMessage: 'Width',
    },
    widthDescription: {
        id: 'instance.settings.tabs.window.width.description',
        defaultMessage: 'The width of the game window when launched.',
    },
    enterWidth: {
        id: 'instance.settings.tabs.window.width.enter',
        defaultMessage: 'Enter width...',
    },
    height: {
        id: 'instance.settings.tabs.window.height',
        defaultMessage: 'Height',
    },
    heightDescription: {
        id: 'instance.settings.tabs.window.height.description',
        defaultMessage: 'The height of the game window when launched.',
    },
    enterHeight: {
        id: 'instance.settings.tabs.window.height.enter',
        defaultMessage: 'Enter height...',
    },
    windowTitle: {
        id: 'instance.settings.tabs.window.window-title',
        defaultMessage: 'Window title',
    },
    windowTitleDescription: {
        id: 'instance.settings.tabs.window.window-title.description',
        defaultMessage: 'Customize the Minecraft window title. Leave empty to keep the default.',
    },
    enterWindowTitle: {
        id: 'instance.settings.tabs.window.window-title.enter',
        defaultMessage: 'Enter window title...',
    },
})
</script>

<template>
    <div class="flex flex-col gap-6">
        <Checkbox
            v-model="overrideWindowSettings"
            :label="formatMessage(messages.customWindowSettings)"
        />
        <div class="flex items-center gap-4 justify-between">
            <div class="flex flex-col gap-1">
                <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.fullscreen) }}
                </h2>
                <p
                    class="m-0"
                    :class="{ 'text-[var(--color-text-tertiary)]': !supportsMaximizeWindow }"
                >
                    {{ formatMessage(messages.fullscreenDescription) }}
                </p>
            </div>
            <Toggle
                id="fullscreen"
                :model-value="
                    overrideWindowSettings ? fullscreenSetting : globalSettings.force_fullscreen
                "
                :disabled="!overrideWindowSettings"
                @update:model-value="
                    (e) => {
                        fullscreenSetting = e
                    }
                "
            />
        </div>
        <div class="flex items-center gap-4 justify-between">
            <div class="flex flex-col gap-1">
                <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.maximizeWindow) }}
                </h2>
                <p class="m-0">
                    {{
                        formatMessage(
                            supportsMaximizeWindow
                                ? messages.maximizeWindowDescription
                                : messages.maximizeWindowUnsupported,
                        )
                    }}
                </p>
            </div>
            <Toggle
                id="maximize-window"
                :model-value="
                    overrideWindowSettings ? maximizeWindowSetting : globalSettings.maximize_window
                "
                :disabled="!overrideWindowSettings || fullscreenSetting || !supportsMaximizeWindow"
                @update:model-value="(value) => (maximizeWindowSetting = value)"
            />
        </div>

        <div class="flex items-center gap-4 justify-between">
            <div class="flex flex-col gap-1">
                <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.width) }}
                </h2>
                <p class="m-0">
                    {{ formatMessage(messages.widthDescription) }}
                </p>
            </div>
            <StyledInput
                id="width"
                v-model="resolution[0]"
                autocomplete="off"
                :disabled="!overrideWindowSettings || fullscreenSetting"
                type="number"
                :placeholder="formatMessage(messages.enterWidth)"
            />
        </div>

        <div class="flex items-center gap-4 justify-between">
            <div class="flex flex-col gap-1">
                <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.height) }}
                </h2>
                <p class="m-0">
                    {{ formatMessage(messages.heightDescription) }}
                </p>
            </div>
            <StyledInput
                id="height"
                v-model="resolution[1]"
                autocomplete="off"
                :disabled="!overrideWindowSettings || fullscreenSetting"
                type="number"
                :placeholder="formatMessage(messages.enterHeight)"
            />
        </div>
        <div
            v-if="globalSettings.custom_window_title_enabled"
            class="flex items-center gap-4 justify-between"
        >
            <div class="flex flex-col gap-1">
                <h2
                    class="m-0 inline-flex items-center gap-2 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.windowTitle) }}
                </h2>
                <p class="m-0">
                    {{ formatMessage(messages.windowTitleDescription) }}
                </p>
            </div>
            <StyledInput
                id="window-title"
                v-model="windowTitle"
                autocomplete="off"
                type="text"
                :placeholder="formatMessage(messages.enterWindowTitle)"
            />
        </div>
    </div>
</template>
