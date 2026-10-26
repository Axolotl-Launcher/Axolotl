import { env } from 'cloudflare:test'
import { describe, expect, it } from 'vitest'

import { app, hmacInstallationId, runMaintenance, type Bindings } from '../src'

function dayAgo(days: number): string {
    return new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 10)
}

function payload(days = [dayAgo(0)], installation = crypto.randomUUID()) {
    return {
        schema_version: 1,
        batch_id: crypto.randomUUID(),
        installation_id: installation,
        app: { version: '1.10.1', environment: 'production', platform: 'windows', arch: 'x86_64' },
        events: days.map((day) => ({
            type: 'heartbeat',
            event_id: crypto.randomUUID(),
            occurred_at: `${day}T12:00:00.000Z`,
            day,
        })),
    }
}

function tracker(onStateRead?: () => Promise<void>) {
    const stats = { rowsWritten: 0, prepared: 0, batches: 0 }
    const originals = new WeakMap<D1PreparedStatement, D1PreparedStatement>()
    function wrap(statement: D1PreparedStatement, sql: string): D1PreparedStatement {
        const proxy = new Proxy(statement, {
            get(target, key) {
                if (key === 'bind')
                    return (...values: unknown[]) => wrap(target.bind(...values), sql)
                if (key === 'first')
                    return async (column?: string) => {
                        const result =
                            column === undefined ? await target.first() : await target.first(column)
                        if (sql.includes('AS batch_installation')) await onStateRead?.()
                        return result
                    }
                const value = Reflect.get(target, key)
                return typeof value === 'function' ? value.bind(target) : value
            },
        })
        originals.set(proxy, statement)
        return proxy
    }
    const db = {
        prepare(sql: string) {
            stats.prepared++
            return wrap(env.DB.prepare(sql), sql)
        },
        async batch(statements: D1PreparedStatement[]) {
            stats.batches++
            const results = await env.DB.batch(
                statements.map((statement) => originals.get(statement) ?? statement),
            )
            stats.rowsWritten += results.reduce((sum, result) => sum + result.meta.rows_written, 0)
            return results
        },
    } as D1Database
    return { db, stats }
}

function rendezvous() {
    let arrivals = 0
    let release!: () => void
    const ready = new Promise<void>((resolve) => {
        release = resolve
    })
    return async () => {
        arrivals++
        if (arrivals === 2) release()
        if (arrivals <= 2) await ready
    }
}

async function post(body: unknown, db = env.DB, overrides: Partial<Bindings> = {}) {
    return app.fetch(
        new Request('https://telemetry.example/v1/batch', {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify(body),
        }),
        { ...env, ...overrides, DB: db },
    )
}

async function count(table: string) {
    return (await env.DB.prepare(`SELECT COUNT(*) AS count FROM ${table}`).first<{
        count: number
    }>())!.count
}

