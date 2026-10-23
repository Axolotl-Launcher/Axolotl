import { computed, onScopeDispose, ref } from 'vue'

/**
 * Port of apps/app-frontend/src/components/ui/settings/network-diagnostics.ts.
 * The two commands it invokes, `plugin:settings|network_test` and
 * `plugin:settings|network_test_cancel`, come from the simulated backend.
 */
export const networkTargets = [
    { id: 'curseforge_api', name: 'CurseForge API', host: 'api.curseforge.com' },
    { id: 'modrinth_api', name: 'Modrinth API', host: 'api.modrinth.com' },
    { id: 'modrinth_cdn', name: 'Modrinth CDN', host: 'cdn.modrinth.com' },
    { id: 'mojang_api', name: 'Mojang API', host: 'api.mojang.com' },
    { id: 'minecraft_services', name: 'Minecraft Services API', host: 'api.minecraftservices.com' },
] as const

export type NetworkTarget = (typeof networkTargets)[number]['id']
export type NetworkMode = 'launcher' | 'direct'
export type ProbeState = 'idle' | 'queued' | 'running' | 'done' | 'failed' | 'cancelled'

export interface NetworkTestResult {
    target: NetworkTarget
    mode: NetworkMode
    tested_at: string
    launcher_version: string
    platform: string
    url: string
    proxy_mode: 'none' | 'system' | 'custom'
    dns_mode: 'system' | 'doh'
    tls_verification: boolean
    resolved_ips: string[]
    dns_ms: number
    dns_error: string | null
    peer_ip: string | null
    status: number | null
    response_ms: number | null
    total_ms: number
    http_version: string | null
    server: string | null
    served_by: string | null
    cache: string | null
    bytes_received: number
    sample_ms: number | null
    sample_mib_per_second: number | null
    error: string | null
}

export interface NetworkProbe {
    state: ProbeState
    result?: NetworkTestResult
}

export interface NetworkBackend {
    networkTest: (args: {
        requestId: string
        target: NetworkTarget
        mode: NetworkMode
    }) => Promise<NetworkTestResult | null>
    networkTestCancel: (args: { requestId: string }) => Promise<void>
}

export function resultStatus(result: NetworkTestResult) {
    if (result.error) return 'failed'
    if (result.status != null && result.status >= 200 && result.status < 300) return 'success'
    if (result.status === 401 || result.status === 403) return 'restricted'
    return 'http_error'
}

export function diagnosticReport(probes: Record<NetworkTarget, NetworkProbe>) {
    return JSON.stringify(
        {
            schema_version: 1,
            generated_at: new Date().toISOString(),
            tests: networkTargets.map((target) => ({
                target: target.id,
                state: probes[target.id].state,
                // Old results remain visible during retests, but must not be exported as new results.
                result: probes[target.id].state === 'done' ? probes[target.id].result : undefined,
            })),
        },
        null,
        2,
    )
}

function randomRequestId() {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
        return crypto.randomUUID()
    }
    return `${Date.now().toString(16)}-${Math.random().toString(16).slice(2)}`
}

export function useNetworkDiagnostics(backend: NetworkBackend) {
    const mode = ref<NetworkMode>('launcher')
    const probes = ref(
        Object.fromEntries(networkTargets.map(({ id }) => [id, { state: 'idle' }])) as Record<
            NetworkTarget,
            NetworkProbe
        >,
    )
    const requests = new Map<NetworkTarget, string>()
    const busy = computed(() =>
        Object.values(probes.value).some(({ state }) => state === 'running' || state === 'queued'),
    )
    const hasResults = computed(() =>
        Object.values(probes.value).some(({ state }) => state === 'done' || state === 'failed'),
    )
    let generation = 0

    async function probe(target: NetworkTarget, run: number, testMode: NetworkMode) {
        if (run !== generation) return
        const requestId = randomRequestId()
        requests.set(target, requestId)
        probes.value[target].state = 'running'
        try {
            const result = await backend.networkTest({
                requestId,
                target,
                mode: testMode,
            })
            if (run !== generation) return
            probes.value[target] = result
                ? { state: 'done', result }
                : { ...probes.value[target], state: 'cancelled' }
        } catch {
            if (run === generation) probes.value[target].state = 'failed'
        } finally {
            if (requests.get(target) === requestId) requests.delete(target)
        }
    }

    async function start(target?: NetworkTarget) {
        if (busy.value) return
        const run = ++generation
        const testMode = mode.value
        const targets = target ? [target] : networkTargets.map(({ id }) => id)
        targets.forEach((id) => {
            probes.value[id].state = 'queued'
        })
        // One transfer at a time avoids having probes compete for the user's bandwidth.
        for (const id of targets) {
            if (run !== generation) break
            await probe(id, run, testMode)
        }
    }

    function stop() {
        ++generation
        for (const requestId of requests.values()) {
            void backend.networkTestCancel({ requestId }).catch(() => {})
        }
        requests.clear()
        Object.values(probes.value).forEach((probe) => {
            if (probe.state === 'running' || probe.state === 'queued') probe.state = 'cancelled'
        })
    }

    onScopeDispose(stop)
    return { mode, probes, busy, hasResults, start, stop }
}
