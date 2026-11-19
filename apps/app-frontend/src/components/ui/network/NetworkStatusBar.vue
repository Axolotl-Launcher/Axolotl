<script setup lang="ts">
import { TagCategoryWifiOffIcon } from '@modrinth/assets'
import { useVIntl } from '@modrinth/ui'

import { useNetworkStatus } from '@/composables/useNetworkStatus'

import { networkMessages as messages } from './messages'

const { formatMessage } = useVIntl()
const { offline } = useNetworkStatus()
</script>

<template>
    <div
        v-if="offline"
        v-tooltip.top="formatMessage(messages.disconnected)"
        class="fixed bottom-4 right-4 z-40 flex items-center rounded-full border border-solid border-surface-4 bg-surface-2 px-4 py-3 text-[var(--color-red)] shadow-lg"
        role="status"
        :aria-label="formatMessage(messages.disconnected)"
        tabindex="0"
    >
        <TagCategoryWifiOffIcon class="size-5" aria-hidden="true" />
    </div>
</template>
