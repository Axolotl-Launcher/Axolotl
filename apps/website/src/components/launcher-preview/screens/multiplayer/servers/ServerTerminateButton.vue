<script setup lang="ts">
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref } from 'vue'

import { useServers } from '../composables/useServers.ts'

const props = withDefaults(defineProps<{ serverId: string; size?: '2xs' | 'sm' | 'md' }>(), {
    size: 'sm',
})
const { killServer } = useServers()
const { formatMessage } = useVIntl()
const pending = ref(false)
const messages = defineMessages({
    terminate: { id: 'app.servers.action.terminate', defaultMessage: 'Terminate' },
})
async function terminate() {
    if (pending.value) return
    pending.value = true
    try {
        await killServer(props.serverId)
    } finally {
        pending.value = false
    }
}
</script>

<template>
    <Button type="outlined" color="red" :size="size" :loading="pending" @click="terminate">
        <XIcon />{{ formatMessage(messages.terminate) }}
    </Button>
</template>
