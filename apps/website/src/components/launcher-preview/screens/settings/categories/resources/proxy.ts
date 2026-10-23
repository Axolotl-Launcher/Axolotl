import { reactive } from 'vue'

import { usePreviewDomain } from '../../../../lib/state/index'
import { type AllSettings, type ProxyMode, useSettingsPage } from '../../state'

/** `ProxyConfig` (helpers/settings.ts, app-lib `util/proxy.rs`). */
export interface ProxyConfig {
    mode: ProxyMode
    url: string
    username: string
    password: string
}

/** `ProxyTestResult`. */
export interface ProxyTestResult {
    success: boolean
    latency_ms: number | null
    message: string
}

/** A serialized `TheseusSerializableError::Theseus`, what a failed `invoke` rejects with. */
export interface TheseusError {
    field_name: 'Theseus'
    message: string
}

export function theseusError(message: string): TheseusError {
    return { field_name: 'Theseus', message }
}

/** `ProxyConfig::default()`: the settings table's `proxy_mode` defaults to `system`. */
export const DEFAULT_PROXY_CONFIG: ProxyConfig = {
    mode: 'system',
    url: '',
    username: '',
    password: '',
}

/** The merged settings object, where the stored proxy is published for other pages. */
type SettingsWithProxy = AllSettings & { proxy_config?: ProxyConfig }

const PROXY_SCHEMES = ['http', 'https', 'socks4', 'socks5', 'socks5h']
const SCHEME_PATTERN = /^[a-zA-Z][a-zA-Z0-9+.-]*:/

/**
 * The `url::ParseError` text for input the WHATWG parser rejects (both follow
 * the URL Standard), or null when it parses.
 */
function urlParseError(input: string): string | null {
    try {
        new URL(input)
        return null
    } catch {
        // classified below
    }
    if (!SCHEME_PATTERN.test(input)) return 'relative URL without a base'
    const authority = (
        input.replace(SCHEME_PATTERN, '').replace(/^\/*/, '').split(/[/?#]/)[0] ?? ''
    ).replace(/^.*@/, '')
    if (!authority) return 'empty host'
    if (authority.startsWith('[')) return 'invalid IPv6 address'
    const portIndex = authority.lastIndexOf(':')
    const host = portIndex >= 0 ? authority.slice(0, portIndex) : authority
    const port = portIndex >= 0 ? authority.slice(portIndex + 1) : ''
    if (port && (!/^\d+$/.test(port) || Number(port) > 65535)) return 'invalid port number'
    if (!host) return 'empty host'
    if (/^[\d.]+$/.test(host)) return 'invalid IPv4 address'
    return 'invalid domain character'
}

/** `ProxyConfig::validate`, with the `Invalid input:` prefix of `ErrorKind::InputError`. */
export function validateProxyConfig(config: ProxyConfig): string | null {
    if (config.mode !== 'custom') return null
    const url = config.url.trim()
    if (!url) return 'Invalid input: Custom proxy URL is required when mode is Custom'
    const parseError = urlParseError(url)
    if (parseError) return `Invalid input: Proxy URL is invalid: ${parseError}`
    const scheme = url.slice(0, url.indexOf(':')).toLowerCase()
    if (!PROXY_SCHEMES.includes(scheme)) {
        return `Invalid input: Unsupported proxy scheme '${scheme}'. Use http, https, socks5, or socks5h.`
    }
    // `reqwest::Proxy::all` needs a proxy host to connect to.
    if (!new URL(url).hostname) return 'Invalid input: Proxy URL is invalid: builder error'
    return null
}

function isLocalHost(hostname: string) {
    return hostname === 'localhost' || hostname === '[::1]' || /^127\./.test(hostname)
}

/**
 * The launcher's stored proxy (`proxy_get` / `proxy_set` / `proxy_test` of
 * apps/app/src/api/settings.rs). It is not part of `AppSettings`; it lives in
 * this domain and is published as `settings.proxy_config` for the network
 * diagnostics page.
 */
export function useProxySettings() {
    const { settings, preview, simulate } = useSettingsPage()
    const merged = settings as SettingsWithProxy
    const stored = usePreviewDomain('settings:proxy', () => {
        const config = reactive<ProxyConfig>({ ...DEFAULT_PROXY_CONFIG, ...merged.proxy_config })
        merged.proxy_config = { ...config }
        return { value: config }
    })

    async function getProxyConfig(): Promise<ProxyConfig> {
        await simulate(40)
        return { ...stored }
    }

    async function setProxyConfig(config: ProxyConfig): Promise<void> {
        const snapshot = { ...config }
        await simulate(60)
        const invalid = validateProxyConfig(snapshot)
        if (invalid) throw theseusError(invalid)
        Object.assign(stored, snapshot)
        merged.proxy_config = { ...snapshot }
    }

    /** Fetches `http://connect.rom.miui.com/generate_204` through the configuration. */
    async function testProxyConfig(config: ProxyConfig): Promise<ProxyTestResult> {
        const snapshot = { ...config }
        const invalid = validateProxyConfig(snapshot)
        if (invalid) {
            await simulate(30)
            return { success: false, latency_ms: null, message: invalid }
        }
        // `latency_ms` is the whole request: connecting (through the proxy) and the reply.
        const random = (min: number, max: number) => min + Math.round(Math.random() * (max - min))
        let elapsed = random(90, 210)
        if (snapshot.mode === 'custom') {
            elapsed = isLocalHost(new URL(snapshot.url.trim()).hostname)
                ? random(140, 320)
                : random(220, 480)
        }
        await new Promise<void>((resolve) => preview.clock.after(elapsed, resolve))
        return {
            success: true,
            latency_ms: elapsed,
            message: `Connection successful (${elapsed} ms)`,
        }
    }

    return { getProxyConfig, setProxyConfig, testProxyConfig }
}
