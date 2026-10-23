import { reactive } from 'vue'

import { type PreviewState, usePreview, usePreviewDomain } from '../../../../lib/state/index'
import { buildSharedLogContent } from './shared-log-content'

/** `AnalysisSource` (apps/app-frontend/src/logagent/types.ts). */
export type AnalysisSource = 'custom' | 'logshare'

/** `AnalysisSettings` (logagent/types.ts): the `crash_analysis_ai_settings` row. */
export interface AnalysisSettings {
    enabled: boolean
    provider_id: string
    model_id: string
    ai_source: AnalysisSource
}

/** `LogShareSettings` as LogShareSettings.vue types it (logshare.rs serializes strings). */
export interface LogShareSettings {
    share_provider: 'logshare' | 'mclogs'
    ai_source: string
    auto_upload: boolean
    multi_file: boolean
    no_storage: boolean
    show_progress: boolean
}

/** `SharedLog` (packages/app-lib/src/api/logs/logshare.rs): one row of `shared_logs`. */
export interface SharedLog {
    id: string
    url: string
    raw: string
    token: string
    provider: string
    instance_id: string | null
    instance_name: string | null
    truncated: boolean
    /** Unix seconds. */
    created_at: number
}

/** The `log_share_settings` row; its `ai_source` column follows the analysis row. */
interface LogShareRow {
    share_provider: string
    ai_source: string
    auto_upload: boolean
    multi_file: boolean
    no_storage: boolean
    show_progress: boolean
}

interface LaunchDomain {
    logShare: LogShareRow
    analysis: AnalysisSettings
    sharedLogs: SharedLog[]
    /** What each shared log was uploaded with, keyed by log id. */
    sharedContent: Record<string, string>
}

const MINUTE = 60 * 1000
const DAY = 24 * 60 * MINUTE

const BASE62 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'

function randomString(length: number, alphabet: string) {
    const bytes = new Uint8Array(length)
    if (typeof crypto !== 'undefined' && crypto.getRandomValues) crypto.getRandomValues(bytes)
    else for (let index = 0; index < length; index++) bytes[index] = Math.floor(Math.random() * 256)
    return Array.from(bytes, (byte) => alphabet[byte % alphabet.length]).join('')
}

/**
 * Diagnostics the preview user shared before: crash windows of past sessions
 * that uploaded to LogShare automatically (`auto_upload`, on by default) and
 * one shared through mclo.gs while that was the selected service. Each one
 * belongs to a session of an existing instance, ending some minutes after it
 * was launched (or, for the older one, weeks before its latest session).
 */
const SHARED_LOG_SEEDS: Array<{
    instanceId: string
    provider: 'logshare' | 'mclogs'
    /** How long before the instance's last launch the session started. */
    sessionAgo: number
    /** How long into that session the game ran out of memory. */
    crashedAfter: number
    truncated?: boolean
}> = [
    { instanceId: 'cobblemon', provider: 'logshare', sessionAgo: 0, crashedAfter: 47 * MINUTE },
    {
        instanceId: 'better-mc',
        provider: 'mclogs',
        sessionAgo: 0,
        crashedAfter: 82 * MINUTE,
        truncated: true,
    },
    { instanceId: 'zombie-invade', provider: 'logshare', sessionAgo: 0, crashedAfter: 34 * MINUTE },
    {
        instanceId: 'fabulously-optimized',
        provider: 'logshare',
        sessionAgo: 18 * DAY,
        crashedAfter: 61 * MINUTE,
    },
]

function seedSharedLogs(state: PreviewState) {
    const sharedLogs: SharedLog[] = []
    const sharedContent: Record<string, string> = {}
    const now = Date.now()
    for (const seed of SHARED_LOG_SEEDS) {
        const instance = state.instances.instances.value.find((item) => item.id === seed.instanceId)
        if (!instance?.last_played) continue
        const startedAt = Date.parse(instance.last_played) - seed.sessionAgo
        const crashedAt = Math.min(startedAt + seed.crashedAfter, now - MINUTE)
        const truncated = seed.truncated ?? false
        let log: SharedLog
        if (seed.provider === 'logshare') {
            const id = randomString(8, BASE62)
            log = {
                id,
                url: `https://logshare.cn/${id}`,
                raw: '',
                token: randomString(32, '0123456789abcdef'),
                provider: 'logshare',
                instance_id: instance.id,
                instance_name: instance.name,
                truncated,
                created_at: Math.floor((crashedAt + 9_000) / 1000),
            }
        } else {
            // mclo.gs shares are recorded by their URL, without a deletion token.
            const url = `https://mclo.gs/${randomString(7, BASE62)}`
            log = {
                id: url,
                url,
                raw: '',
                token: '',
                provider: 'mclogs',
                instance_id: instance.id,
                instance_name: instance.name,
                truncated,
                created_at: Math.floor((crashedAt + 95_000) / 1000),
            }
        }
        sharedLogs.push(log)
        sharedContent[log.id] = buildSharedLogContent({
            instance,
            startedAt,
            crashedAt,
            truncated,
        })
    }
    return { sharedLogs, sharedContent }
}

