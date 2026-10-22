import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { listen } from '@tauri-apps/api/event'
import { computed, onScopeDispose, readonly, type Ref, shallowRef, watch } from 'vue'

import {
    applyMojangAuthSource,
    getMojangAuthConfiguration,
    getNetworkScope,
    probeMirrorMojangServices,
    probeOfficialMojangServices,
    probeSelectedMojangSource,
} from '@/helpers/mojang-auth'
import { createMojangAuthMonitor, mojangAuthKeys } from '@/helpers/mojang-auth/monitor'

import { useNetworkStatus } from './useNetworkStatus'

const reachable = shallowRef<boolean>()

/** Consumers read the application monitor; entering a page never starts a second query. */
export function useMojangAuthStatus() {
    return { reachable: readonly(reachable) }
}

/** Owned once by App, enabled only after the local application shell is ready. */
export function setupMojangAuthMonitor(ready: Ref<boolean>) {
    const queryClient = useQueryClient()
    const { browserOffline } = useNetworkStatus()
    const enabled = computed(() => ready.value && !browserOffline.value)
    const monitor = createMojangAuthMonitor({
        configuration: getMojangAuthConfiguration,
        probeOfficial: probeOfficialMojangServices,
        probeMirror: probeMirrorMojangServices,
        applySource: applyMojangAuthSource,
        probeSelected: probeSelectedMojangSource,
        enabled: () => enabled.value,
        publish(value) {
            reachable.value = value
        },
    })
    const query = useQuery({
        queryKey: mojangAuthKeys.status,
        queryFn: () => monitor.run(),
        enabled,
        // Slow or failing probes are retried by this one scheduler, without three full probe rounds.
        refetchInterval: (query) =>
            query.state.status === 'error' || query.state.data === false ? 15_000 : 5 * 60_000,
        retry: false,
        refetchOnWindowFocus: false,
        refetchOnReconnect: false,
        networkMode: 'always',
    })
    const network = useQuery({
        queryKey: ['mojang', 'network', 'scope'],
        queryFn: getNetworkScope,
        enabled: ready,
        refetchInterval: 5_000,
        retry: false,
        networkMode: 'always',
        refetchIntervalInBackground: true,
    })
    watch(network.data, (scope, previous) => {
        if (previous === undefined || scope === previous || !enabled.value) return
        monitor.invalidate()
        void query.refetch({ cancelRefetch: false })
    })
    watch(browserOffline, () => monitor.invalidate(), { flush: 'sync' })
    watch(query.error, (error) => {
        if (error) console.warn('Mojang background check failed', error)
    })

    let disposed = false
    let unlisten: (() => void) | undefined
    void listen('settings', () => {
        if (!enabled.value || disposed) return
        void monitor
            .configurationChanged()
            .then((changed) => {
                if (!changed || disposed) return
                monitor.invalidate()
                void query.refetch()
            })
            .catch(console.error)
    })
        .then((dispose) => {
            if (disposed) dispose()
            else unlisten = dispose
        })
        .catch(console.error)
    onScopeDispose(() => {
        disposed = true
        monitor.dispose()
        void queryClient.cancelQueries({ queryKey: mojangAuthKeys.status, exact: true })
        unlisten?.()
    })
}
