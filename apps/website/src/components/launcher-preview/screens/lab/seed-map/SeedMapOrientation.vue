<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useId } from 'vue'

import type { PreviewAxis } from './preview-overlays.ts'

defineProps<{ axes: PreviewAxis[] }>()
const id = useId()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    axes: {
        id: 'app.lab.seed-map.preview.axes',
        defaultMessage: 'World axes: +X east, +Y up, +Z south',
    },
})
const colors = { X: 'var(--color-red)', Y: 'var(--color-green)', Z: 'var(--color-blue)' }
</script>

<template>
    <svg
        viewBox="-48 -48 96 96"
        class="pointer-events-none absolute bottom-2 right-2 size-24 rounded-lg bg-[color-mix(in_srgb,var(--surface-2)_var(--opacity-ratio-keep-80),transparent)]"
        role="img"
        :aria-label="formatMessage(messages.axes)"
        data-testid="seed-map-axis-indicator"
    >
        <title>{{ formatMessage(messages.axes) }}</title>
        <defs>
            <marker
                v-for="axis in axes"
                :id="`${id}-${axis.axis}`"
                :key="axis.axis"
                viewBox="0 0 6 6"
                refX="5"
                refY="3"
                markerWidth="5"
                markerHeight="5"
                orient="auto-start-reverse"
            >
                <path d="M 0 0 L 6 3 L 0 6 z" :fill="colors[axis.axis]" />
            </marker>
        </defs>
        <g
            v-for="axis in axes"
            :key="axis.axis"
            :data-testid="`seed-map-axis-${axis.axis}`"
            :style="{ color: colors[axis.axis] }"
        >
            <line
                x1="0"
                y1="0"
                :x2="axis.x"
                :y2="axis.y"
                stroke="currentColor"
                stroke-width="2"
                :marker-end="`url(#${id}-${axis.axis})`"
            />
            <text
                :x="axis.x * 1.3"
                :y="axis.y * 1.3 - 3"
                fill="currentColor"
                text-anchor="middle"
                dominant-baseline="middle"
                class="text-[11px] font-bold"
            >
                {{ `+${axis.axis}` }}
            </text>
        </g>
    </svg>
</template>
