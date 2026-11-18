import { invoke as tauriInvoke } from '@tauri-apps/api/core'

/**
 * Frontend telemetry bridge.
 *
 * Events are queued and sent in small batches to the native observability
 * plugin. The native side re-validates every field against its own
 * allowlist, so this module only removes obviously unusable values and keeps
 * the wire format small. Telemetry is best effort: when the pipeline is
 * disabled or a send fails, events are dropped without affecting the app.
 */

export type ObservabilityEventName =
    | 'page_view'
    | 'command'
    | 'ui_error'
    | 'instance_create'
    | 'instance_start'
    | 'instance_stop'
    | 'project_install'
    | 'pack_install'
    | 'account_login'
    | 'account_logout'
    | 'theme_apply'
    | 'app_update'

export type ObservabilityAttributeKey =
    | 'route.name'
    | 'name'
    | 'outcome'
    | 'error.kind'
    | 'ui.area'
    | 'source'
    | 'instance.loader'
    | 'instance.game_version'
    | 'exit.code'
    | 'pack.id'
    | 'update.channel'

export type ObservabilityValue = string | boolean | number

export type ObservabilityErrorKind =
    'vue_error' | 'router_error' | 'window_error' | 'unhandled_rejection' | 'ipc_error' | 'unknown'

export type ObservabilityArea =
    | 'startup'
    | 'router'
    | 'ipc'
    | 'instance'
    | 'install'
    | 'account'
    | 'theme'
    | 'updater'
    | 'settings'
    | 'unknown'

interface ObservabilityStatus {
    enabled: boolean
    signals?: string[]
    session_id?: string | null
}

interface ObservabilityEvent {
    name: ObservabilityEventName
    attributes?: Record<string, ObservabilityValue>
    duration_ms?: number
}

/**
 * Attribute keys accepted for each event. Mirrors the native allowlist; keys
 * carrying user content (server names, instance names, paths, URLs) are
 * deliberately absent.
 */
export const OBSERVABILITY_EVENT_ATTRIBUTES = {
    page_view: ['route.name'],
    command: ['name', 'outcome', 'error.kind'],
    ui_error: ['error.kind', 'ui.area'],
    instance_create: ['source', 'instance.loader', 'instance.game_version'],
    instance_start: ['source', 'instance.loader', 'instance.game_version'],
    instance_stop: ['source', 'instance.loader', 'instance.game_version', 'exit.code'],
    project_install: ['source', 'instance.loader', 'instance.game_version'],
    pack_install: ['source'],
    account_login: ['source'],
    account_logout: [],
    theme_apply: ['pack.id', 'ui.area', 'source'],
    app_update: ['source', 'update.channel'],
} as const satisfies Record<ObservabilityEventName, readonly ObservabilityAttributeKey[]>

const MAX_QUEUE = 64
const MAX_BATCH = 20
const FLUSH_INTERVAL_MS = 5_000
const MAX_ATTRIBUTES = 8
const MAX_VALUE_LENGTH = 64
const MAX_DURATION_MS = 3_600_000
const IDENTIFIER_PATTERN = /^[A-Za-z0-9_.:-]+$/
const COMMAND_NAME_PATTERN = /^[A-Za-z0-9_.:|-]+$/
const ROUTE_NAME_PATTERN = /^[A-Za-z0-9_.:-]+$/
const AREA_PATTERN = /^[a-z_]+$/

let ready = false
let enabled = false
let pending: ObservabilityEvent[] = []
let flushTimer: ReturnType<typeof setInterval> | null = null
let flushInFlight: Promise<void> | null = null

/**
 * Records a bounded event. Safe to call before initialization: events are
 * queued and either flushed once the status resolves or dropped when
 * observability is disabled.
 */
export function recordEvent(
    name: ObservabilityEventName,
    attributes: Partial<Record<ObservabilityAttributeKey, ObservabilityValue>> = {},
    durationMs?: number,
): void {
    if (ready && !enabled) {
        return
    }
    enqueue(buildEvent(name, attributes, durationMs))
}

export function recordNavigation(routeName: unknown): void {
    if (typeof routeName !== 'string') {
        return
    }
    const name = routeName.trim()
    if (name.length === 0 || name.length > MAX_VALUE_LENGTH || !ROUTE_NAME_PATTERN.test(name)) {
        return
    }
    recordEvent('page_view', { 'route.name': name })
}

