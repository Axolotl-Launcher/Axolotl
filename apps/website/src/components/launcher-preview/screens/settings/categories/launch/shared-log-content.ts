import type { GameInstance } from '../../../../lib/types'

/**
 * What a shared diagnostic was uploaded with: the censored combined log the
 * crash window builds (`build_combined_log` in packages/app-lib/src/api/logs/
 * crash_analysis.rs: one `===== <file> =====` block per collected log, player
 * names replaced by `{MINECRAFT_USERNAME}`). The lines follow the startup output
 * the preview's game processes write, so a shared log reads like the Logs tab.
 */
const LOADER_NAMES: Partial<Record<GameInstance['loader'], string>> = {
    fabric: 'Fabric Loader',
    quilt: 'Quilt Loader',
    forge: 'Forge',
    neoforge: 'NeoForge',
}

function clockTime(at: number) {
    return new Date(at).toTimeString().slice(0, 8)
}

function line(at: number, thread: string, message: string, level = 'INFO') {
    return `[${clockTime(at)}] [${thread}/${level}]: ${message}`
}

export interface SharedLogSession {
    instance: Pick<GameInstance, 'game_version' | 'loader' | 'loader_version'>
    /** When the game was launched (ms). */
    startedAt: number
    /** When it ran out of memory (ms). */
    crashedAt: number
    /** mclo.gs only received the tail of an oversized log. */
    truncated: boolean
}

function startupLines(session: SharedLogSession): string[] {
    const { instance, startedAt } = session
    const loader = LOADER_NAMES[instance.loader]
    const lines: string[] = []
    if (loader && (instance.loader === 'fabric' || instance.loader === 'quilt')) {
        lines.push(
            line(
                startedAt,
                'main',
                `Loading Minecraft ${instance.game_version} with ${loader} ${instance.loader_version ?? ''}`.trim(),
            ),
        )
    } else if (loader) {
        lines.push(
            line(
                startedAt,
                'main',
                `ModLauncher running: ${loader} ${instance.loader_version ?? ''}`.trim(),
            ),
        )
    } else {
        lines.push(line(startedAt, 'main', 'Building optimized datafixer'))
    }
    lines.push(
        line(startedAt + 6_000, 'Render thread', 'Setting user: {MINECRAFT_USERNAME}'),
        line(startedAt + 6_000, 'Render thread', 'Backend library: LWJGL version 3.3.3-snapshot'),
        line(
            startedAt + 9_000,
            'Render thread',
            `Reloading ResourceManager: vanilla${loader ? `, ${instance.loader}` : ''}`,
        ),
        line(startedAt + 11_000, 'Render thread', 'Sound engine started'),
    )
    return lines
}

/** The integrated server falling behind, the spam that grows a log past mclo.gs' limit. */
function lagLines(crashedAt: number): string[] {
    return Array.from({ length: 6 }, (_, index) =>
        line(
            crashedAt - (6 - index) * 4_800,
            'Server thread',
            `Can't keep up! Is the server overloaded? Running ${2_040 + index * 186}ms or ${40 + index * 3} ticks behind`,
            'WARN',
        ),
    )
}

export function buildSharedLogContent(session: SharedLogSession): string {
    const lines = session.truncated ? lagLines(session.crashedAt) : startupLines(session)
    lines.push(
        line(session.crashedAt, 'Render thread', 'Out of memory', 'ERROR'),
        'java.lang.OutOfMemoryError: Java heap space',
        line(session.crashedAt + 1_400, 'Render thread', 'Stopping!'),
    )
    return `\n===== latest.log =====\n${lines.join('\n')}\n`
}
