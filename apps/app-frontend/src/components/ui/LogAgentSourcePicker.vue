<script setup lang="ts">
import { CloudIcon, MonitorIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'

import type { AnalysisSource } from '@/logagent/types'

const props = defineProps<{ modelValue: AnalysisSource; disabled?: boolean; localModel?: string }>()
const emit = defineEmits<{ 'update:modelValue': [source: AnalysisSource] }>()
const { formatMessage } = useVIntl()
function moveSelection(event: KeyboardEvent, source: AnalysisSource) {
    if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key) || props.disabled)
        return
    event.preventDefault()
    const next = source === 'custom' ? 'logshare' : 'custom'
    emit('update:modelValue', next)
    const group = (event.currentTarget as HTMLElement).parentElement
    group?.querySelector<HTMLButtonElement>(`[value="${next}"]`)?.focus()
}
const messages = defineMessages({
    label: { id: 'app.logagent.picker.label', defaultMessage: 'Analysis source' },
    local: { id: 'app.logagent.source-local', defaultMessage: 'Local LogAgent' },
    cloud: { id: 'app.logagent.source-cloud', defaultMessage: 'Cloud LogShare' },
    localHint: {
        id: 'app.logagent.picker.local-hint',
        defaultMessage: 'Uses your configured model. Relevant excerpts go to its provider.',
    },
    cloudHint: {
        id: 'app.logagent.picker.cloud-hint',
        defaultMessage: 'Sends crash logs to LogShare for analysis.',
    },
})
</script>

<template>
    <div
        role="radiogroup"
        :aria-label="formatMessage(messages.label)"
        class="flex min-w-0 flex-wrap gap-2"
    >
        <button
            v-for="source in ['custom', 'logshare'] as const"
            :key="source"
            type="button"
            role="radio"
            :aria-checked="modelValue === source"
            :tabindex="modelValue === source ? 0 : -1"
            :value="source"
            :disabled="disabled"
            class="flex min-w-0 flex-1 basis-48 gap-3 rounded-lg border border-solid p-3 text-left text-[var(--color-text-primary)] transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-not-allowed disabled:opacity-60"
            :class="
                modelValue === source
                    ? 'border-brand bg-surface-3'
                    : 'border-surface-4 bg-surface-2 hover:bg-surface-3'
            "
            @click="emit('update:modelValue', source)"
            @keydown="moveSelection($event, source)"
        >
            <component
                :is="source === 'custom' ? MonitorIcon : CloudIcon"
                class="mt-0.5 size-5 shrink-0 text-[var(--color-text-tertiary)]"
                aria-hidden="true"
            />
            <span class="flex min-w-0 flex-1 flex-col gap-1">
                <span class="flex items-center justify-between gap-2 text-sm font-semibold">
                    {{ formatMessage(source === 'custom' ? messages.local : messages.cloud) }}
                    <span
                        class="flex size-4 shrink-0 items-center justify-center rounded-full border border-solid"
                        :class="modelValue === source ? 'border-brand' : 'border-surface-5'"
                        aria-hidden="true"
                        ><span v-if="modelValue === source" class="size-2 rounded-full bg-brand"
                    /></span>
                </span>
                <span
                    class="break-words text-xs leading-relaxed text-[var(--color-text-tertiary)]"
                    >{{
                        source === 'custom' && localModel
                            ? localModel
                            : formatMessage(
                                  source === 'custom' ? messages.localHint : messages.cloudHint,
                              )
                    }}</span
                >
            </span>
        </button>
    </div>
</template>
