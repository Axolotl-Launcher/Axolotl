import { env, SELF } from 'cloudflare:test'
import { describe, expect, it } from 'vitest'

import { runMaintenance, type Bindings } from '../src'
import { batchSchema } from '../src/schema'

declare global {
    namespace Cloudflare {
        interface Env extends Bindings {}
    }
}

const installationId = '018f6ee8-4cb1-7db3-8a8d-8df96f122d85'

function batch(
    batchId: string,
    events: Array<Record<string, unknown>>,
    clientInstallationId = installationId,
): Record<string, unknown> {
    return {
        schema_version: 1,
        batch_id: batchId,
        installation_id: clientInstallationId,
        app: {
            version: '1.7.1',
            environment: 'production',
            platform: 'windows',
            arch: 'x86_64',
        },
        events,
    }
}

function heartbeat(eventId: string, day = new Date().toISOString().slice(0, 10)) {
    return { type: 'heartbeat', event_id: eventId, occurred_at: `${day}T12:00:00.000Z`, day }
}

async function post(payload: unknown): Promise<Response> {
    return await SELF.fetch('https://telemetry.example/v1/batch', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(payload),
    })
}

describe('telemetry worker', () => {
    it('serves health without storage access', async () => {
        const response = await SELF.fetch('https://telemetry.example/health')
        expect(response.status).toBe(200)
        expect(await response.json()).toEqual({ status: 'ok', schema_version: 1 })
    })

    it('accepts strict heartbeat batches and rejects unknown structures', async () => {
        const invalid = batch('11111111-1111-4111-8111-111111111111', [
            { ...heartbeat('21111111-1111-4111-8111-111111111111'), unknown: true },
        ])
        expect(batchSchema.safeParse(invalid).success).toBe(false)
        expect((await post(invalid)).status).toBe(400)

        const oversized = await SELF.fetch('https://telemetry.example/v1/batch', {
            method: 'POST',
            body: 'x'.repeat(65 * 1024),
        })
        expect(oversized.status).toBe(413)
    })

    it('rejects legacy error batches without charging quota or storing payloads', async () => {
        const payload = batch('12111111-1111-4111-8111-111111111111', [
            {
                type: 'error',
                event_id: '13111111-1111-4111-8111-111111111111',
                occurred_at: new Date().toISOString(),
                fingerprint: 'a'.repeat(64),
                occurrence_count: 1,
                error_type: 'legacy_error',
                message: 'must not be retained',
            },
        ])
        const response = await post(payload)
        expect(response.status).toBe(400)
        expect(await response.json()).toEqual({ error: 'error_reporting_disabled' })

        const stored = await env.DB.prepare(
            'SELECT COUNT(*) AS count FROM accepted_batches WHERE batch_id = ?',
        )
            .bind('12111111-1111-4111-8111-111111111111')
            .first<{ count: number }>()
        expect(stored?.count).toBe(0)
    })

    it('enforces the daily installation batch cap without charging duplicates', async () => {
        const cappedInstallation = '018f6ee8-4cb1-7db3-8a8d-8df96f122d99'
        const day = new Date().toISOString().slice(0, 10)
        const payloadFor = (index: number) =>
            batch(
                `60000000-0000-4000-8000-${index.toString().padStart(12, '0')}`,
                [heartbeat(`70000000-0000-4000-8000-${index.toString().padStart(12, '0')}`, day)],
                cappedInstallation,
            )

        for (let index = 0; index < 25; index++)
            expect((await post(payloadFor(index))).status).toBe(200)
        expect((await post(payloadFor(0))).status).toBe(200)

        const rejected = await post(payloadFor(25))
        expect(rejected.status).toBe(429)
        expect(rejected.headers.get('Retry-After')).not.toBeNull()
    })

    it('opens the global ingestion circuit breaker at 100,000 accepted batches', async () => {
        const day = new Date().toISOString().slice(0, 10)
        await env.DB.prepare(
            `INSERT INTO ingestion_global_daily (day, accepted_batches)
			VALUES (?, 99999)
			ON CONFLICT (day) DO UPDATE SET accepted_batches = 99999`,
        )
            .bind(day)
            .run()

        const first = batch(
            '80000000-0000-4000-8000-000000000001',
            [heartbeat('81000000-0000-4000-8000-000000000001', day)],
            '018f6ee8-4cb1-7db3-8a8d-8df96f122d98',
        )
        expect((await post(first)).status).toBe(200)
        const rejected = await post({
            ...first,
            batch_id: '80000000-0000-4000-8000-000000000002',
            events: [heartbeat('81000000-0000-4000-8000-000000000002', day)],
        })
        expect(rejected.status).toBe(429)
    })

    it('hashes installations and keeps heartbeat batches idempotent', async () => {
        const payload = batch('31111111-1111-4111-8111-111111111111', [
            heartbeat('41111111-1111-4111-8111-111111111111', '2026-08-14'),
        ])
        expect((await post(payload)).status).toBe(200)
        expect((await post(payload)).status).toBe(200)

        const installation = await env.DB.prepare(
            'SELECT installation_hash FROM installations',
        ).first<{
            installation_hash: string
        }>()
        expect(installation?.installation_hash).toMatch(/^[0-9a-f]{64}$/)
        expect(installation?.installation_hash).not.toBe(installationId)
        const active = await env.DB.prepare('SELECT COUNT(*) AS count FROM daily_active').first<{
            count: number
        }>()
        expect(active?.count).toBe(1)
    })

    it('keeps offline heartbeat dates for DAU, WAU, and MAU queries', async () => {
        const day = (daysAgo: number) =>
            new Date(Date.now() - daysAgo * 24 * 60 * 60 * 1_000).toISOString().slice(0, 10)
        const samples = [
            [
                '32111111-1111-4111-8111-111111111111',
                '42111111-1111-4111-8111-111111111111',
                installationId.replace(/85$/, '81'),
                0,
            ],
            [
                '33111111-1111-4111-8111-111111111111',
                '43111111-1111-4111-8111-111111111111',
                installationId.replace(/85$/, '82'),
                6,
            ],
            [
                '34111111-1111-4111-8111-111111111111',
                '44111111-1111-4111-8111-111111111111',
                installationId.replace(/85$/, '83'),
                29,
            ],
        ] as const
        for (const [batchId, eventId, clientId, offset] of samples) {
            const heartbeatDay = day(offset)
            expect(
                (await post(batch(batchId, [heartbeat(eventId, heartbeatDay)], clientId))).status,
            ).toBe(200)
        }

        const counts = await env.DB.prepare(
            `SELECT
				COUNT(DISTINCT CASE WHEN day = ? THEN installation_hash END) AS dau,
				COUNT(DISTINCT CASE WHEN day >= date(?, '-6 days') THEN installation_hash END) AS wau,
				COUNT(DISTINCT CASE WHEN day >= date(?, '-29 days') THEN installation_hash END) AS mau
			FROM daily_active`,
        )
            .bind(day(0), day(0), day(0))
            .first<{ dau: number; wau: number; mau: number }>()
        expect(counts).toEqual({ dau: 1, wau: 2, mau: 3 })
    })

    it('refreshes usage totals without rebuilding error aggregates', async () => {
        const yesterday = new Date(Date.now() - 24 * 60 * 60 * 1_000).toISOString().slice(0, 10)
        const response = await post(
            batch('a2111111-1111-4111-8111-111111111111', [
                heartbeat('a3111111-1111-4111-8111-111111111111', yesterday),
            ]),
        )
        expect(response.status).toBe(200)
        await runMaintenance(env.DB)
        const totals = await env.DB.prepare(
            'SELECT new_installations, active_installations, error_occurrences, distinct_error_groups FROM daily_totals WHERE day = ?',
        )
            .bind(yesterday)
            .first<Record<string, number>>()
        expect(totals?.active_installations).toBeGreaterThanOrEqual(1)
        expect(totals?.error_occurrences).toBe(0)
        expect(totals?.distinct_error_groups).toBe(0)
    })
})
