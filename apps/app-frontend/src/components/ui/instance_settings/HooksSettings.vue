<script setup lang="ts">
import {
    Checkbox,
    defineMessages,
    injectNotificationManager,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import { edit } from '@/helpers/instance'
import { get } from '@/helpers/settings.ts'
import { injectInstanceSettings } from '@/providers/instance-settings'

import type { Hooks } from '../../../helpers/types'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const { instance } = injectInstanceSettings()

const globalSettings = await get().catch((error) => {
    handleError(error)
    throw error
})

const overrideHooks = ref(
    !!instance.value.hooks.pre_launch ||
        !!instance.value.hooks.wrapper ||
        !!instance.value.hooks.post_exit,
)
const hooks = ref(instance.value.hooks ?? globalSettings.hooks)
const overrideLaunchPreparationTimeout = ref(instance.value.launch_preparation_timeout != null)
const launchPreparationTimeout = ref(
    Math.min(600, Math.max(30, instance.value.launch_preparation_timeout ?? 60)),
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

watch(
    [overrideHooks, hooks, overrideLaunchPreparationTimeout, launchPreparationTimeout],
    async () => {
        await edit(instance.value.id, editInstanceObject.value)
    },
    { deep: true },
)
const messages = defineMessages({
    hooks: {
        id: 'instance.settings.tabs.hooks.title',
        defaultMessage: 'Launch preparation',
    },
    hooksDescription: {
        id: 'instance.settings.tabs.hooks.description',
        defaultMessage:
            'Configure the time allowed for launch preparation and optional commands that run before and after the game.',
    },
    launchPreparationTimeout: {
        id: 'instance.settings.tabs.hooks.launch-preparation-timeout',
        defaultMessage: 'Launch preparation timeout',
    },
    launchPreparationTimeoutDescription: {
        id: 'instance.settings.tabs.hooks.launch-preparation-timeout.description',
        defaultMessage:
            'Maximum time to wait for launch preparation to finish, in seconds (30–600).',
    },
    customLaunchPreparationTimeout: {
        id: 'instance.settings.tabs.hooks.custom-launch-preparation-timeout',
        defaultMessage: 'Use a custom launch preparation timeout',
    },
    customHooks: {
        id: 'instance.settings.tabs.hooks.custom-hooks',
        defaultMessage: 'Custom launch hooks',
    },
    preLaunch: {
        id: 'instance.settings.tabs.hooks.pre-launch',
        defaultMessage: 'Pre-launch',
    },
    preLaunchDescription: {
        id: 'instance.settings.tabs.hooks.pre-launch.description',
        defaultMessage: 'Ran before the instance is launched.',
    },
    preLaunchEnter: {
        id: 'instance.settings.tabs.hooks.pre-launch.enter',
        defaultMessage: 'Enter pre-launch command...',
    },
    wrapper: {
        id: 'instance.settings.tabs.hooks.wrapper',
        defaultMessage: 'Wrapper',
    },
    wrapperDescription: {
        id: 'instance.settings.tabs.hooks.wrapper.description',
        defaultMessage: 'Wrapper command for launching Minecraft.',
    },
    wrapperEnter: {
        id: 'instance.settings.tabs.hooks.wrapper.enter',
        defaultMessage: 'Enter wrapper command...',
    },
    postExit: {
        id: 'instance.settings.tabs.hooks.post-exit',
        defaultMessage: 'Post-exit',
    },
    postExitDescription: {
        id: 'instance.settings.tabs.hooks.post-exit.description',
        defaultMessage: 'Ran after the game closes.',
    },
    postExitEnter: {
        id: 'instance.settings.tabs.hooks.post-exit.enter',
        defaultMessage: 'Enter post-exit command...',
    },
})

function normalizeLaunchPreparationTimeout() {
    launchPreparationTimeout.value = Math.min(
        600,
        Math.max(30, Math.round(Number(launchPreparationTimeout.value) || 60)),
    )
}
</script>

<template>
    <div>
        <h2 class="m-0 m-0 text-lg font-semibold text-[var(--color-text-primary)]">
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

        <h2 class="mt-6 m-0 text-lg font-semibold text-[var(--color-text-primary)]">
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

        <h2 class="mt-6 m-0 text-lg font-semibold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.preLaunch) }}
        </h2>
        <StyledInput
            id="pre-launch"
            :model-value="hooks.pre_launch ?? ''"
            autocomplete="off"
            :disabled="!overrideHooks"
            :placeholder="formatMessage(messages.preLaunchEnter)"
            wrapper-class="w-full my-2.5"
            @update:model-value="(value) => (hooks.pre_launch = String(value ?? ''))"
        />
        <p class="m-0">
            {{ formatMessage(messages.preLaunchDescription) }}
        </p>

        <h2 class="mt-6 m-0 text-lg font-semibold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.wrapper) }}
        </h2>
        <StyledInput
            id="wrapper"
            :model-value="hooks.wrapper ?? ''"
            autocomplete="off"
            :disabled="!overrideHooks"
            :placeholder="formatMessage(messages.wrapperEnter)"
            wrapper-class="w-full my-2.5"
            @update:model-value="(value) => (hooks.wrapper = String(value ?? ''))"
        />
        <p class="m-0">
            {{ formatMessage(messages.wrapperDescription) }}
        </p>

        <h2 class="mt-6 m-0 text-lg font-semibold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.postExit) }}
        </h2>
        <StyledInput
            id="post-exit"
            :model-value="hooks.post_exit ?? ''"
            autocomplete="off"
            :disabled="!overrideHooks"
            :placeholder="formatMessage(messages.postExitEnter)"
            wrapper-class="w-full my-2.5"
            @update:model-value="(value) => (hooks.post_exit = String(value ?? ''))"
        />
        <p class="m-0">
            {{ formatMessage(messages.postExitDescription) }}
        </p>
    </div>
</template>
