<script setup lang="ts">
import { defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'

const { formatMessage, locale } = useVIntl()
const { handleError } = injectNotificationManager()
const opening = ref(false)
const trigger = ref<HTMLButtonElement>()
const messages = defineMessages({
    open: { id: 'app.agent-window.open', defaultMessage: 'Open agent window' },
    title: { id: 'app.agent-window.title', defaultMessage: 'Axolotl Agent Window' },
    minimize: { id: 'app.window-controls.minimize', defaultMessage: 'Minimize' },
    maximize: { id: 'app.window-controls.maximize', defaultMessage: 'Maximize' },
    restore: { id: 'app.window-controls.restore', defaultMessage: 'Restore' },
    close: { id: 'app.window-controls.close', defaultMessage: 'Close' },
})

async function openWindow() {
    if (opening.value) return
    opening.value = true
    try {
        const style = getComputedStyle(trigger.value ?? document.documentElement)
        await invoke('plugin:agent-window|open_agent_window', {
            appearance: {
                locale: locale.value,
                background: style.getPropertyValue('--surface-1').trim(),
                surface:
                    style.getPropertyValue('--surface-bar').trim() ||
                    style.getPropertyValue('--surface-3').trim(),
                text: style.getPropertyValue('--color-text-primary').trim(),
                accent: style.getPropertyValue('--color-brand').trim(),
                labels: {
                    title: formatMessage(messages.title),
                    minimize: formatMessage(messages.minimize),
                    maximize: formatMessage(messages.maximize),
                    restore: formatMessage(messages.restore),
                    close: formatMessage(messages.close),
                },
            },
        })
    } catch (error) {
        handleError(error)
    } finally {
        opening.value = false
    }
}
</script>

<template>
    <button
        ref="trigger"
        type="button"
        data-tauri-drag-region-exclude
        data-testid="agent-window-button"
        :aria-label="formatMessage(messages.open)"
        :aria-busy="opening"
        :disabled="opening"
        class="agent-window-trigger relative inline-flex size-9 shrink-0 cursor-pointer items-center justify-center rounded-full border-none bg-transparent p-0 text-[var(--color-text-default)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-wait"
        @click="openWindow"
    >
        <span
            aria-hidden="true"
            class="agent-window-label pointer-events-none absolute right-0 top-0 z-10 flex h-full items-center whitespace-nowrap rounded-xl bg-surface-4 pl-3 pr-9 text-sm font-medium leading-5"
            >{{ formatMessage(messages.open) }}</span
        >
        <svg
            aria-hidden="true"
            viewBox="0 0 24 24"
            fill="currentColor"
            class="relative z-20 size-6 shrink-0"
        >
            <path
                fill-rule="evenodd"
                d="M11 2a1 1 0 0 1 2 0v2h4a4 4 0 0 1 4 4v9a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4V8a4 4 0 0 1 4-4h4V2ZM8.5 9a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3Zm7 0a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3ZM8 16a1 1 0 0 0 0 2h8a1 1 0 1 0 0-2H8Z"
                clip-rule="evenodd"
            />
        </svg>
    </button>
</template>

<style scoped>
.agent-window-trigger {
    transition: color 150ms ease;
}

.agent-window-label {
    opacity: 0;
    transform: translateX(0.5rem);
    visibility: hidden;
    transition:
        opacity 160ms cubic-bezier(0.23, 1, 0.32, 1),
        transform 160ms cubic-bezier(0.23, 1, 0.32, 1),
        visibility 160ms;
}

.agent-window-trigger:focus-visible {
    color: var(--color-brand);
}

.agent-window-trigger:focus-visible .agent-window-label {
    opacity: 1;
    transform: translateX(0);
    visibility: visible;
    transition: none;
    pointer-events: auto;
}

@media (hover: hover) and (pointer: fine) {
    .agent-window-trigger:hover {
        color: var(--color-brand);
    }

    .agent-window-trigger:hover .agent-window-label {
        opacity: 1;
        transform: translateX(0);
        visibility: visible;
        pointer-events: auto;
    }
}

@media (prefers-reduced-motion: reduce) {
    .agent-window-label {
        transform: none;
        transition: opacity 125ms ease;
    }
}
</style>