function createLaunchDomain(state: PreviewState): LaunchDomain {
    // Defaults of the migrations (log-share.sql, crash-analysis-ai.sql).
    return reactive({
        logShare: {
            share_provider: 'logshare',
            ai_source: 'logshare',
            auto_upload: true,
            multi_file: true,
            no_storage: false,
            show_progress: true,
        },
        analysis: { enabled: false, provider_id: '', model_id: '', ai_source: 'logshare' },
        ...seedSharedLogs(state),
    }) as LaunchDomain
}

function normalizeAiSource(value: string): AnalysisSource {
    return value === 'custom' ? 'custom' : 'logshare'
}

/**
 * The log commands of the app (apps/app-frontend/src/helpers/logs.js) over the
 * preview's `settings:launch` domain, with the outcomes of their Rust side
 * (packages/app-lib/src/api/logs/logshare.rs, analysis_settings.rs). Each call
 * resolves after an IPC round trip on the window clock and returns copies, as
 * values crossing the bridge are.
 */
export function useLaunchBackend() {
    const preview = usePreview()
    const domain = usePreviewDomain<LaunchDomain>('settings:launch', (state) => ({
        value: createLaunchDomain(state),
    }))

    function roundTrip(ms: number) {
        return new Promise<void>((resolve) => {
            preview.clock.after(ms, resolve)
        })
    }

    /** `get_log_share_settings`: the share row joined with the analysis source. */
    async function get_log_share_settings(): Promise<LogShareSettings> {
        await roundTrip(45)
        const row = domain.logShare
        return {
            share_provider: row.share_provider === 'logshare' ? 'logshare' : 'mclogs',
            ai_source: domain.analysis.ai_source,
            auto_upload: row.auto_upload,
            multi_file: row.multi_file,
            no_storage: row.no_storage,
            show_progress: row.show_progress,
        }
    }

    /** `update_log_share_settings`: normalizes the provider; the source is not written. */
    async function update_log_share_settings(settings: LogShareSettings): Promise<void> {
        await roundTrip(180)
        Object.assign(domain.logShare, {
            share_provider: settings.share_provider === 'logshare' ? 'logshare' : 'mclogs',
            auto_upload: settings.auto_upload,
            multi_file: settings.multi_file,
            no_storage: settings.no_storage,
            show_progress: settings.show_progress,
        })
    }

    async function get_crash_analysis_ai_settings(): Promise<AnalysisSettings> {
        await roundTrip(40)
        return { ...domain.analysis }
    }

    /** Saves the analysis row and mirrors its source into the share row, in one transaction. */
    async function update_crash_analysis_ai_settings(settings: AnalysisSettings): Promise<void> {
        await roundTrip(180)
        const aiSource = normalizeAiSource(settings.ai_source)
        domain.analysis = {
            enabled: settings.enabled,
            provider_id: settings.provider_id.trim(),
            model_id: settings.model_id.trim(),
            ai_source: aiSource,
        }
        domain.logShare.ai_source = aiSource
    }

    /** `list_shared_logs`: newest first. */
    async function list_shared_logs(): Promise<SharedLog[]> {
        await roundTrip(70)
        return [...domain.sharedLogs]
            .sort((left, right) => right.created_at - left.created_at)
            .map((log) => ({ ...log }))
    }

    /**
     * `record_shared_log`: upserts by id, keeping the first record time. The
     * preview also keeps the uploaded content, which the shared logs list opens.
     */
    async function record_shared_log(log: SharedLog, content: string): Promise<void> {
        await roundTrip(30)
        const existing = domain.sharedLogs.find((item) => item.id === log.id)
        if (existing) {
            Object.assign(existing, { ...log, created_at: existing.created_at })
        } else {
            domain.sharedLogs.push({
                ...log,
                created_at: Math.max(log.created_at, Math.floor(Date.now() / 1000)),
            })
        }
        domain.sharedContent[log.id] = content
    }

    /**
     * `delete_shared_log`: asks LogShare to delete the upload (`DELETE /v1/log/<id>`
     * with the token), then removes the record. A failed remote deletion is only
     * logged by the backend, so the record goes either way; mclo.gs shares carry
     * no token and their request is refused right away.
     */
    async function delete_shared_log(id: string, token: string): Promise<void> {
        await roundTrip(token ? 640 : 380)
        domain.sharedLogs = domain.sharedLogs.filter((item) => item.id !== id)
        Reflect.deleteProperty(domain.sharedContent, id)
    }

    /** The content a shared log was uploaded with (what its share page shows). */
    function shared_log_content(id: string): string | null {
        return domain.sharedContent[id] ?? null
    }

    return {
        get_log_share_settings,
        update_log_share_settings,
        get_crash_analysis_ai_settings,
        update_crash_analysis_ai_settings,
        list_shared_logs,
        record_shared_log,
        delete_shared_log,
        shared_log_content,
    }
}
