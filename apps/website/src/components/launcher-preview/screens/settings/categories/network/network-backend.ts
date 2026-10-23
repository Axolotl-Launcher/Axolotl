import type { PreviewClock } from '../../../../lib/clock'
import { PREVIEW_APP_VERSION } from '../../state'
import type {
    NetworkBackend,
    NetworkMode,
    NetworkTarget,
    NetworkTestResult,
} from './network-diagnostics'

/**
 * `plugin:settings|network_test`, after `probe_connection` in
 * packages/app-lib/src/util/network_diagnostics.rs: resolve the host with the
 * launcher's resolver (both families, IPv6 and IPv4 interleaved), send one GET
 * through a fresh client that does not follow redirects, and on the CDN read at
 * most an 8 MiB sample within 3 seconds. The command resolves after the probe's
 * own duration on the window clock and with `None` when it is cancelled.
 *
 * The Mac is on an IPv4-only home network with no system proxy configured, so
 * connections go straight to the first IPv4 address. Addresses are drawn from
 * the documentation ranges (RFC 5737 / RFC 3849): they stand for whatever the
 * visitor's resolver would return, not for the services' real addresses.
 */
interface ProbeProfile {
    url: string
    v4: string[]
    v6: string[]
    /** Time to response headers on a fresh connection (TCP + TLS + request), ms. */
    response: [number, number]
    status: number
}

const PROFILES: Record<NetworkTarget, ProbeProfile> = {
    curseforge_api: {
        url: 'https://api.curseforge.com/v1/games/432',
        v4: ['192.0.2.41', '192.0.2.57'],
        v6: ['2001:db8:cf::29', '2001:db8:cf::39'],
        response: [238, 412],
        status: 200,
    },
    modrinth_api: {
        url: 'https://api.modrinth.com/v2/tag/game_version',
        v4: ['198.51.100.23', '198.51.100.24'],
        v6: ['2001:db8:4d52::17', '2001:db8:4d52::18'],
        response: [176, 328],
        status: 200,
    },
    modrinth_cdn: {
        url: 'https://cdn.modrinth.com/data/uCdwusMi/versions/13xaVT4W/DistantHorizons-2.1.0-a-1.20.6-noForge.jar',
        v4: ['198.51.100.71', '198.51.100.72'],
        v6: ['2001:db8:4d52::47', '2001:db8:4d52::48'],
        response: [118, 236],
        // A `Range: bytes=0-8388607` request.
        status: 206,
    },
    mojang_api: {
        url: 'https://api.mojang.com/users/profiles/minecraft/Notch',
        v4: ['203.0.113.14'],
        v6: ['2001:db8:6d6a::e'],
        response: [284, 466],
        status: 200,
    },
    minecraft_services: {
        url: 'https://api.minecraftservices.com/publickeys',
        v4: ['203.0.113.36', '203.0.113.37'],
        v6: ['2001:db8:6d73::24'],
        response: [262, 448],
        status: 200,
    },
}

/** `DOWNLOAD_SAMPLE_BYTES`. */
const DOWNLOAD_SAMPLE_BYTES = 8 * 1024 * 1024
/** `std::env::consts::{OS, ARCH}` of an Apple silicon Mac. */
const PLATFORM = 'macos-aarch64'

export interface ProxyConfig {
    mode: 'none' | 'system' | 'custom'
    url: string
}

export interface NetworkConfig {
    proxy: ProxyConfig
    dohEnabled: boolean
    ignoreSslErrors: boolean
}

function between([min, max]: [number, number]) {
    return Math.round(min + Math.random() * (max - min))
}

/** `order_addresses`: IPv6 and IPv4 alternate, IPv6 first. */
function interleave(v6: string[], v4: string[]) {
    const ordered: string[] = []
    for (let index = 0; index < Math.max(v6.length, v4.length); index++) {
        if (v6[index]) ordered.push(v6[index])
        if (v4[index]) ordered.push(v4[index])
    }
    return ordered
}

