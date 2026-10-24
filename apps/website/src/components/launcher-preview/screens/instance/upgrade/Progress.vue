<template>
    <p class="m-0 py-2 text-[var(--color-text-tertiary)]">{{ formatMessage(messages.opening) }}</p>
</template>

<script setup lang="ts">
import './messages'

import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { watch } from 'vue'
import { useRouter } from 'vue-router'

import { upgradeProgressDestination, useInstanceUpgradeFlow } from './flow'

const messages = defineMessages({
    opening: {
        id: 'instance.upgrade.progress.opening-downloads',
        defaultMessage: 'Opening download task…',
    },
})

const flow = useInstanceUpgradeFlow()
const router = useRouter()
const { formatMessage } = useVIntl()
let navigating = false

flow.registerStepControls(null)

watch(
    [flow.jobRecoveryState, flow.activeJobId],
    async ([recoveryState, jobId]) => {
        const destination = upgradeProgressDestination(recoveryState, jobId, flow.instanceId.value)
        if (!destination || navigating) return
        navigating = true
        try {
            await router.replace(destination)
        } finally {
            navigating = false
        }
    },
    { immediate: true },
)
</script>
