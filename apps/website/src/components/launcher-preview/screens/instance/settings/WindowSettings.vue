<script setup lang="ts">
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { type Ref, ref, watch } from 'vue'

import { usePreview } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'
import { windowSettingsMessages as messages } from '../messages'
import { injectInstanceSettings } from './context'
import { useGlobalSettings, useSimulatedInvoke } from './state'

/**
 * Port of `apps/app-frontend/src/components/ui/instance_settings/WindowSettings.vue`.
 * The window is the macOS build, where `platform()` is not `windows`, so the
 * maximize option is reported as unsupported.
 */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const preview = usePreview()
const simulate = useSimulatedInvoke()

const { instance } = injectInstanceSettings()
const initialInstance = instance.value
const supportsMaximizeWindow = false

const { settings: globalSettings, customWindowTitleEnabled } = useGlobalSettings()

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

/** `edit(instanceId, patch)`. */
async function edit(instanceId: string, patch: Partial<GameInstance>) {
    await simulate()
    const target = preview.instances.get(instanceId)
    if (!target) throw new Error('Unknown instance')
    Object.assign(target, patch)
}

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
</script>

<template>
    <div class="flex flex-col gap-6">
        <Checkbox
            v-model="overrideWindowSettings"
            :label="formatMessage(messages.customWindowSettings)"
        />
        <div class="flex items-center justify-between gap-4">
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
                        fullscreenSetting = !!e
                    }
                "
            />
        </div>
        <div class="flex items-center justify-between gap-4">
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
                @update:model-value="(value) => (maximizeWindowSetting = !!value)"
            />
        </div>

        <div class="flex items-center justify-between gap-4">
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

        <div class="flex items-center justify-between gap-4">
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
        <div v-if="customWindowTitleEnabled" class="flex items-center justify-between gap-4">
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