export function recordError(kind: ObservabilityErrorKind, area?: ObservabilityArea): void {
    const attributes: Partial<Record<ObservabilityAttributeKey, ObservabilityValue>> = {
        'error.kind': kind,
    }
    if (area !== undefined && AREA_PATTERN.test(area)) {
        attributes['ui.area'] = area
    }
    recordEvent('ui_error', attributes)
}

/**
 * Resolves whether telemetry is enabled and installs the batch timer. Called
 * once from the app entry point; repeated calls are ignored.
 */
export async function initObservability(): Promise<boolean> {
    if (ready) {
        return enabled
    }
    try {
        const status = await tauriInvoke<ObservabilityStatus | null>(
            'plugin:observability|observability_status',
        )
        enabled = status?.enabled === true
    } catch {
        enabled = false
    }
    ready = true
    if (!enabled) {
        pending = []
        return false
    }
    flushTimer = setInterval(() => {
        void flushQueue()
    }, FLUSH_INTERVAL_MS)
    if (typeof window !== 'undefined') {
        window.addEventListener('pagehide', handlePageHide)
    }
    void flushQueue()
    return true
}

/** Clears the batch timer and queued events. Used on teardown and in tests. */
export function resetObservability(): void {
    if (flushTimer !== null) {
        clearInterval(flushTimer)
        flushTimer = null
    }
    if (typeof window !== 'undefined') {
        window.removeEventListener('pagehide', handlePageHide)
    }
    ready = false
    enabled = false
    pending = []
}

function buildEvent(
    name: ObservabilityEventName,
    attributes: Partial<Record<ObservabilityAttributeKey, ObservabilityValue>>,
    durationMs?: number,
): ObservabilityEvent {
    const event: ObservabilityEvent = { name }
    const allowed = OBSERVABILITY_EVENT_ATTRIBUTES[name]
    const sanitized: Record<string, ObservabilityValue> = {}
    let attributeCount = 0
    for (const [key, value] of Object.entries(attributes)) {
        if (attributeCount >= MAX_ATTRIBUTES) {
            break
        }
        if (!(allowed as readonly string[]).includes(key)) {
            continue
        }
        const sanitizedValue = sanitizeValue(key, value)
        if (sanitizedValue === undefined) {
            continue
        }
        sanitized[key] = sanitizedValue
        attributeCount += 1
    }
    if (attributeCount > 0) {
        event.attributes = sanitized
    }
    if (durationMs !== undefined && Number.isFinite(durationMs) && durationMs >= 0) {
        event.duration_ms = Math.min(Math.round(durationMs), MAX_DURATION_MS)
    }
    return event
}

function sanitizeValue(
    key: string,
    value: ObservabilityValue | undefined,
): ObservabilityValue | undefined {
    if (typeof value === 'string') {
        return sanitizeIdentifier(value, key === 'name') ?? undefined
    }
    if (typeof value === 'boolean') {
        return value
    }
    if (typeof value === 'number' && Number.isFinite(value)) {
        return value
    }
    return undefined
}

function sanitizeIdentifier(value: unknown, allowPipe = false): string | null {
    if (typeof value !== 'string') {
        return null
    }
    const trimmed = value.trim()
    if (trimmed.length === 0 || trimmed.length > MAX_VALUE_LENGTH) {
        return null
    }
    const pattern = allowPipe ? COMMAND_NAME_PATTERN : IDENTIFIER_PATTERN
    return pattern.test(trimmed) ? trimmed : null
}

function enqueue(event: ObservabilityEvent): void {
    if (pending.length >= MAX_QUEUE) {
        pending.shift()
    }
    pending.push(event)
}

function flushQueue(): Promise<void> {
    if (flushInFlight !== null) {
        return flushInFlight
    }
    if (!enabled || pending.length === 0) {
        return Promise.resolve()
    }
    flushInFlight = sendBatches().finally(() => {
        flushInFlight = null
    })
    return flushInFlight
}

async function drainQueue(): Promise<void> {
    while (pending.length > 0) {
        await flushQueue()
    }
}

async function sendBatches(): Promise<void> {
    // Bound the loop: events may keep arriving while batches are in flight.
    for (let sent = 0; sent < MAX_QUEUE && pending.length > 0; sent += MAX_BATCH) {
        const batch = pending.splice(0, MAX_BATCH)
        try {
            await tauriInvoke('plugin:observability|observability_record', { events: batch })
        } catch {
            // Telemetry is best effort; drop the batch instead of retrying.
            break
        }
    }
}

function handlePageHide(): void {
    // Flush native exporters only after the final batch has been delivered.
    void drainQueue().finally(() => {
        void tauriInvoke('plugin:observability|observability_flush').catch(() => {})
    })
}
