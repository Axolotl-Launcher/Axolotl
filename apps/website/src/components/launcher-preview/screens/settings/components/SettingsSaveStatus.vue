<script setup lang="ts">
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

import { messages } from '../messages'

/**
 * Inline "Saving… / Saved / Could not save [Retry]" indicator of the desktop app
 * (components/ui/settings/SettingsSaveStatus.vue), shown in a section header.
 */
const props = withDefaults(
    defineProps<{
        status?: 'idle' | 'saving' | 'saved' | 'error'
        retry?: (() => void) | undefined
    }>(),
    {
        status: 'idle',
        retry: undefined,
    },
)

const { formatMessage } = useVIntl()

const statusMessage = {
    saving: messages.appSettingsSaveStatusSaving,
    saved: messages.appSettingsSaveStatusSaved,
    error: messages.appSettingsSaveStatusError,
} as const
</script>

<template>
    <div
        v-if="props.status !== 'idle'"
        class="settings-save-status inline-flex items-center gap-1 text-xs text-[var(--color-text-tertiary)]"
        role="status"
    >
        <SpinnerIcon v-if="props.status === 'saving'" class="size-3.5 animate-spin" />
        <span>{{ props.status === 'idle' ? '' : formatMessage(statusMessage[props.status]) }}</span>
        <Button
            v-if="props.status === 'error' && props.retry"
            type="quiet"
            size="2xs"
            @click="props.retry"
        >
            {{ formatMessage(messages.appSettingsSaveStatusRetry) }}
        </Button>
    </div>
</template>
