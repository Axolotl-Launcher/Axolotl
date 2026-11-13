<script setup lang="ts">
import { Button, defineMessages, useVIntl } from '@modrinth/ui'

import { usePinnedOverride } from './pinned-override'

const props = defineProps<{
    keyName: string
}>()

const emit = defineEmits<{
    cleared: []
}>()

const { formatMessage } = useVIntl()
const { isOverridden, restoring, restore } = usePinnedOverride(() => props.keyName)

const messages = defineMessages({
    hint: {
        id: 'app.settings.override.hint',
        defaultMessage: 'Overrides the deployed value.',
    },
    restore: {
        id: 'app.settings.override.restore',
        defaultMessage: 'Restore deployed value',
    },
})

async function restoreOverride() {
    if (await restore()) emit('cleared')
}
</script>

<template>
    <div v-if="isOverridden" class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1">
        <span class="text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.hint) }}
        </span>
        <Button type="quiet" size="sm" :loading="restoring" @click="restoreOverride">
            {{ formatMessage(messages.restore) }}
        </Button>
    </div>
</template>
