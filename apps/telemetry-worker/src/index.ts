import { Hono } from 'hono'

import { ingestBatch, type IngestionLimits } from './ingest'
import { redact, truncateUtf8 } from './redact'
import { batchSchema, heartbeatDayWindow, type TelemetryBatch } from './schema'

const MAX_REQUEST_BYTES = 64 * 1024
const HARD_MAX_BATCHES_PER_INSTALLATION_PER_DAY = 25
const HARD_MAX_ACCEPTED_BATCHES_PER_DAY = 100_000
const DEFAULT_MAX_RESERVED_WRITES_PER_DAY = 60_000
const HARD_MAX_RESERVED_WRITES_PER_DAY = 80_000

export interface Bindings {
    DB: D1Database
    INSTALLATION_HMAC_SECRET: string
    INGEST_ENABLED?: string
    MAX_BATCHES_PER_INSTALLATION_PER_DAY?: string
    MAX_ACCEPTED_BATCHES_PER_DAY?: string
    MAX_RESERVED_WRITES_PER_DAY?: string
}

type Variables = { requestId: string }

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>()

app.use('*', async (context, next) => {
    context.set('requestId', crypto.randomUUID())
    await next()
    context.header('Cache-Control', 'no-store')
})

app.get('/health', (context) =>
    context.json({ status: 'ok', schema_version: 1 }, 200, { 'Cache-Control': 'no-store' }),
)

app.post('/v1/batch', async (context) => {
    const contentLength = Number(context.req.header('content-length') ?? '0')
    if (Number.isFinite(contentLength) && contentLength > MAX_REQUEST_BYTES) {
        return context.json({ error: 'request_too_large' }, 413)
    }

    const raw = await context.req.arrayBuffer()
    if (raw.byteLength > MAX_REQUEST_BYTES) return context.json({ error: 'request_too_large' }, 413)

    let input: unknown
    try {
        input = JSON.parse(new TextDecoder().decode(raw))
    } catch {
        return context.json({ error: 'invalid_json' }, 400)
    }

    const parsed = batchSchema.safeParse(input)
    if (!parsed.success) {
        // Do not accept or log legacy error payloads after error reporting was removed.
        if (containsLegacyErrorEvent(input))
            return context.json({ error: 'error_reporting_disabled' }, 400)
        return context.json(
            {
                error: 'invalid_batch',
                issues: parsed.error.issues.map((issue) => ({
                    path: issue.path.join('.'),
                    code: issue.code,
                })),
            },
            400,
        )
    }
    if (context.env.INGEST_ENABLED === 'false') {
        return context.json({ error: 'ingest_disabled' }, 503, { 'Retry-After': '60' })
    }
    if (!context.env.INSTALLATION_HMAC_SECRET || context.env.INSTALLATION_HMAC_SECRET.length < 32) {
        return context.json({ error: 'service_unavailable' }, 503)
    }

    try {
        const installationHash = await hmacInstallationId(
            context.env.INSTALLATION_HMAC_SECRET,
            parsed.data.installation_id,
        )
        const limits = ingestionLimits(context.env)
        const result = await ingestBatch(
            context.env.DB,
            sanitizeBatch(parsed.data),
            installationHash,
            limits,
        )
        if (result === 'duplicate') return context.json({ accepted: true, duplicate: true })
        if (result === 'conflict') return context.json({ error: 'batch_id_conflict' }, 409)
        if (result === 'installation_limit') {
            return context.json({ error: 'installation_batch_limit' }, 429, {
                'Retry-After': retryAfterSeconds().toString(),
            })
        }
        if (result === 'global_limit' || result === 'write_limit') {
            return context.json(
                { error: result === 'global_limit' ? 'global_batch_limit' : 'global_write_limit' },
                429,
                {
                    'Retry-After': retryAfterSeconds().toString(),
                },
            )
        }

        return context.json({ accepted: true, duplicate: false })
    } catch (error) {
        console.error('Telemetry ingestion failed', {
            requestId: context.get('requestId'),
            error: error instanceof Error ? error.message : String(error),
        })
        return context.json({ error: 'temporarily_unavailable' }, 503)
    }
})

app.notFound((context) => context.json({ error: 'not_found' }, 404))
app.onError((error, context) => {
    console.error('Unhandled telemetry worker error', {
        requestId: context.get('requestId'),
        error: error.message,
    })
    return context.json({ error: 'temporarily_unavailable' }, 503)
})

function containsLegacyErrorEvent(input: unknown): boolean {
    if (!input || typeof input !== 'object') return false
    const events = (input as { events?: unknown }).events
    return (
        Array.isArray(events) &&
        events.some(
            (event) =>
                Boolean(event) &&
                typeof event === 'object' &&
                (event as { type?: unknown }).type === 'error',
        )
    )
}

