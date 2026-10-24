<script setup lang="ts">
import { CheckIcon } from '@modrinth/assets'
import { defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import ManagedServerIcon from '@/components/multiplayer/servers/ServerIcon.vue'
import type { ServerInfoData } from '@/helpers/servers'

const props = defineProps<{
    servers: ServerInfoData[]
    selectedServerId?: string | null
}>()

const emit = defineEmits<{
    select: [server: ServerInfoData]
}>()

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof NewModal>>()

const messages = defineMessages({
    title: {
        id: 'app.home.servers.link-picker.title',
        defaultMessage: 'Link a local server',
    },
    description: {
        id: 'app.home.servers.link-picker.description',
        defaultMessage:
            'Launching this server from Home will start the chosen local server first and wait until it is ready.',
    },
    empty: {
        id: 'app.home.servers.link-picker.empty',
        defaultMessage: 'No local servers yet. Create one first, then link it here.',
    },
    select: {
        id: 'app.home.servers.link-picker.select',
        defaultMessage: 'Link {name}',
    },
})

function show() {
    modal.value?.show()
}

function select(server: ServerInfoData) {
    emit('select', server)
    modal.value?.hide()
}

function addressOf(server: ServerInfoData) {
    return server.port ? `localhost:${server.port}` : `${server.serverType} ${server.gameVersion}`
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        max-width="560px"
        width="min(560px, calc(100vw - 2rem))"
        scrollable
        max-content-height="min(36rem, 70vh)"
    >
        <div class="flex min-w-0 flex-col gap-3">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
            <p
                v-if="props.servers.length === 0"
                class="m-0 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.empty) }}
            </p>
            <ul v-else class="m-0 flex list-none flex-col gap-1 p-0">
                <li v-for="server in props.servers" :key="server.id">
                    <button
                        type="button"
                        class="flex w-full min-w-0 items-center gap-3 rounded-lg border border-solid border-transparent p-2 text-left transition-colors hover:bg-button-bg focus-visible:bg-button-bg"
                        :aria-label="formatMessage(messages.select, { name: server.name })"
                        @click="select(server)"
                    >
                        <ManagedServerIcon
                            :icon-path="server.iconPath"
                            :server-type="server.serverType"
                            :server-id="server.id"
                            size="36px"
                        />
                        <span class="flex min-w-0 flex-1 flex-col">
                            <span
                                class="truncate text-sm font-semibold text-[var(--color-text-primary)]"
                                >{{ server.name }}</span
                            >
                            <span class="truncate text-xs text-[var(--color-text-tertiary)]">
                                {{ server.running ? '▶ ' : '' }}{{ addressOf(server) }}
                            </span>
                        </span>
                        <CheckIcon
                            v-if="server.id === props.selectedServerId"
                            class="size-5 shrink-0 text-brand"
                            aria-hidden="true"
                        />
                    </button>
                </li>
            </ul>
        </div>
    </NewModal>
</template>
