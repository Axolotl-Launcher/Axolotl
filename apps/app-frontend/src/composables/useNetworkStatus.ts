import { readonly, ref } from 'vue'

export function createNetworkStatus(initialBrowserOffline = false) {
    const browserOffline = ref(initialBrowserOffline)

    return {
        offline: readonly(browserOffline),
        browserOffline: readonly(browserOffline),
        setBrowserOffline(value: boolean) {
            browserOffline.value = value
        },
        markBrowserOnline() {
            browserOffline.value = false
        },
        refreshBrowserOffline() {
            browserOffline.value = typeof navigator !== 'undefined' && !navigator.onLine
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