async function hmacInstallationId(secret: string, installationId: string): Promise<string> {
    const key = await crypto.subtle.importKey(
        'raw',
        new TextEncoder().encode(secret),
        { name: 'HMAC', hash: 'SHA-256' },
        false,
        ['sign'],
    )
    const signature = await crypto.subtle.sign(
        'HMAC',
        key,
        new TextEncoder().encode(installationId),
    )
    return [...new Uint8Array(signature)].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

function ingestionLimits(env: Bindings): IngestionLimits {
    return {
        installation: Math.min(
            positiveInteger(
                env.MAX_BATCHES_PER_INSTALLATION_PER_DAY,
                HARD_MAX_BATCHES_PER_INSTALLATION_PER_DAY,
            ),
            HARD_MAX_BATCHES_PER_INSTALLATION_PER_DAY,
        ),
        global: Math.min(
            positiveInteger(env.MAX_ACCEPTED_BATCHES_PER_DAY, HARD_MAX_ACCEPTED_BATCHES_PER_DAY),
            HARD_MAX_ACCEPTED_BATCHES_PER_DAY,
        ),
        writes: Math.min(
            positiveInteger(env.MAX_RESERVED_WRITES_PER_DAY, DEFAULT_MAX_RESERVED_WRITES_PER_DAY),
            HARD_MAX_RESERVED_WRITES_PER_DAY,
        ),
    }
}

function sanitizeBatch(batch: TelemetryBatch): TelemetryBatch {
    return {
        ...batch,
        app: {
            ...batch.app,
            version: truncateUtf8(redact(batch.app.version), 64),
            platform: truncateUtf8(redact(batch.app.platform), 32),
            arch: truncateUtf8(redact(batch.app.arch), 32),
        },
        events: batch.events,
    }
}

async function runMaintenance(db: D1Database): Promise<void> {
    const now = new Date()
    const yesterday = daysAgo(now, 1)
    const { oldest, latest } = heartbeatDayWindow(now)
    const dayMinus = (days: number) => daysAgo(now, days)

    // Usage-only maintenance. Error aggregate/R2 work was intentionally removed.
    await db.batch([
        db
            .prepare(
                `WITH active AS (
                    SELECT day, COUNT(*) AS count FROM daily_active
                    WHERE day BETWEEN ?1 AND ?2 GROUP BY day
                ), new_installs AS (
                    SELECT first_seen_day AS day, COUNT(*) AS count FROM installations
                    WHERE first_seen_day BETWEEN ?1 AND ?2 GROUP BY first_seen_day
                ), days AS (
                    SELECT ?2 AS day UNION SELECT day FROM active UNION SELECT day FROM new_installs
                    UNION SELECT day FROM daily_totals WHERE day BETWEEN ?1 AND ?2
                )
                INSERT INTO daily_totals (day, new_installations, active_installations)
                SELECT days.day, COALESCE(new_installs.count, 0), COALESCE(active.count, 0)
                FROM days LEFT JOIN active USING (day) LEFT JOIN new_installs USING (day)
                WHERE 1
                ON CONFLICT (day) DO UPDATE SET
                    new_installations = excluded.new_installations,
                    active_installations = excluded.active_installations
                WHERE daily_totals.new_installations <> excluded.new_installations
                    OR daily_totals.active_installations <> excluded.active_installations`,
            )
            .bind(oldest, yesterday),
        db
            .prepare(
                `DELETE FROM wau_seen WHERE NOT EXISTS (
                    SELECT 1 FROM daily_active da
                    WHERE da.installation_hash = wau_seen.installation_hash AND da.day BETWEEN ? AND ?
                )`,
            )
            .bind(dayMinus(6), latest),
        db
            .prepare(
                `DELETE FROM mau_seen WHERE NOT EXISTS (
                    SELECT 1 FROM daily_active da
                    WHERE da.installation_hash = mau_seen.installation_hash AND da.day BETWEEN ? AND ?
                )`,
            )
            .bind(dayMinus(29), latest),
        db.prepare('DELETE FROM daily_active WHERE day < ? OR day > ?').bind(oldest, latest),
        db.prepare("DELETE FROM accepted_batches WHERE accepted_at < unixepoch('now', '-8 days')"),
        db.prepare("DELETE FROM ingestion_daily WHERE day < date('now', '-8 days')"),
        db.prepare("DELETE FROM ingestion_global_daily WHERE day < date('now', '-8 days')"),
        db
            .prepare(
                "DELETE FROM daily_active_dims WHERE day < date('now', '-365 days') OR day > ?",
            )
            .bind(latest),
    ])
}

function daysAgo(now: Date, days: number): string {
    const date = new Date(now)
    date.setUTCDate(date.getUTCDate() - days)
    return date.toISOString().slice(0, 10)
}

function positiveInteger(value: string | undefined, fallback: number): number {
    const parsed = Number(value)
    return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback
}

function retryAfterSeconds(now = new Date()): number {
    const nextDay = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() + 1)
    return Math.max(1, Math.ceil((nextDay - now.getTime()) / 1_000))
}

export { app, hmacInstallationId, runMaintenance, sanitizeBatch }

export default {
    fetch: app.fetch,
    async scheduled(_controller: ScheduledController, env: Bindings, context: ExecutionContext) {
        context.waitUntil(runMaintenance(env.DB))
    },
} satisfies ExportedHandler<Bindings>
