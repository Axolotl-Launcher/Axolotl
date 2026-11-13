<script setup lang="ts">
import { CheckIcon, SpinnerIcon } from '@modrinth/assets'

import Button from '#ui/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'

withDefaults(
    defineProps<{
        count: number
        progress?: { completed: number; total: number } | null
        disabled?: boolean
    }>(),
    { progress: null, disabled: false },
)
const emit = defineEmits<{
    select: []
}>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
    label: {
        id: 'browse.select-all-button.label',
        defaultMessage: 'Select all ({count})',
    },
    selecting: {
        id: 'browse.select-all-button.selecting',
        defaultMessage: 'Selecting {completed}/{total}',
    },
})
</script>

<template>
    <Button
        v-if="count > 0 || progress"
        type="outlined"
        :disabled="disabled || !!progress"
        @click="emit('select')"
    >
        <SpinnerIcon v-if="progress" class="animate-spin" />
        <CheckIcon v-else />
        {{
            progress
                ? formatMessage(messages.selecting, {
                      completed: progress.completed,
                      total: progress.total,
                  })
                : formatMessage(messages.label, { count })
        }}
    </Button>
</template>
