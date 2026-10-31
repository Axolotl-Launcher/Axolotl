<script setup lang="ts">
import { defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { invoke } from '@tauri-apps/api/core'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { agentWindowMessages } from './messages'

const { formatMessage, locale } = useVIntl()
const { handleError } = injectNotificationManager()
const opening = ref(false)
const trigger = ref<HTMLButtonElement>()
let windowOpened = false
let appearanceObserver: MutationObserver | undefined
let updateScheduled: number | undefined
let updateNeeded = false
let appearanceFlight: Promise<void> | undefined
let disposed = false
const messages = defineMessages({
    open: { id: 'app.agent-window.open', defaultMessage: 'Open agent window' },
    title: { id: 'app.agent-window.title', defaultMessage: 'Axolotl Agent Window' },
    minimize: { id: 'app.window-controls.minimize', defaultMessage: 'Minimize' },
    maximize: { id: 'app.window-controls.maximize', defaultMessage: 'Maximize' },
    restore: { id: 'app.window-controls.restore', defaultMessage: 'Restore' },
    close: { id: 'app.window-controls.close', defaultMessage: 'Close' },
})

function windowAppearance() {
    const host = trigger.value ?? document.documentElement
    const probe = document.createElement('span')
    probe.hidden = true
    host.appendChild(probe)
    const resolve = (variable: string) => {
        probe.style.color = `var(${variable})`
        return getComputedStyle(probe).color
    }
    try {
        probe.style.borderRadius = 'var(--radius-lg)'
        return {
            locale: locale.value,
            background: resolve('--surface-1'),
            surface: resolve('--surface-bar, var(--surface-3)'),
            text: resolve('--color-text-primary'),
            accent: resolve('--color-brand'),
            scrollbar: resolve('--color-scrollbar'),
            scrollbar_track: resolve('--color-bg'),
            scrollbar_radius: getComputedStyle(probe).borderTopLeftRadius,
            messages: Object.fromEntries(
                Object.entries(agentWindowMessages).map(([key, message]) => [
                    key,
                    formatMessage(message),
                ]),
            ),
            labels: {
                title: formatMessage(messages.title),
                minimize: formatMessage(messages.minimize),
                maximize: formatMessage(messages.maximize),
                restore: formatMessage(messages.restore),
                close: formatMessage(messages.close),
            },
        }
    } finally {
        probe.remove()
    }
}

function scheduleAppearanceUpdate() {
    if (!windowOpened || disposed) return
    updateNeeded = true
    if (opening.value || appearanceFlight || updateScheduled !== undefined) return
    updateScheduled = requestAnimationFrame(() => {
        updateScheduled = undefined
        void flushAppearanceUpdates()
    })
}

async function flushAppearanceUpdates() {
    if (opening.value || appearanceFlight || disposed) return
    appearanceFlight = (async () => {
        while (updateNeeded && !opening.value && !disposed) {
            updateNeeded = false
            try {
                await invoke('plugin:agent-window|update_agent_window_appearance', {
                    appearance: windowAppearance(),
                })
            } catch (error) {
                handleError(error)
            }
        }
    })()
    try {
        await appearanceFlight
    } finally {
        appearanceFlight = undefined
        if (updateNeeded) scheduleAppearanceUpdate()
    }
}

onMounted(() => {
    appearanceObserver = new MutationObserver(scheduleAppearanceUpdate)
    appearanceObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['class', 'style'],
    })
})
watch(locale, scheduleAppearanceUpdate)
onBeforeUnmount(() => {
    disposed = true
    appearanceObserver?.disconnect()
    if (updateScheduled !== undefined) cancelAnimationFrame(updateScheduled)
})

async function openWindow() {
    if (opening.value) return
    opening.value = true
    if (updateScheduled !== undefined) {
        cancelAnimationFrame(updateScheduled)
        updateScheduled = undefined
    }
    try {
        await appearanceFlight
        await invoke('plugin:agent-window|open_agent_window', { appearance: windowAppearance() })
        windowOpened = true
    } catch (error) {
        handleError(error)
    } finally {
        opening.value = false
        if (windowOpened) scheduleAppearanceUpdate()
    }
}
</script>

<template>
    <button
        ref="trigger"
        type="button"
        data-tauri-drag-region-exclude
        data-testid="agent-window-button"
        data-onboarding-id="agent-window-open"
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
