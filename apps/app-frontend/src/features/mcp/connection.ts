import { computed } from 'vue'

import { useMcpPreview } from './preview'
import { useMcpRuntime } from './runtime'

export function useMcpConnection() {
    const runtime = useMcpRuntime()
    const preview = useMcpPreview()
    const realConnection = computed(() => runtime.state.value.connection)
    const pendingConnectionId = computed(() =>
        realConnection.value
            ? realConnection.value.awaiting_approval
                ? realConnection.value.id
                : null
            : preview.pendingClient.value
              ? `preview:${preview.pendingClient.value}`
              : null,
    )
    return {
        pendingConnectionId,
        connectedClient: computed(() =>
            realConnection.value
                ? realConnection.value.connected
                    ? realConnection.value.client
                    : null
                : preview.connectedClient.value,
        ),
        pendingClient: computed(() =>
            realConnection.value
                ? realConnection.value.awaiting_approval
                    ? realConnection.value.client
                    : null
                : preview.pendingClient.value,
        ),
        fullAccess: computed(() =>
            realConnection.value ? runtime.state.value.full_access : preview.fullAccess.value,
        ),
        error: runtime.error,
        approveConnection(expectedId?: string) {
            if (expectedId && pendingConnectionId.value !== expectedId) return false
            if (realConnection.value) return runtime.approve(realConnection.value.id)
            preview.approveConnection()
        },
        disconnect() {
            if (realConnection.value) return runtime.disconnect(realConnection.value.id)
            preview.disconnect()
        },
    }
}
