<script setup lang="ts">
import CheckCircleIcon from '@modrinth/assets/icons/check-circle.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import ProgressBar from '@modrinth/ui/src/components/base/ProgressBar.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onMounted } from 'vue'

import { injectCreateServerFlow } from '../create-server-flow.ts'

const { formatMessage } = useVIntl()
const ctx = injectCreateServerFlow()

const messages = defineMessages({
    downloading: {
        id: 'app.servers.wizard.downloading',
        defaultMessage: 'Downloading server files...',
    },
    firstRun: { id: 'app.servers.wizard.first-run', defaultMessage: 'Running first start...' },
    eulaWait: {
        id: 'app.servers.wizard.eula-wait',
        defaultMessage: 'Waiting for EULA confirmation',
    },
    done: { id: 'app.servers.wizard.done', defaultMessage: 'Server ready' },
    failed: { id: 'app.servers.wizard.failed', defaultMessage: 'Setup failed' },
    installLog: { id: 'app.servers.wizard.log', defaultMessage: 'Output' },
    backgroundHint: {
        id: 'app.servers.wizard.background-hint',
        defaultMessage: 'You can close this window — the download continues in the background.',
    },
})

onMounted(() => {
    if (ctx.installPhase.value === 'idle' || ctx.installPhase.value === 'error') {
        void ctx.beginInstall()
    }
})

const phaseText = computed(() => {
    switch (ctx.installPhase.value) {
        case 'first-run':
            return formatMessage(messages.firstRun)
        case 'eula':
            return formatMessage(messages.eulaWait)
        case 'done':
            return formatMessage(messages.done)
        case 'error':
            return formatMessage(messages.failed)
        default:
            return formatMessage(messages.downloading)
    }
})

const progressPercent = computed(() => {
    const progress = ctx.downloadProgress.value
    if (!progress || !progress.total) return 0
    return Math.min(100, (progress.downloaded / progress.total) * 100)
})

const isBusy = computed(
    () =>
        ctx.installPhase.value === 'preparing' ||
        ctx.installPhase.value === 'downloading' ||
        ctx.installPhase.value === 'first-run',
)
</script>

<template>
    <div class="flex flex-col gap-5">
        <div class="flex items-center gap-3">
            <SpinnerIcon v-if="isBusy" class="size-6 shrink-0 animate-spin text-orange" />
            <CheckCircleIcon
                v-else-if="ctx.installPhase.value === 'done'"
                class="size-6 shrink-0 text-green"
            />
            <span class="text-lg font-semibold text-[var(--color-text-primary)]">{{
                phaseText
            }}</span>
        </div>

        <ProgressBar
            v-if="ctx.installPhase.value === 'downloading'"
            full-width
            :progress="progressPercent"
            :max="100"
            :waiting="progressPercent === 0"
            :label="formatMessage(messages.downloading)"
            show-progress
        />

        <p
            v-if="ctx.installPhase.value === 'downloading'"
            class="m-0 text-xs font-medium text-[var(--color-text-tertiary)]"
        >
            {{ formatMessage(messages.backgroundHint) }}
        </p>

        <Admonition
            v-if="ctx.installPhase.value === 'error'"
            type="critical"
            :header="formatMessage(messages.failed)"
        >
            {{ ctx.installError.value }}
        </Admonition>

        <div v-if="ctx.installPhase.value === 'error'" class="flex flex-col gap-2">
            <span class="text-sm font-semibold text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.installLog) }}
            </span>
            <pre
                class="max-h-56 overflow-y-auto whitespace-pre-wrap rounded-xl border border-solid border-surface-4 bg-surface-3 p-3 font-mono text-xs leading-relaxed text-[var(--color-text-default)]"
                >{{ ctx.installLog.value.slice(-40).join('\n') }}</pre>
        </div>
    </div>
</template>
