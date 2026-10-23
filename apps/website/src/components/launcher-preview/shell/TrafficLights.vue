<script setup lang="ts">
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

import { shellMessages } from '../lib/messages'
import { injectPreviewWindow } from '../lib/window'

/**
 * macOS window controls, drawn where the desktop app leaves the native ones:
 * AppKit's default left inset, inside the 5rem titlebar padding, centred on the
 * titlebar (`update_traffic_lights_anchor`). The glyphs appear while the group
 * is hovered or focused.
 */
const emit = defineEmits<{
    close: [event: MouseEvent]
    minimize: [event: MouseEvent]
    zoom: [event: MouseEvent]
}>()

const { formatMessage } = useVIntl()
const { fullScreen } = injectPreviewWindow()

const buttonClass =
    'flex size-3 cursor-default items-center justify-center rounded-full border-0 p-0 shadow-[inset_0_0_0_0.5px_rgba(0,0,0,0.18)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-focus-ring)] active:brightness-[0.82]'
const glyphClass =
    'size-3 opacity-0 transition-opacity duration-75 group-focus-within/lights:opacity-100 group-hover/lights:opacity-100'
</script>

<template>
    <div
        class="group/lights absolute left-2 top-1/2 z-10 flex -translate-y-1/2 items-center gap-2"
        role="group"
        :aria-label="formatMessage(shellMessages.windowControls)"
    >
        <button
            type="button"
            :class="[buttonClass, 'bg-[#ff5f57]']"
            :aria-label="formatMessage(shellMessages.windowClose)"
            @click="emit('close', $event)"
        >
            <svg
                viewBox="0 0 12 12"
                :class="glyphClass"
                fill="none"
                stroke="#4c0002"
                stroke-width="1.15"
                stroke-linecap="round"
                aria-hidden="true"
            >
                <path d="M3.6 3.6l4.8 4.8M8.4 3.6l-4.8 4.8" />
            </svg>
        </button>
        <button
            type="button"
            :class="[buttonClass, 'bg-[#febc2e]']"
            :aria-label="formatMessage(shellMessages.windowMinimize)"
            @click="emit('minimize', $event)"
        >
            <svg
                viewBox="0 0 12 12"
                :class="glyphClass"
                fill="none"
                stroke="#985712"
                stroke-width="1.15"
                stroke-linecap="round"
                aria-hidden="true"
            >
                <path d="M2.9 6h6.2" />
            </svg>
        </button>
        <button
            type="button"
            :class="[buttonClass, 'bg-[#28c840]']"
            :aria-label="
                formatMessage(
                    fullScreen
                        ? shellMessages.windowExitFullScreen
                        : shellMessages.windowFullScreen,
                )
            "
            @click="emit('zoom', $event)"
        >
            <svg viewBox="0 0 12 12" :class="glyphClass" fill="#0a6417" aria-hidden="true">
                <path d="M3.3 3.3h4l-4 4z" />
                <path d="M8.7 8.7h-4l4-4z" />
            </svg>
        </button>
    </div>
</template>
