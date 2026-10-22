<script setup lang="ts">
import { Admonition, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'

import { useServers } from '@/composables/useServers'
import FileStudio from '@/pages/instance/FileStudio.vue'

const route = useRoute()
const serverId = route.params.id as string
const { servers, refresh } = useServers()
const { formatMessage } = useVIntl()
const isLoaded = ref(false)

const messages = defineMessages({
    notFound: {
        id: 'app.servers.detail.not-found',
        defaultMessage: 'This server no longer exists.',
    },
    runningTitle: {
        id: 'app.servers.files.studio-running-title',
        defaultMessage: 'Stop the server before opening Studio',
    },
    runningDescription: {
        id: 'app.servers.files.busy-tooltip',
        defaultMessage: 'Stop the server to modify files',
    },
})

const server = computed(() => servers.value.find((entry) => entry.id === serverId))

onMounted(async () => {
    if (servers.value.length === 0) await refresh()
    isLoaded.value = true
})
</script>

<template>
    <FileStudio v-if="server && !server.running && !server.starting" :server="server" />
    <div v-else-if="server" class="flex size-full items-center justify-center p-6">
        <Admonition type="warning" :header="formatMessage(messages.runningTitle)">
            {{ formatMessage(messages.runningDescription) }}
        </Admonition>
    </div>
    <div
        v-else-if="isLoaded"
        class="flex size-full items-center justify-center text-[var(--color-text-tertiary)]"
    >
        {{ formatMessage(messages.notFound) }}
    </div>
</template>
