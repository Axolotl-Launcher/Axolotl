import { PREVIEW_APP_DIRECTORY } from '../../state'

/**
 * The launcher's own log files on this Mac, as packages/app-lib/src/logger.rs
 * writes them: one `session_<local start time>.log` per launch in
 * `<data dir>/launcher_logs`, pruned in place as they age (trace after 30
 * minutes, debug after 2 hours, everything after 3 days). The current session
 * started when the window opened and keeps growing at the chosen level.
 */
export const LAUNCHER_LOGS_DIR = `${PREVIEW_APP_DIRECTORY}/launcher_logs`
export const HOME_DIR = '/Users/steve'

export type LogExportRange = 'last30Minutes' | 'last2Hours' | 'all'
export type LogExportLevel = 'all' | 'debug' | 'info'

export interface LogExportOptions {
    range: LogExportRange
    level: LogExportLevel
    includeSystemInfo: boolean
    includeInstanceLogs: boolean
    includeCrashAnalysis: boolean
    /** Connection tests run against the launcher's services; the preview runs none. */
    includeNetworkConnections: boolean
}

export interface LauncherLogFile {
    name: string
    /** Last write, epoch milliseconds. */
    modified: number
    bytes: number
    /** The file of the running session. */
    current: boolean
}

const MINUTE = 60 * 1000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/** Bytes per minute the launcher writes at each level while it is in use. */
const BYTES_PER_MINUTE: Record<string, number> = {
    error: 120,
    warn: 380,
    info: 5_600,
    debug: 41_000,
    trace: 176_000,
}

/** Start-up output (settings, database, Java scan, metadata refresh) at debug level. */
const STARTUP_BYTES = 61_440

/** Share of debug-level output that is info and above, and of trace output that is trace. */
const INFO_SHARE = 0.16
const TRACE_SHARE_OF_TRACE_LOG = 0.74

function pad(value: number) {
    return String(value).padStart(2, '0')
}

/** `format!("session_{}", Local::now().format("%Y%m%d_%H%M%S"))`. */
export function sessionLogName(start: Date) {
    return (
        `session_${start.getFullYear()}${pad(start.getMonth() + 1)}${pad(start.getDate())}` +
        `_${pad(start.getHours())}${pad(start.getMinutes())}${pad(start.getSeconds())}.log`
    )
}

/** When this launch began: the moment the visitor opened the page. */
export function currentSessionStart() {
    const origin = typeof performance !== 'undefined' ? performance.timeOrigin : Date.now()
    return Math.floor(origin / 1000) * 1000
}

/** Two earlier evenings' sessions, already pruned down to info and above. */
function previousSessions(now: number) {
    const day = (offset: number, hours: number, minutes: number, seconds: number) => {
        const date = new Date(now - offset * DAY)
        date.setHours(hours, minutes, seconds, 0)
        return date.getTime()
    }
    return [
        { start: day(2, 19, 47, 52), length: 94 * MINUTE, bytes: 118_272 },
        { start: day(1, 21, 13, 5), length: 137 * MINUTE, bytes: 173_568 },
    ]
}

function currentSessionBytes(now: number, level: string) {
    const minutes = Math.max(0, (now - currentSessionStart()) / MINUTE)
    const rate = BYTES_PER_MINUTE[level] ?? BYTES_PER_MINUTE.debug
    return Math.round(STARTUP_BYTES + minutes * rate)
}

/** The folder's `session_*.log` files, oldest first (the order the export reads them). */
export function launcherLogFiles(now: number, level: string): LauncherLogFile[] {
    const files: LauncherLogFile[] = previousSessions(now).map((session) => ({
        name: sessionLogName(new Date(session.start)),
        modified: session.start + session.length,
        bytes: session.bytes,
        current: false,
    }))
    files.push({
        name: sessionLogName(new Date(currentSessionStart())),
        modified: now,
        bytes: currentSessionBytes(now, level),
        current: true,
    })
    return files
}

/** The default name `exportLauncherLogs()` proposes in the save panel. */
export function defaultExportFileName(now = new Date()) {
    const timestamp = now.toISOString().replace(/[:.]/g, '-')
    return `Axolotl Launcher logs ${timestamp}.zip`
}

/** `manifest.txt`, `system-information.txt` and each archive entry's own headers. */
const MANIFEST_BYTES = 520
const SYSTEM_INFO_BYTES = 118
const ENTRY_OVERHEAD_BYTES = 132
const END_OF_ARCHIVE_BYTES = 22
/** `LOG_EXPORT_INSTANCE_LOG_TAIL_BYTES`, and what the newest instance log holds. */
const INSTANCE_LOG_TAIL_BYTES = 512 * 1024
const NEWEST_INSTANCE_LOG_BYTES = 196_608
/** Deflate on launcher log text. */
const COMPRESSION = 0.11

/**
 * Size of the ZIP `export_launcher_logs` writes: every session log filtered to
 * the range and level (an older session outside the range still gets its empty
 * entry), the manifest, and the optional extras. The newest instance log holds
 * no crash, so the crash analysis adds no entry.
 */
export function exportArchiveBytes(options: LogExportOptions, now: number, level: string) {
    const elapsed = Math.max(1, now - currentSessionStart())
    const maxAge =
        options.range === 'last30Minutes'
            ? 30 * MINUTE
            : options.range === 'last2Hours'
              ? 2 * HOUR
              : null
    let raw = MANIFEST_BYTES
    let entries = 1
    for (const file of launcherLogFiles(now, level)) {
        entries++
        if (!file.current) {
            if (maxAge === null) raw += file.bytes
            continue
        }
        let bytes = file.bytes * (maxAge === null ? 1 : Math.min(1, maxAge / elapsed))
        if (options.level === 'info') {
            bytes *= level === 'trace' ? INFO_SHARE * (1 - TRACE_SHARE_OF_TRACE_LOG) : INFO_SHARE
        } else if (options.level === 'debug' && level === 'trace') {
            bytes *= 1 - TRACE_SHARE_OF_TRACE_LOG
        }
        raw += bytes
    }
    if (options.includeSystemInfo) {
        raw += SYSTEM_INFO_BYTES
        entries++
    }
    if (options.includeInstanceLogs) {
        raw += Math.min(INSTANCE_LOG_TAIL_BYTES, NEWEST_INSTANCE_LOG_BYTES)
        entries++
    }
    return Math.round(raw * COMPRESSION + entries * ENTRY_OVERHEAD_BYTES + END_OF_ARCHIVE_BYTES)
}