/** The transport peer through a custom proxy is the proxy itself. */
function proxyPeer(url: string) {
    try {
        const host = new URL(url.includes('://') ? url : `http://${url}`).hostname.replace(
            /^\[|\]$/g,
            '',
        )
        if (host === 'localhost') return '127.0.0.1'
        if (/^\d{1,3}(\.\d{1,3}){3}$/.test(host) || host.includes(':')) return host
    } catch {
        // An unparsable URL is rejected when the proxy is saved; fall through.
    }
    return '192.0.2.200'
}

/** `chrono::Utc::now().to_rfc3339()`: nanosecond fraction and a numeric offset. */
function rfc3339Now() {
    const iso = new Date().toISOString()
    const nanos = String(Math.floor(Math.random() * 1_000_000)).padStart(6, '0')
    return `${iso.slice(0, 23)}${nanos}+00:00`
}

function buildResult(
    target: NetworkTarget,
    mode: NetworkMode,
    config: NetworkConfig,
): NetworkTestResult {
    const profile = PROFILES[target]
    const proxy: ProxyConfig = mode === 'launcher' ? config.proxy : { mode: 'none', url: '' }
    const viaCustomProxy = proxy.mode === 'custom' && proxy.url.trim().length > 0
    // DNS over HTTPS opens its own TLS connection for each fresh resolver.
    const dnsMs = config.dohEnabled ? between([38, 126]) : between([4, 28])
    const responseMs = between(profile.response) + (viaCustomProxy ? between([24, 70]) : 0)
    const clientMs = between([3, 9])

    let sampleMs: number | null = null
    let sampleMibPerSecond: number | null = null
    let bytesReceived = 0
    if (target === 'modrinth_cdn') {
        const speed = 8.5 + Math.random() * 11
        sampleMs = Math.round((DOWNLOAD_SAMPLE_BYTES / 1048576 / speed) * 1000)
        bytesReceived = DOWNLOAD_SAMPLE_BYTES
        sampleMibPerSecond = bytesReceived / 1048576 / (sampleMs / 1000)
    }

    return {
        target,
        mode,
        tested_at: rfc3339Now(),
        launcher_version: PREVIEW_APP_VERSION,
        platform: PLATFORM,
        url: profile.url,
        proxy_mode: proxy.mode,
        dns_mode: config.dohEnabled ? 'doh' : 'system',
        tls_verification: !config.ignoreSslErrors,
        resolved_ips: interleave(profile.v6, profile.v4),
        dns_ms: dnsMs,
        dns_error: null,
        peer_ip: viaCustomProxy ? proxyPeer(proxy.url) : profile.v4[0],
        status: profile.status,
        response_ms: responseMs,
        total_ms: clientMs + dnsMs + responseMs + (sampleMs ?? 0),
        http_version: 'HTTP/2.0',
        server: null,
        served_by: null,
        cache: null,
        bytes_received: bytesReceived,
        sample_ms: sampleMs,
        sample_mib_per_second: sampleMibPerSecond,
        error: null,
    }
}

export function createNetworkBackend(
    clock: PreviewClock,
    readConfig: () => NetworkConfig,
): NetworkBackend {
    const pending = new Map<
        string,
        { cancel: () => void; resolve: (result: NetworkTestResult | null) => void }
    >()

    return {
        networkTest({ requestId, target, mode }) {
            return new Promise((resolve) => {
                const result = buildResult(target, mode, readConfig())
                // The IPC round trip on top of the probe itself.
                const cancel = clock.after(result.total_ms + 12, () => {
                    pending.delete(requestId)
                    resolve(result)
                })
                pending.set(requestId, { cancel, resolve })
            })
        },
        async networkTestCancel({ requestId }) {
            const entry = pending.get(requestId)
            if (!entry) return
            pending.delete(requestId)
            entry.cancel()
            entry.resolve(null)
        },
    }
}
