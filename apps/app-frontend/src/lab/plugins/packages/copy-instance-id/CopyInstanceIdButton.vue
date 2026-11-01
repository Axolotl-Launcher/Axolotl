<script setup lang="ts">
import { CopyIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { onBeforeUnmount, ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

const props = defineProps<{ instance: GameInstance }>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    copy: {
        id: 'app.lab.plugins.copy-instance-id.copy',
        defaultMessage: 'Copy instance ID',
    },
    copied: {
        id: 'app.lab.plugins.copy-instance-id.copied',
        defaultMessage: 'Instance ID copied',
    },
})

const copied = ref(false)
let resetTimer: ReturnType<typeof setTimeout> | undefined

async function copyInstanceId() {
    try {
        await navigator.clipboard.writeText(props.instance.id)
        copied.value = true
        if (resetTimer) clearTimeout(resetTimer)
        resetTimer = setTimeout(() => {
            copied.value = false
        }, 2000)
    } catch {
        copied.value = false
    }
}

onBeforeUnmount(() => {
    if (resetTimer) clearTimeout(resetTimer)
})
</script>

<template>
    <Button
        v-tooltip="formatMessage(copied ? messages.copied : messages.copy)"
        type="outlined"
        size="xl"
        circular
        icon-only
        :label="formatMessage(copied ? messages.copied : messages.copy)"
        @click="copyInstanceId"
    >
        <CopyIcon />
    </Button>
</template>
