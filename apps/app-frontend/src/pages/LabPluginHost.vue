<script setup lang="ts">
import { PlugIcon } from '@modrinth/assets'
import { Button, defineMessages, EmptyState, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import LabPluginBoundary from '@/components/lab/plugins/LabPluginBoundary.vue'
import { getLabPluginRegistration } from '@/lab/plugins/registry'
import {
    getLabPluginFailure,
    isLabPluginEnabled,
    retryLabPlugin,
    setLabPluginEnabled,
} from '@/lab/plugins/state'
import { useLabPluginText } from '@/lab/plugins/use-lab-plugin-text'

const route = useRoute()
const router = useRouter()
const { formatMessage } = useVIntl()
const resolveText = useLabPluginText()

const messages = defineMessages({
    missingTitle: { id: 'app.lab.plugins.missing-title', defaultMessage: 'Plugin not found' },
    missingDescription: {
        id: 'app.lab.plugins.missing-description',
        defaultMessage: 'This plugin is not part of this launcher build.',
    },
    backToLab: { id: 'app.lab.plugins.back-to-lab', defaultMessage: 'Back to the lab' },
    offTitle: { id: 'app.lab.plugins.off-title', defaultMessage: 'This plugin is turned off' },
    offDescription: {
        id: 'app.lab.plugins.off-description',
        defaultMessage: 'Turn it back on to use it. Plugins stay off until you do.',
    },
    turnOn: { id: 'app.lab.plugins.turn-on', defaultMessage: 'Turn on' },
    failedHint: {
        id: 'app.lab.plugins.failed-hint',
        defaultMessage: 'It was turned off after an error:',
    },
    noToolTitle: {
        id: 'app.lab.plugins.no-tool-title',
        defaultMessage: 'This plugin has no page',
    },
    noToolDescription: {
        id: 'app.lab.plugins.no-tool-description',
        defaultMessage:
            'It adds itself to the launcher elsewhere, for example the instance action row.',
    },
})

const pluginId = computed(() => String(route.params.pluginId ?? ''))
const registration = computed(() => getLabPluginRegistration(pluginId.value))
const manifest = computed(() => registration.value?.plugin.manifest)
const tool = computed(() => registration.value?.plugin.tool)
const enabled = computed(() => isLabPluginEnabled(pluginId.value))
const failure = computed(() => getLabPluginFailure(pluginId.value))

function turnOn() {
    if (failure.value) retryLabPlugin(pluginId.value)
    else setLabPluginEnabled(pluginId.value, true)
}

const title = computed(() => (manifest.value ? resolveText(manifest.value.name) : ''))
</script>

<template>
    <main class="mx-auto flex w-full max-w-[90rem] flex-col gap-5 p-6">
        <EmptyState
            v-if="!registration"
            type="error"
            :heading="formatMessage(messages.missingTitle)"
            :description="formatMessage(messages.missingDescription)"
        >
            <template #actions>
                <Button type="outlined" @click="router.push('/lab')">
                    {{ formatMessage(messages.backToLab) }}
                </Button>
            </template>
        </EmptyState>

        <EmptyState v-else-if="!enabled" type="warning" :heading="formatMessage(messages.offTitle)">
            <template #description>
                <span>{{ formatMessage(messages.offDescription) }}</span>
                <span v-if="failure" class="mt-2 block text-xs">
                    {{ formatMessage(messages.failedHint) }} {{ failure }}
                </span>
            </template>
            <template #actions>
                <Button type="colored" color="brand" @click="turnOn">
                    {{ formatMessage(messages.turnOn) }}
                </Button>
            </template>
        </EmptyState>

        <template v-else>
            <header class="flex min-w-0 flex-wrap items-center justify-between gap-3">
                <div class="flex min-w-0 items-center gap-3">
                    <div
                        class="flex size-9 shrink-0 items-center justify-center rounded-[var(--radius-md)] bg-brand-highlight text-brand"
                    >
                        <PlugIcon class="size-5" />
                    </div>
                    <div class="min-w-0">
                        <h1
                            class="m-0 truncate text-2xl font-bold text-[var(--color-text-primary)]"
                        >
                            {{ title }}
                        </h1>
                        <p class="m-0 text-xs text-[var(--color-text-tertiary)]">
                            {{ manifest?.version }}
                            <template v-if="manifest?.author"> · {{ manifest.author }}</template>
                        </p>
                    </div>
                </div>
            </header>

            <LabPluginBoundary
                v-if="tool"
                :plugin-id="pluginId"
                :component="tool.component"
                variant="page"
            />
            <EmptyState
                v-else
                type="empty"
                :heading="formatMessage(messages.noToolTitle)"
                :description="formatMessage(messages.noToolDescription)"
            />
        </template>
    </main>
</template>