describe('heartbeat ingestion efficiency and compatibility', () => {
    it.each([
        'error',
        'mixed_error',
        'crash',
        'event_context',
        'download_stats',
        'app_context',
        'batch_context',
    ])('rejects retired or extra payload %s before database access', async (kind) => {
        const body = payload()
        const error = {
            type: 'error',
            event_id: crypto.randomUUID(),
            occurred_at: new Date().toISOString(),
            fingerprint: 'a'.repeat(64),
            occurrence_count: 1,
            error_type: 'legacy',
            message: 'never retained',
            context: 'never retained',
        }
        const invalid =
            kind === 'error'
                ? { ...body, events: [error] }
                : kind === 'mixed_error'
                  ? { ...body, events: [...body.events, error] }
                  : kind === 'crash'
                    ? { ...body, events: [{ ...body.events[0], type: 'crash' }] }
                    : kind === 'event_context'
                      ? { ...body, events: [{ ...body.events[0], context: 'never retained' }] }
                      : kind === 'download_stats'
                        ? { ...body, events: [{ ...body.events[0], download_stats: {} }] }
                        : kind === 'app_context'
                          ? { ...body, app: { ...body.app, context: {} } }
                          : { ...body, context: {} }
        const tracked = tracker()
        const response = await post(invalid, tracked.db)
        expect(response.status).toBe(400)
        expect(tracked.stats).toEqual({ rowsWritten: 0, prepared: 0, batches: 0 })
        if (kind.includes('error'))
            expect(await response.json()).toEqual({ error: 'error_reporting_disabled' })
    })

    it.each(['9999-99-99', '2026-02-30', dayAgo(-1), dayAgo(36)])(
        'rejects invalid or out-of-retention day %s without storage access',
        async (day) => {
            const tracked = tracker()
            expect((await post(payload([day]), tracked.db)).status).toBe(400)
            expect(tracked.stats.prepared).toBe(0)
        },
    )

    it('requires day to match occurred_at in UTC, including offset timestamps', async () => {
        const invalid = payload([dayAgo(1)])
        invalid.events[0].occurred_at = `${dayAgo(0)}T00:00:00.000Z`
        const tracked = tracker()
        expect((await post(invalid, tracked.db)).status).toBe(400)
        expect(tracked.stats.prepared).toBe(0)

        const valid = payload([dayAgo(0)])
        valid.events[0].occurred_at = `${dayAgo(0)}T08:00:00.000+08:00`
        expect((await post(valid)).status).toBe(200)
    })

    it('retains the v1 shape, additive metadata values, and 35-day offline window', async () => {
        const body = payload([dayAgo(35), dayAgo(29), dayAgo(7), dayAgo(0)])
        body.app.version = '1.10.1+compatible-client'
        body.app.platform = 'future-platform'
        body.app.arch = 'future-arch'
        const tracked = tracker()
        const response = await post(body, tracked.db)
        expect(response.status).toBe(200)
        expect(await response.json()).toEqual({ accepted: true, duplicate: false })
        expect(await count('daily_active')).toBe(4)
        const reserved = await env.DB.prepare(
            'SELECT reserved_writes FROM ingestion_global_daily',
        ).first<{ reserved_writes: number }>()
        expect(reserved!.reserved_writes).toBeGreaterThanOrEqual(tracked.stats.rowsWritten)
    })

    it('does not write for retries or new batch IDs covering an already recorded day', async () => {
        const body = payload()
        expect((await post(body)).status).toBe(200)
        for (const request of [
            body,
            { ...body, batch_id: crypto.randomUUID(), events: payload().events },
        ]) {
            const tracked = tracker()
            const response = await post(request, tracked.db)
            expect(await response.json()).toEqual({ accepted: true, duplicate: true })
            expect(tracked.stats.rowsWritten).toBe(0)
            expect(tracked.stats.batches).toBe(0)
        }
        expect(await count('accepted_batches')).toBe(1)
        const totals = await env.DB.prepare(
            'SELECT accepted_batches FROM ingestion_global_daily',
        ).first<{ accepted_batches: number }>()
        expect(totals!.accepted_batches).toBe(1)
    })

    it('deduplicates days within and across batches without inflating rollups', async () => {
        const body = payload([dayAgo(0), dayAgo(0)])
        expect((await post(body)).status).toBe(200)
        expect(
            (
                await post({
                    ...body,
                    batch_id: crypto.randomUUID(),
                    events: payload([dayAgo(0), dayAgo(1), dayAgo(1)]).events,
                })
            ).status,
        ).toBe(200)
        expect(await count('daily_active')).toBe(2)
        expect(await count('daily_active_dims')).toBe(6)
        const counts = await env.DB.prepare(
            'SELECT MIN(install_count) AS min, MAX(install_count) AS max FROM daily_active_dims',
        ).first()
        expect(counts).toEqual({ min: 1, max: 1 })
        const totals = await env.DB.prepare(
            'SELECT accepted_batches, reserved_writes FROM ingestion_global_daily',
        ).first()
        expect(totals).toEqual({ accepted_batches: 2, reserved_writes: 32 })
    })

    it('returns an existing duplicate even after quotas are exhausted', async () => {
        const body = payload()
        expect((await post(body)).status).toBe(200)
        await env.DB.prepare('UPDATE ingestion_daily SET accepted_batches = 25').run()
        await env.DB.prepare(
            'UPDATE ingestion_global_daily SET accepted_batches = 100000, reserved_writes = 80000',
        ).run()
        const tracked = tracker()
        expect(
            await (await post({ ...body, batch_id: crypto.randomUUID() }, tracked.db)).json(),
        ).toEqual({ accepted: true, duplicate: true })
        expect(tracked.stats.rowsWritten).toBe(0)
    })

    it('rejects reuse of another installation batch ID without writing', async () => {
        const body = payload()
        expect((await post(body)).status).toBe(200)
        const tracked = tracker()
        const response = await post({ ...body, installation_id: crypto.randomUUID() }, tracked.db)
        expect(response.status).toBe(409)
        expect(tracked.stats.rowsWritten).toBe(0)
    })

    it.each(['installation', 'global', 'writes'] as const)(
        'does not reserve, update, delete, or leave identity rows on %s rejection',
        async (kind) => {
            const body = payload()
            if (kind === 'installation') {
                const hash = await hmacInstallationId(
                    env.INSTALLATION_HMAC_SECRET,
                    body.installation_id,
                )
                await env.DB.prepare('INSERT INTO ingestion_daily VALUES (?, ?, 25)')
                    .bind(dayAgo(0), hash)
                    .run()
            } else {
                await env.DB.prepare(
                    'INSERT INTO ingestion_global_daily (day, accepted_batches, reserved_writes) VALUES (?, ?, ?)',
                )
                    .bind(dayAgo(0), kind === 'global' ? 100000 : 0, kind === 'writes' ? 60000 : 0)
                    .run()
            }
            const tracked = tracker()
            const response = await post(body, tracked.db)
            expect(response.status).toBe(429)
            expect(response.headers.get('Retry-After')).not.toBeNull()
            expect(tracked.stats.rowsWritten).toBe(0)
            expect(tracked.stats.batches).toBe(0)
            expect(await count('accepted_batches')).toBe(0)
            expect(await count('installations')).toBe(0)
            expect(await count('ingestion_daily')).toBe(kind === 'installation' ? 1 : 0)
        },
    )

    it('reserves enough for the whole batch and rejects without partial persistence', async () => {
        const tracked = tracker()
        const response = await post(payload([dayAgo(0), dayAgo(1), dayAgo(2)]), tracked.db, {
            MAX_RESERVED_WRITES_PER_DAY: '31',
        })
        expect(response.status).toBe(429)
        expect(await response.json()).toEqual({ error: 'global_write_limit' })
        expect(tracked.stats.rowsWritten).toBe(0)
        expect(await count('daily_active')).toBe(0)
        expect(await count('ingestion_global_daily')).toBe(0)
    })

    it('caps configured write budgets even when an operator supplies an excessive value', async () => {
        await env.DB.prepare(
            'INSERT INTO ingestion_global_daily (day, accepted_batches, reserved_writes) VALUES (?, 1, 79990)',
        )
            .bind(dayAgo(0))
            .run()
        const tracked = tracker()
        expect(
            (await post(payload(), tracked.db, { MAX_RESERVED_WRITES_PER_DAY: '999999999' }))
                .status,
        ).toBe(429)
        expect(tracked.stats.rowsWritten).toBe(0)
    })

    it.each(['batch', 'installation-day'])(
        'deduplicates simultaneous %s submissions atomically',
        async (kind) => {
            const body = payload()
            const barrier = rendezvous()
            const first = tracker(barrier)
            const second = tracker(barrier)
            const responses = await Promise.all([
                post(body, first.db),
                post(
                    kind === 'batch' ? body : { ...body, batch_id: crypto.randomUUID() },
                    second.db,
                ),
            ])
            expect(responses.map((response) => response.status)).toEqual([200, 200])
            const result = await Promise.all(
                responses.map((response) => response.json() as Promise<{ duplicate: boolean }>),
            )
            expect(result.map((entry) => entry.duplicate).sort()).toEqual([false, true])
            expect(await count('accepted_batches')).toBe(1)
            expect(await count('daily_active')).toBe(1)
            expect(Math.min(first.stats.rowsWritten, second.stats.rowsWritten)).toBe(0)
            expect(first.stats.rowsWritten + second.stats.rowsWritten).toBe(16)
        },
    )

    it.each(['global', 'writes', 'installation'] as const)(
        'prevents concurrent requests exceeding the final %s quota slot without rejected writes',
        async (kind) => {
            const barrier = rendezvous()
            const first = tracker(barrier)
            const second = tracker(barrier)
            const body = payload()
            const overrides =
                kind === 'global'
                    ? { MAX_ACCEPTED_BATCHES_PER_DAY: '1' }
                    : kind === 'writes'
                      ? { MAX_RESERVED_WRITES_PER_DAY: '16' }
                      : { MAX_BATCHES_PER_INSTALLATION_PER_DAY: '1' }
            const other =
                kind === 'installation' ? payload([dayAgo(1)], body.installation_id) : payload()
            const responses = await Promise.all([
                post(body, first.db, overrides),
                post(other, second.db, overrides),
            ])
            expect(responses.map((response) => response.status).sort()).toEqual([200, 429])
            const rejected = responses[0].status === 429 ? first : second
            expect(rejected.stats.rowsWritten).toBe(0)
            expect(await count('accepted_batches')).toBe(1)
            expect(await count('daily_active')).toBe(1)
            expect(await count('ingestion_daily')).toBe(1)
        },
    )

    it('rolls back the entire reservation on SQL failure and persists a retry normally', async () => {
        const body = payload()
        await env.DB.prepare(
            "CREATE TRIGGER test_fail_heartbeat BEFORE INSERT ON daily_active BEGIN SELECT RAISE(ABORT, 'injected failure'); END",
        ).run()
        try {
            expect((await post(body)).status).toBe(503)
            for (const table of [
                'accepted_batches',
                'ingestion_daily',
                'ingestion_global_daily',
                'installations',
                'platforms',
                'daily_active',
                'daily_active_dims',
            ])
                expect(await count(table)).toBe(0)
        } finally {
            await env.DB.prepare('DROP TRIGGER test_fail_heartbeat').run()
        }
        const response = await post(body)
        expect(response.status).toBe(200)
        expect(await response.json()).toEqual({ accepted: true, duplicate: false })
        expect(await count('daily_active')).toBe(1)
    })

    it('does not put older offline days into current WAU or MAU sets', async () => {
        expect((await post(payload([dayAgo(30)]))).status).toBe(200)
        expect(await count('wau_seen')).toBe(0)
        expect(await count('mau_seen')).toBe(0)
        expect((await post(payload([dayAgo(7)]))).status).toBe(200)
        expect(await count('wau_seen')).toBe(0)
        expect(await count('mau_seen')).toBe(1)
    })

    it('repairs late historical totals but does not rewrite unchanged maintenance results', async () => {
        expect((await post(payload([dayAgo(3)]))).status).toBe(200)
        await runMaintenance(env.DB)
        expect(
            (await env.DB.prepare('SELECT active_installations FROM daily_totals WHERE day = ?')
                .bind(dayAgo(3))
                .first())!.active_installations,
        ).toBe(1)
        expect((await post(payload([dayAgo(3)]))).status).toBe(200)
        await runMaintenance(env.DB)
        expect(
            (await env.DB.prepare('SELECT active_installations FROM daily_totals WHERE day = ?')
                .bind(dayAgo(3))
                .first())!.active_installations,
        ).toBe(2)
        const tracked = tracker()
        await runMaintenance(tracked.db)
        expect(tracked.stats.rowsWritten).toBe(0)
    })
})
