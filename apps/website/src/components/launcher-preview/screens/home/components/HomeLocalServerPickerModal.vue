<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import { homeMessages } from '../messages'
import type { HomeLocalServer } from '../state'
import ManagedServerIcon from './ManagedServerIcon.vue'

const props = defineProps<{
    servers: HomeLocalServer[]
    selectedServerId?: string | null
}>()

const emit = defineEmits<{
    select: [server: HomeLocalServer]
}>()

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof PreviewModal>>()
const messages = homeMessages

function show() {
    modal.value?.show()
}

function select(server: HomeLocalServer) {
    emit('select', server)
    modal.value?.hide()
}

function addressOf(server: HomeLocalServer) {
    return server.port ? `localhost:${server.port}` : `${server.serverType} ${server.gameVersion}`
}

defineExpose({ show })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.serversLinkPickerTitle)"
        max-width="560px"
        width="min(560px, calc(100% - 2rem))"
        scrollable
        max-content-height="min(36rem, 560px)"
    >
        <div class="flex min-w-0 flex-col gap-3">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.serversLinkPickerDescription) }}
            </p>
            <p
                v-if="props.servers.length === 0"
                class="m-0 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.serversLinkPickerEmpty) }}
            </p>
            <ul v-else class="m-0 flex list-none flex-col gap-1 p-0">
                <li v-for="server in props.servers" :key="server.id">
                    <button
                        type="button"
                        class="hover:bg-button-bg focus-visible:bg-button-bg flex w-full min-w-0 items-center gap-3 rounded-lg border border-solid border-transparent p-2 text-left transition-colors"
                        :aria-label="
                            formatMessage(messages.serversLinkPickerSelect, { name: server.name })
                        "
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
    </PreviewModal>
</template>
