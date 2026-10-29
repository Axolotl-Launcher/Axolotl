import { computed, readonly, shallowRef } from 'vue'

export function createMcpPreviewSession() {
    const pendingClient = shallowRef<string | null>(null)
    const connectedClient = shallowRef<string | null>(null)

    function disconnect() {
        pendingClient.value = null
        connectedClient.value = null
    }

    function requestConnection(client: string, fullAccess: boolean) {
        if (pendingClient.value || connectedClient.value) return
        if (fullAccess) connectedClient.value = client
        else pendingClient.value = client
    }

    function approveConnection() {
        if (!pendingClient.value) return
        connectedClient.value = pendingClient.value
        pendingClient.value = null
    }

    return {
        pendingClient: readonly(pendingClient),
        connectedClient: readonly(connectedClient),
        requestConnection,
        approveConnection,
        disconnect,
    }
}

// Debug sessions and their permissions never persist or authorize real clients.
const fullAccess = shallowRef(false)
const session = createMcpPreviewSession()

export function useMcpPreview() {
    return {
        ...session,
        fullAccess: computed(() => fullAccess.value),
        setFullAccess(enabled: boolean) {
            session.disconnect()
            fullAccess.value = enabled
        },
        simulateConnection(client: string) {
            session.requestConnection(client, fullAccess.value)
        },
    }
}
