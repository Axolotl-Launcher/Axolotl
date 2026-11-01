<script setup lang="ts">
import { TriangleAlertIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import type { Component } from 'vue'
import { computed, onErrorCaptured, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'

import LabPluginSurface from '@/components/lab/plugins/LabPluginSurface.vue'
import { getLabPluginRegistration } from '@/lab/plugins/registry'
import { recordLabPluginFailure } from '@/lab/plugins/state'
import { useLabPluginText } from '@/lab/plugins/use-lab-plugin-text'

const props = withDefaults(
    defineProps<{
        /** Manifest id of the plugin being rendered. */
        pluginId: string
        /** The plugin component to mount. */
        component: Component
        /** Props taken from the slot context and forwarded to the component. */
        componentProps?: Record<string, unknown>
        /** `page` shows a full explanation, `inline` a one-line notice. */
        variant?: 'inline' | 'page'
    }>(),
    { variant: 'inline' },
)

const { formatMessage } = useVIntl()
const resolveText = useLabPluginText()

/** Error message from the plugin, or null while it is healthy. */
const failure = ref<string | null>(null)

const pluginName = computed(() => {
    const registration = getLabPluginRegistration(props.pluginId)
    return registration ? resolveText(registration.plugin.manifest.name) : props.pluginId
})

watch(
    () => props.pluginId,
    () => {
        failure.value = null
    },
)

onErrorCaptured((error) => {
    const message = error instanceof Error ? error.message : String(error)
    failure.value = message
    // Persisting touches shared reactive state; defer it out of the current
    // error handling pass so the swap to the fallback is not interrupted.
    queueMicrotask(() => recordLabPluginFailure(props.pluginId, message))
    return false
})

const messages = defineMessages({
    inline: {
        id: 'app.lab.plugins.crashed.inline',
        defaultMessage: '{name} stopped working',
    },
    title: {
        id: 'app.lab.plugins.crashed.title',
        defaultMessage: 'This plugin stopped working',
    },
    description: {
        id: 'app.lab.plugins.crashed.description',
        defaultMessage: '{name} was turned off to keep this page working.',
    },
    manage: {
        id: 'app.lab.plugins.crashed.manage',
        defaultMessage: 'Open plugins',
    },
})
</script>

<template>
    <span
        v-if="failure && variant === 'inline'"
        v-tooltip="failure ?? undefined"
        class="inline-flex items-center gap-1.5 text-xs text-[var(--color-text-tertiary)]"
    >
        <TriangleAlertIcon class="size-3.5 shrink-0" />
        {{ formatMessage(messages.inline, { name: pluginName }) }}
    </span>

    <section
        v-else-if="failure"
        class="flex flex-col gap-2 rounded-[var(--radius-lg)] border border-surface-5 bg-surface-2 p-4"
    >
        <h2
            class="m-0 flex items-center gap-2 text-base font-bold text-[var(--color-text-primary)]"
        >
            <TriangleAlertIcon class="size-4 shrink-0" />
            {{ formatMessage(messages.title) }}
        </h2>
        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.description, { name: pluginName }) }}
        </p>
        <p class="m-0 break-words text-xs text-[var(--color-text-tertiary)]">{{ failure }}</p>
        <RouterLink
            to="/lab/plugins"
            class="w-fit text-sm text-brand no-underline hover:underline focus-visible:outline-none"
        >
            {{ formatMessage(messages.manage) }}
        </RouterLink>
    </section>

    <LabPluginSurface
        v-else
        :plugin-id="pluginId"
        :component="component"
        :component-props="componentProps"
    />
</template>
