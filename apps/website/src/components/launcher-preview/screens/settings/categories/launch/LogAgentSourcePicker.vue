<script setup lang="ts">
import CloudIcon from '@modrinth/assets/icons/cloud.svg?component'
import MonitorIcon from '@modrinth/assets/icons/monitor.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

import { messages } from '../../messages'
import type { AnalysisSource } from './state'

/**
 * Analysis source radio cards of the app (apps/app-frontend/src/components/ui/
 * LogAgentSourcePicker.vue): Local LogAgent or Cloud LogShare, arrow keys move
 * the selection between them.
 */
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
</script>

<template>
    <div
        role="radiogroup"
        :aria-label="formatMessage(messages.appLogagentPickerLabel)"
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
                    {{
                        formatMessage(
                            source === 'custom'
                                ? messages.appLogagentSourceLocal
                                : messages.appLogagentSourceCloud,
                        )
                    }}
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
                                  source === 'custom'
                                      ? messages.appLogagentPickerLocalHint
                                      : messages.appLogagentPickerCloudHint,
                              )
                    }}</span
                >
            </span>
        </button>
    </div>
</template>
