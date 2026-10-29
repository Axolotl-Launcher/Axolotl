<script setup lang="ts">
import { Button, NewModal, useVIntl } from '@modrinth/ui'
import { nextTick, ref, watch } from 'vue'

import { useMcpConnection } from './connection'
import { mcpMessages as messages } from './messages'

defineProps<{ rounded?: boolean }>()

const {
    connectedClient,
    pendingClient,
    pendingConnectionId,
    approveConnection,
    disconnect,
    error,
} = useMcpConnection()
const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof NewModal>>()
const requestingClient = ref('')
const requestingId = ref('')

watch(
    pendingConnectionId,
    async (id) => {
        if (id && pendingClient.value) {
            requestingClient.value = pendingClient.value
            requestingId.value = id
            await nextTick()
            if (pendingConnectionId.value === id) modal.value?.show()
        } else {
            modal.value?.hide()
        }
    },
    { immediate: true },
)

const approving = ref(false)
async function allow() {
    if (approving.value) return
    approving.value = true
    try {
        await approveConnection(requestingId.value)
    } finally {
        approving.value = false
        await nextTick()
        if (!pendingClient.value) modal.value?.hide()
    }
}

function decline() {
    if (pendingClient.value && !approving.value) disconnect()
}
</script>

<template>
    <Teleport to="body">
        <Transition name="mcp-connection" appear :duration="{ enter: 600, leave: 180 }">
            <div
                v-if="connectedClient"
                aria-hidden="true"
                data-testid="mcp-connection-border"
                class="mcp-connection-glow pointer-events-none fixed inset-0 z-[210] overflow-hidden shadow-[inset_0_0_0_1px_color-mix(in_srgb,var(--color-brand)_40%,transparent)] before:opacity-40"
                :class="{ 'rounded-[8px]': rounded }"
            />
        </Transition>
    </Teleport>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.connectionTitle)"
        width="min(30rem, calc(100vw - 2rem))"
        :on-hide="decline"
        :disable-close="approving"
    >
        <div class="flex flex-col gap-4">
            <p v-if="error" role="alert" class="m-0 text-sm text-red">{{ error }}</p>
            <p class="m-0 text-base text-[var(--color-text-primary)]">
                {{ formatMessage(messages.connectionDescription, { client: requestingClient }) }}
            </p>
            <ul
                class="m-0 flex flex-col gap-2 pl-5 text-sm leading-relaxed text-[var(--color-text-tertiary)]"
            >
                <li>{{ formatMessage(messages.noDeletion) }}</li>
                <li>{{ formatMessage(messages.toolsOnly) }}</li>
            </ul>
            <p class="m-0 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.disconnectHint) }}
            </p>
        </div>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button :disabled="approving" @click="disconnect">{{
                    formatMessage(messages.denyConnection)
                }}</Button>
                <Button
                    type="colored"
                    :disabled="approving"
                    data-testid="mcp-approve"
                    @click="allow"
                >
                    {{ formatMessage(messages.allowConnection) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<style scoped>
.mcp-connection-enter-active,
.mcp-connection-leave-active {
    transition: opacity 180ms cubic-bezier(0.23, 1, 0.32, 1);
}

.mcp-connection-enter-from,
.mcp-connection-leave-to {
    opacity: 0;
}

.mcp-connection-glow::before {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--color-brand);
    mask:
        linear-gradient(to bottom, #000, #0008 28%, transparent) top / 100% 40px no-repeat,
        linear-gradient(to left, #000, #0008 28%, transparent) right / 40px 100% no-repeat,
        linear-gradient(to top, #000, #0008 28%, transparent) bottom / 100% 40px no-repeat,
        linear-gradient(to right, #000, #0008 28%, transparent) left / 40px 100% no-repeat;
}

@media (prefers-reduced-motion: no-preference) {
    .mcp-connection-glow::before {
        animation: mcp-connection-breathe 4s cubic-bezier(0.77, 0, 0.175, 1) infinite;
    }

    .mcp-connection-enter-active::before {
        animation: mcp-connection-arrive 600ms cubic-bezier(0.23, 1, 0.32, 1) both;
    }
}

@media (prefers-reduced-motion: reduce) {
    .mcp-connection-enter-active,
    .mcp-connection-leave-active {
        transition-duration: 100ms;
    }
}

@keyframes mcp-connection-arrive {
    0%,
    100% {
        opacity: 0.36;
    }
    20% {
        opacity: 0.88;
    }
}

@keyframes mcp-connection-breathe {
    0%,
    100% {
        opacity: 0.36;
    }
    50% {
        opacity: 0.46;
    }
}
</style>
