import { computed, readonly, ref } from 'vue'

export function createNetworkStatus(initialBrowserOffline = false) {
    const browserOffline = ref(initialBrowserOffline)
    const simulatedOffline = ref(false)
    const retrying = ref(false)
    const offline = computed(() => browserOffline.value || simulatedOffline.value)
    let retry: Promise<boolean> | undefined

    function refreshBrowserOffline() {
        browserOffline.value = typeof navigator !== 'undefined' && !navigator.onLine
    }

    return {
        offline,
        browserOffline: readonly(browserOffline),
        simulatedOffline: readonly(simulatedOffline),
        retrying: readonly(retrying),
        setSimulatedOffline(value: boolean) {
            simulatedOffline.value = value
        },
        setBrowserOffline(value: boolean) {
            browserOffline.value = value
        },
        markBrowserOnline() {
            browserOffline.value = false
        },
        refreshBrowserOffline,
        retryConnection() {
            if (retry) return retry
            retrying.value = true
            // Coalesce rapid clicks, then read the current OS/browser state rather than a stale snapshot.
            retry = new Promise<boolean>((resolve) => {
                setTimeout(() => {
                    refreshBrowserOffline()
                    retrying.value = false
                    retry = undefined
                    resolve(!offline.value)
                }, 600)
            })
            return retry
        },
    }
}

const sharedNetworkStatus = createNetworkStatus(
    typeof navigator !== 'undefined' && !navigator.onLine,
)

if (typeof window !== 'undefined') {
    window.addEventListener('offline', () => {
        sharedNetworkStatus.setBrowserOffline(true)
    })
    window.addEventListener('online', () => {
        sharedNetworkStatus.markBrowserOnline()
    })
}

export function useNetworkStatus() {
    return sharedNetworkStatus
}

export function isOfflineMode() {
    return sharedNetworkStatus.offline.value
}
