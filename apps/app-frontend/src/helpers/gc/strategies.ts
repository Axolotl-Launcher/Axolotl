import type { GcContext, GcStrategyDefinition, GcStrategyId, ResolvedGcStrategyId } from './types'

// Official Minecraft launcher G1GC tuning. `-XX:SurvivorRatio=8` (the flag in
// the leak of the original list was misspelled "SurvialRation", which JVMs
// would reject).
function buildG1gcMojangArgs(): string {
    return [
        '-XX:+UseG1GC',
        '-XX:+ParallelRefProcEnabled',
        '-XX:MaxGCPauseMillis=200',
        '-XX:+UnlockExperimentalVMOptions',
        '-XX:+DisableExplicitGC',
        '-XX:+AlwaysPreTouch',
        '-XX:G1NewSizePercent=30',
        '-XX:G1MaxNewSizePercent=40',
        '-XX:G1HeapRegionSize=8M',
        '-XX:G1ReservePercent=15',
        '-XX:G1HeapWastePercent=5',
        '-XX:G1MixedGCCountTarget=4',
        '-XX:InitiatingHeapOccupancyPercent=15',
        '-XX:G1MixedGCLiveThresholdPercent=90',
        '-XX:G1RSetUpdatingPauseTimePercent=5',
        '-XX:SurvivorRatio=8',
    ].join(' ')
}

// PCL-style Shenandoah (adaptive, no large pages — the safe default variant).
function buildPclShenandoahArgs(): string {
    return [
        '-XX:+UseShenandoahGC',
        '-XX:ShenandoahGCHeuristics=adaptive',
        '-XX:+AlwaysPreTouch',
        '-XX:+DisableExplicitGC',
    ].join(' ')
}

// Shenandoah with large pages enabled (may warn on systems without support).
function buildShenandoahArgs(): string {
    return [
        '-XX:+UseShenandoahGC',
        '-XX:ShenandoahGCHeuristics=adaptive',
        '-XX:+AlwaysPreTouch',
        '-XX:+UseLargePages',
        '-XX:+DisableExplicitGC',
    ].join(' ')
}

function buildZgcArgs(javaMajorVersion: number | null): string {
    const args = ['-XX:+UseZGC']
    // Generational ZGC only exists on JDK 21+.
    if (javaMajorVersion !== null && javaMajorVersion >= 21) {
        args.push('-XX:+ZGenerational')
    }
    args.push('-XX:+AlwaysPreTouch', '-XX:-ZUncommit')
    return args.join(' ')
}

// Detection only tags a preset when its *complete* flag set is present in the
// pasted args — a partial or edited arg list is treated as the user's own raw
// args, never auto-mislabeled as a preset.
function tokensOf(argString: string): string[] {
    return argString.split(/\s+/).filter(Boolean)
}

function hasFullArgSet(pastedArgs: string, presetArgString: string): boolean {
    const inputSet = new Set(tokensOf(pastedArgs))
    return tokensOf(presetArgString).every((token) => inputSet.has(token))
}

function detectG1gcMojang(args: string): boolean {
    return hasFullArgSet(args, buildG1gcMojangArgs())
}

// PCL Shenandoah: complete adaptive set, and no large pages.
function detectPclShenandoah(args: string): boolean {
    return hasFullArgSet(args, buildPclShenandoahArgs()) && !args.includes('-XX:+UseLargePages')
}

// Shenandoah with large pages (its full set already requires `-XX:+UseLargePages`).
function detectShenandoah(args: string): boolean {
    return hasFullArgSet(args, buildShenandoahArgs())
}

function detectZgc(args: string): boolean {
    return hasFullArgSet(args, buildZgcArgs(null))
}

export const GC_STRATEGY_DEFINITIONS: Record<ResolvedGcStrategyId, GcStrategyDefinition> = {
    'g1gc-mojang': {
        id: 'g1gc-mojang',
        baseArgs: buildG1gcMojangArgs(),
        detect: detectG1gcMojang,
        buildArgs: () => buildG1gcMojangArgs(),
    },
    pcl: {
        id: 'pcl',
        baseArgs: buildPclShenandoahArgs(),
        detect: detectPclShenandoah,
        buildArgs: () => buildPclShenandoahArgs(),
    },
    shenandoah: {
        id: 'shenandoah',
        baseArgs: buildShenandoahArgs(),
        detect: detectShenandoah,
        buildArgs: () => buildShenandoahArgs(),
    },
    zgc: {
        id: 'zgc',
        baseArgs: buildZgcArgs(null),
        detect: detectZgc,
        buildArgs: (context) => buildZgcArgs(context?.javaMajorVersion ?? null),
    },
}

export function detectGcStrategy(args: string): ResolvedGcStrategyId | null {
    for (const [strategyId, definition] of Object.entries(GC_STRATEGY_DEFINITIONS)) {
        if (definition.detect(args)) {
            return strategyId as ResolvedGcStrategyId
        }
    }
    return null
}

export function getStrategyBaseArgs(strategyId: GcStrategyId): string {
    if (strategyId === 'auto') {
        return GC_STRATEGY_DEFINITIONS['g1gc-mojang'].baseArgs
    }
    return GC_STRATEGY_DEFINITIONS[strategyId].baseArgs
}

/**
 * The preferred strategy plus the fallback chain, ordered by preference. The
 * backend verifies each block against the actual JVM and picks the first one
 * that is accepted (pruning unsupported tuning flags along the way).
 *
 * Fallbacks only ever move to less resource-hungry strategies — if the
 * heuristic deliberately avoided ZGC (insufficient resources), we must not
 * silently jump back up to it when Shenandoah is unavailable.
 */
const SAFE_TO_DEMANDING: ResolvedGcStrategyId[] = ['g1gc-mojang', 'pcl', 'shenandoah', 'zgc']

export function buildGcCandidateChain(
    context: GcContext,
    preferred: ResolvedGcStrategyId,
): { ids: string[]; args: string[][] } {
    const preferredDemand = SAFE_TO_DEMANDING.indexOf(preferred)
    const ids: Array<ResolvedGcStrategyId | 'minimal-g1'> = [preferred]
    if (preferredDemand > 0) {
        for (let demand = preferredDemand - 1; demand >= 0; demand -= 1) {
            ids.push(SAFE_TO_DEMANDING[demand])
        }
    }
    // Absolute last resort: just the G1 selector (known to every HotSpot JVM).
    ids.push('minimal-g1')
    const args = ids.map((id) => {
        if (id === 'minimal-g1') return ['-XX:+UseG1GC']
        return GC_STRATEGY_DEFINITIONS[id].buildArgs(context).split(/\s+/).filter(Boolean)
    })
    return { ids, args }
}
