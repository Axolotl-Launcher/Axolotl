import { applyD1Migrations, env } from 'cloudflare:test'
import { expect, it } from 'vitest'

import { ingestBatch } from '../src/ingest'

declare global {
    namespace Cloudflare {
        interface Env {
            UPGRADE_DB: D1Database
        }
    }
}

it('upgrades populated pre-0006 databases without rewriting usage data or breaking legacy inserts', async () => {
    const migrationIndex = env.TEST_MIGRATIONS.findIndex((migration) =>
        migration.name.startsWith('0006_'),
    )
    expect(migrationIndex).toBeGreaterThan(0)
    const db = env.UPGRADE_DB
    await applyD1Migrations(db, env.TEST_MIGRATIONS.slice(0, migrationIndex))
    const day = new Date().toISOString().slice(0, 10)
    const batchId = crypto.randomUUID()
    await db.batch([
        db
            .prepare('INSERT INTO installations VALUES (?, 1, 2, ?, ?, ?, ?)')
            .bind('legacy-install', day, '1.7.1', 'windows', 'x86_64'),
        db
            .prepare('INSERT INTO daily_active VALUES (?, ?, ?, ?, ?)')
            .bind(day, 'legacy-install', '1.7.1', 'windows', 'x86_64'),
        db
            .prepare(
                'INSERT INTO accepted_batches (batch_id, installation_hash, accepted_at) VALUES (?, ?, 1)',
            )
            .bind(batchId, 'legacy-install'),
        db
            .prepare('INSERT INTO ingestion_global_daily (day, accepted_batches) VALUES (?, 2)')
            .bind(day),
    ])
    const before = await db.prepare('SELECT * FROM installations').first()
    await applyD1Migrations(db, env.TEST_MIGRATIONS.slice(migrationIndex))
    expect(await db.prepare('SELECT * FROM installations').first()).toEqual(before)
    expect(await db.prepare('SELECT COUNT(*) AS count FROM daily_active').first()).toEqual({
        count: 1,
    })
    expect(await db.prepare('SELECT COUNT(*) AS count FROM daily_active_dims').first()).toEqual({
        count: 3,
    })
    expect(await db.prepare('SELECT ingestion_token FROM accepted_batches').first()).toEqual({
        ingestion_token: null,
    })
    expect(
        await db
            .prepare('SELECT accepted_batches, reserved_writes FROM ingestion_global_daily')
            .first(),
    ).toEqual({ accepted_batches: 2, reserved_writes: 32 })

    // Existing readers and writers can continue naming their original columns.
    await db
        .prepare(
            'INSERT INTO accepted_batches (batch_id, installation_hash, accepted_at) VALUES (?, ?, 1)',
        )
        .bind(crypto.randomUUID(), 'older-writer')
        .run()
    expect(await db.prepare('SELECT COUNT(*) AS count FROM accepted_batches').first()).toEqual({
        count: 2,
    })
    expect(
        await ingestBatch(
            db,
            {
                schema_version: 1,
                batch_id: batchId,
                installation_id: crypto.randomUUID(),
                app: {
                    version: '1.7.1',
                    environment: 'production',
                    platform: 'windows',
                    arch: 'x86_64',
                },
                events: [
                    {
                        type: 'heartbeat',
                        event_id: crypto.randomUUID(),
                        occurred_at: `${day}T12:00:00.000Z`,
                        day,
                    },
                ],
            },
            'legacy-install',
            { installation: 25, global: 100000, writes: 60000 },
        ),
    ).toBe('duplicate')

    const yesterday = new Date(Date.now() - 86_400_000).toISOString().slice(0, 10)
    expect(
        await ingestBatch(
            db,
            {
                schema_version: 1,
                batch_id: crypto.randomUUID(),
                installation_id: crypto.randomUUID(),
                app: {
                    version: '1.7.1',
                    environment: 'production',
                    platform: 'windows',
                    arch: 'x86_64',
                },
                events: [
                    {
                        type: 'heartbeat',
                        event_id: crypto.randomUUID(),
                        occurred_at: `${yesterday}T12:00:00.000Z`,
                        day: yesterday,
                    },
                ],
            },
            'legacy-install',
            { installation: 25, global: 100000, writes: 60000 },
        ),
    ).toBe('accepted')
    expect(await db.prepare('SELECT COUNT(*) AS count FROM daily_active').first()).toEqual({
        count: 2,
    })
    expect(await db.prepare('SELECT * FROM installations').first()).toEqual(before)
})
