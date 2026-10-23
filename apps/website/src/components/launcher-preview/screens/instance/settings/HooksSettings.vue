<script setup lang="ts">
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref, watch } from 'vue'

import { usePreview } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'
import { hooksSettingsMessages as messages } from '../messages'
import { injectInstanceSettings } from './context'
import { useGlobalSettings, useInstanceSettingsState, useSimulatedInvoke } from './state'

/** Port of `apps/app-frontend/src/components/ui/instance_settings/HooksSettings.vue`. */
type Hooks = GameInstance['hooks']

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const preview = usePreview()
const settingsState = useInstanceSettingsState()
const simulate = useSimulatedInvoke()

const { instance } = injectInstanceSettings()

const { settings: globalSettings } = useGlobalSettings()
const extras = settingsState.extrasFor(instance.value.id)

const overrideHooks = ref(
    !!instance.value.hooks.pre_launch ||
        !!instance.value.hooks.wrapper ||
        !!instance.value.hooks.post_exit,
)
const hooks = ref<Hooks>(instance.value.hooks ?? globalSettings.hooks)
const overrideLaunchPreparationTimeout = ref(extras.launch_preparation_timeout != null)
const launchPreparationTimeout = ref(
    Math.min(600, Math.max(30, extras.launch_preparation_timeout ?? 60)),
)

const editInstanceObject = computed(() => {
    const editInstancePatch: {
        hooks?: Hooks
        launch_preparation_timeout?: number | null
    } = {}

    // When hooks are not overridden per-instance, we want to clear them
    editInstancePatch.hooks = overrideHooks.value ? hooks.value : {}
    editInstancePatch.launch_preparation_timeout = overrideLaunchPreparationTimeout.value
        ? Math.min(600, Math.max(30, Math.round(launchPreparationTimeout.value || 60)))
        : null

    return editInstancePatch
})

/** `edit(instanceId, patch)`. */
async function edit(
    instanceId: string,
    patch: { hooks?: Hooks; launch_preparation_timeout?: number | null },
) {
    await simulate()
    const target = preview.instances.get(instanceId)
    if (!target) throw new Error('Unknown instance')
    if (patch.hooks) target.hooks = { ...patch.hooks }
    if (patch.launch_preparation_timeout !== undefined) {
        settingsState.extrasFor(instanceId).launch_preparation_timeout =
            patch.launch_preparation_timeout
    }
}

watch(
    [overrideHooks, hooks, overrideLaunchPreparationTimeout, launchPreparationTimeout],
    async () => {
        await edit(instance.value.id, editInstanceObject.value).catch(handleError)
    },
    { deep: true },
)

function normalizeLaunchPreparationTimeout() {
    launchPreparationTimeout.value = Math.min(
        600,
        Math.max(30, Math.round(Number(launchPreparationTimeout.value) || 60)),
    )
}
</script>

<template>
    <div>
        <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.hooks) }}
        </h2>
        <Checkbox
            v-model="overrideHooks"
            :label="formatMessage(messages.customHooks)"
            class="my-2.5"
        />
        <p class="m-0">
            {{ formatMessage(messages.hooksDescription) }}
        </p>

        <h2 class="m-0 mt-6 text-lg font-semibold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.launchPreparationTimeout) }}
        </h2>
        <Checkbox
            v-model="overrideLaunchPreparationTimeout"
            :label="formatMessage(messages.customLaunchPreparationTimeout)"
            class="my-2.5"
        />
        <StyledInput
            id="launch-preparation-timeout"
            v-model="launchPreparationTimeout"
            autocomplete="off"
            :disabled="!overrideLaunchPreparationTimeout"
            type="number"
            :min="30"
            :max="600"
            :step="1"
            wrapper-class="w-full my-2.5"
            @blur="normalizeLaunchPreparationTimeout"
        />
        <p class="m-0">
            {{ formatMessage(messages.launchPreparationTimeoutDescription) }}
        </p>

        <h2 class="m-0 mt-6 text-lg font-semibold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.preLaunch) }}
        </h2>
        <StyledInput
            id="pre-launch"
            v-model="hooks.pre_launch"
            autocomplete="off"
            :disabled="!overrideHooks"
            :placeholder="formatMessage(messages.preLaunchEnter)"
            wrapper-class="w-full my-2.5"
        />
        <p class="m-0">
            {{ formatMessage(messages.preLaunchDescription) }}
        </p>

        <h2 class="m-0 mt-6 text-lg font-semibold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.wrapper) }}
        </h2>
        <StyledInput
            id="wrapper"
            v-model="hooks.wrapper"
            autocomplete="off"
            :disabled="!overrideHooks"
            :placeholder="formatMessage(messages.wrapperEnter)"
            wrapper-class="w-full my-2.5"
        />
        <p class="m-0">
            {{ formatMessage(messages.wrapperDescription) }}
        </p>

        <h2 class="m-0 mt-6 text-lg font-semibold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.postExit) }}
        </h2>
        <StyledInput
            id="post-exit"
            v-model="hooks.post_exit"
            autocomplete="off"
            :disabled="!overrideHooks"
            :placeholder="formatMessage(messages.postExitEnter)"
            wrapper-class="w-full my-2.5"
        />
        <p class="m-0">
            {{ formatMessage(messages.postExitDescription) }}
        </p>
    </div>
</template>
