import type { TelemetryBatch } from './schema'

export interface IngestionLimits {
    installation: number
    global: number
    writes: number
}

type IngestionStatus =
    'accepted' | 'duplicate' | 'installation_limit' | 'global_limit' | 'write_limit' | 'conflict'

interface IngestionState {
    batch_installation: string | null
    installation_batches: number
    global_batches: number
    reserved_writes: number
    active_days: string
}

// Includes quota rows, the accepted-batch index, installation/platform rows,
// daily-active indexes and rollup-trigger writes. Maintenance uses separate headroom.
function writeReservation(days: number): number {
    return 8 + 8 * days
}

export async function ingestBatch(
    db: D1Database,
    batch: TelemetryBatch,
    installationHash: string,
    limits: IngestionLimits,
): Promise<IngestionStatus> {
    const acceptedAt = Math.floor(Date.now() / 1_000)
    const day = new Date(acceptedAt * 1_000).toISOString().slice(0, 10)
    const days = [...new Set(batch.events.map((event) => event.day))]
    const daysJson = JSON.stringify(days)
    const readState = async (): Promise<IngestionState> => {
        const state = await db
            .prepare(
                `SELECT
                    (SELECT installation_hash FROM accepted_batches WHERE batch_id = ?1) AS batch_installation,
                    COALESCE((SELECT accepted_batches FROM ingestion_daily
                        WHERE day = ?2 AND installation_hash = ?3), 0) AS installation_batches,
                    COALESCE((SELECT accepted_batches FROM ingestion_global_daily WHERE day = ?2), 0) AS global_batches,
                    COALESCE((SELECT reserved_writes FROM ingestion_global_daily WHERE day = ?2), 0) AS reserved_writes,
                    (SELECT json_group_array(day) FROM daily_active
                        WHERE installation_hash = ?3 AND day IN (SELECT value FROM json_each(?4))) AS active_days`,
            )
            .bind(batch.batch_id, day, installationHash, daysJson)
            .first<IngestionState>()
        if (!state) throw new Error('Missing ingestion state')
        return state
    }
    const missingDays = (state: IngestionState): string[] => {
        const recorded = new Set<string>(JSON.parse(state.active_days))
        return days.filter((candidate) => !recorded.has(candidate))
    }
    const status = (state: IngestionState): Exclude<IngestionStatus, 'accepted'> | undefined => {
        if (state.batch_installation !== null) {
            return state.batch_installation === installationHash ? 'duplicate' : 'conflict'
        }
        const missing = missingDays(state)
        if (missing.length === 0) return 'duplicate'
        if (state.installation_batches >= limits.installation) return 'installation_limit'
        if (state.global_batches >= limits.global) return 'global_limit'
        if (state.reserved_writes + writeReservation(missing.length) > limits.writes)
            return 'write_limit'
        return undefined
    }

    const initial = await readState()
    const initialStatus = status(initial)
    if (initialStatus) return initialStatus

    const newDays = missingDays(initial)
    const newDaysJson = JSON.stringify(newDays)
    const reservedWrites = writeReservation(newDays.length)
    const token = crypto.randomUUID()
    const ownsBatch = `EXISTS (
        SELECT 1 FROM accepted_batches WHERE batch_id = ? AND ingestion_token = ?
    )`
    // D1 batch() is transactional. Recheck quota and semantic deduplication
    // inside the transaction so concurrent requests cannot exceed either limit.
    const results = await db.batch([
        db
            .prepare(
                `INSERT INTO accepted_batches (batch_id, installation_hash, accepted_at, ingestion_token)
                SELECT ?1, ?2, ?3, ?4
                WHERE NOT EXISTS (SELECT 1 FROM accepted_batches WHERE batch_id = ?1)
                    AND COALESCE((SELECT accepted_batches FROM ingestion_daily
                        WHERE day = ?5 AND installation_hash = ?2), 0) < ?6
                    AND COALESCE((SELECT accepted_batches FROM ingestion_global_daily WHERE day = ?5), 0) < ?7
                    AND COALESCE((SELECT reserved_writes FROM ingestion_global_daily WHERE day = ?5), 0) + ?8 <= ?9
                    AND EXISTS (SELECT 1 FROM json_each(?10) AS event_day
                        WHERE NOT EXISTS (SELECT 1 FROM daily_active
                            WHERE day = event_day.value AND installation_hash = ?2))`,
            )
            .bind(
                batch.batch_id,
                installationHash,
                acceptedAt,
                token,
                day,
                limits.installation,
                limits.global,
                reservedWrites,
                limits.writes,
                newDaysJson,
            ),
        db
            .prepare(
                `INSERT INTO ingestion_daily (day, installation_hash, accepted_batches)
                SELECT ?, ?, 1 WHERE ${ownsBatch}
                ON CONFLICT (day, installation_hash) DO UPDATE
                SET accepted_batches = accepted_batches + 1`,
            )
            .bind(day, installationHash, batch.batch_id, token),
        db
            .prepare(
                `INSERT INTO ingestion_global_daily (day, accepted_batches, reserved_writes)
                SELECT ?, 1, ? WHERE ${ownsBatch}
                ON CONFLICT (day) DO UPDATE SET
                    accepted_batches = accepted_batches + 1,
                    reserved_writes = reserved_writes + excluded.reserved_writes
                RETURNING accepted_batches, reserved_writes`,
            )
            .bind(day, reservedWrites, batch.batch_id, token),
        db
            .prepare(
                `INSERT INTO installations (
                    installation_hash, first_seen_at, last_seen_at, first_seen_day, app_version, platform, arch
                ) SELECT ?, ?, ?, ?, ?, ?, ? WHERE ${ownsBatch}
                ON CONFLICT (installation_hash) DO NOTHING`,
            )
            .bind(
                installationHash,
                acceptedAt,
                acceptedAt,
                day,
                batch.app.version,
                batch.app.platform,
                batch.app.arch,
                batch.batch_id,
                token,
            ),
        db
            .prepare(
                `INSERT INTO platforms (platform) SELECT ? WHERE ${ownsBatch}
                ON CONFLICT (platform) DO NOTHING`,
            )
            .bind(batch.app.platform, batch.batch_id, token),
        db
            .prepare(
                `INSERT INTO daily_active (day, installation_hash, app_version, platform, arch)
                SELECT event_day.value, ?, ?, ?, ? FROM json_each(?) AS event_day
                WHERE ${ownsBatch}
                ON CONFLICT (day, installation_hash) DO NOTHING`,
            )
            .bind(
                installationHash,
                batch.app.version,
                batch.app.platform,
                batch.app.arch,
                newDaysJson,
                batch.batch_id,
                token,
            ),
    ])

    if (results[0].meta.changes !== 1) {
        const currentStatus = status(await readState())
        if (currentStatus) return currentStatus
        throw new Error('Ingestion state changed; retry request')
    }
    const totals = results[2].results[0] as { accepted_batches: number; reserved_writes: number }
    for (const ratio of [0.8, 0.9, 0.95]) {
        const threshold = Math.ceil(limits.writes * ratio)
        if (
            totals.reserved_writes - reservedWrites < threshold &&
            totals.reserved_writes >= threshold
        )
            console.warn('Telemetry write budget threshold reached', {
                reservedWrites: totals.reserved_writes,
                limit: limits.writes,
                percent: ratio * 100,
            })
    }
    return 'accepted'
}
