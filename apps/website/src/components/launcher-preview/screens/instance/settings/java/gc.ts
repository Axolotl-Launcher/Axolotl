import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'

import type { InstanceLoader } from '../../../../lib/types'
import { gcReasonMessages, javaPresetMessages } from '../messages.extra'

/*
 * Port of the app's GC helpers (`apps/app-frontend/src/helpers/gc/*`) and Java
 * argument presets (`helpers/java-argument-presets.ts`, `helpers/java-arguments.ts`).
 */

export type GcStrategyId = 'g1gc-mojang' | 'pcl' | 'shenandoah' | 'zgc' | 'auto'

export type ResolvedGcStrategyId = Exclude<GcStrategyId, 'auto'>

export interface GcContext {
    javaMajorVersion: number | null
    allocatedMemoryMb: number
    systemCpuCores: number
    systemLogicalProcessors: number
    modCount: number
    loader: InstanceLoader
}

export interface GcReason extends MessageDescriptor {
    values?: Record<string, unknown>
}

export interface GcResolution {
    resolvedStrategy: ResolvedGcStrategyId
    reasonChain: (string | GcReason)[]
}

interface GcStrategyDefinition {
    id: GcStrategyId
    baseArgs: string
    detect: (currentArgs: string) => boolean
    buildArgs: (context?: GcContext) => string
}

export interface JavaArgumentPreset {
    id: string
    title: MessageDescriptor
    description: MessageDescriptor
    args: string
    link: string
    group: string
    resolveArgs?: (context?: GcContext) => string
    detect?: (currentArgs: string) => boolean
    autoResolvedName?: string
    autoReasonChain?: (string | GcReason)[]
}

/* ------------------------------------------------------------- java-arguments.ts */

const FALLEN_AUTH_PROXY_BLOG_URL = 'https://blog.fallenbreath.me/zh-CN/2025/minecraft-service-proxy'

const FALLEN_AUTH_PROXY_JAVA_ARGS = [
    '-Dminecraft.api.auth.host=https://auth.msp.fallenbreath.me',
    '-Dminecraft.api.account.host=https://account.msp.fallenbreath.me',
    '-Dminecraft.api.session.host=https://session.msp.fallenbreath.me',
    '-Dminecraft.api.services.host=https://services.msp.fallenbreath.me',
    '-Dminecraft.api.profiles.host=https://profiles.msp.fallenbreath.me',
]

const FALLEN_AUTH_PROXY_JAVA_ARGS_STRING = FALLEN_AUTH_PROXY_JAVA_ARGS.join(' ')

export const AUTO_GC_PRESET_ARG = '@axolotl:gc:auto'

/* ------------------------------------------------------------------ strategies.ts */

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

/* --------------------------------------------------------------- auto-selector.ts */

function reason(descriptor: MessageDescriptor, values?: Record<string, unknown>): GcReason {
    return values ? { ...descriptor, values } : descriptor
}

export function resolveAutoGcArgs(context: GcContext): string {
    const resolution = resolveAutoGcStrategy(context)
    return GC_STRATEGY_DEFINITIONS[resolution.resolvedStrategy].buildArgs(context)
}

export function resolveAutoGcStrategy(context: GcContext): GcResolution {
    const reasonChain: (string | GcReason)[] = []

    const javaVersion = context.javaMajorVersion
    if (javaVersion !== null) {
        reasonChain.push(`Java ${javaVersion}`)
    } else {
        reasonChain.push(reason(gcReasonMessages.javaVersionUnknown))
    }

    if (javaVersion === null || javaVersion < 15) {
        reasonChain.push(reason(gcReasonMessages.javaTooOld))
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    if (context.allocatedMemoryMb < 4096) {
        reasonChain.push(
            reason(gcReasonMessages.insufficientMemory, {
                allocated: Math.round(context.allocatedMemoryMb / 1024),
            }),
        )
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    if (context.systemCpuCores <= 4 && context.systemLogicalProcessors <= 8) {
        reasonChain.push(
            reason(gcReasonMessages.insufficientCpu, {
                cores: context.systemCpuCores,
                threads: context.systemLogicalProcessors,
            }),
        )
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    if (context.modCount >= 200 && context.allocatedMemoryMb < 8192) {
        reasonChain.push(
            reason(gcReasonMessages.largeModpackLowMemory, {
                mods: context.modCount,
                allocated: Math.round(context.allocatedMemoryMb / 1024),
            }),
        )
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    const isLightweight =
        (context.loader === 'vanilla' ||
            context.loader === 'fabric' ||
            context.loader === 'quilt') &&
        context.modCount < 30

    if (isLightweight) {
        reasonChain.push(
            reason(gcReasonMessages.lightweightInstance, {
                loader: context.loader,
                mods: context.modCount,
            }),
        )
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    reasonChain.push(
        reason(gcReasonMessages.heavyInstance, {
            loader: context.loader,
            mods: context.modCount,
        }),
    )

    const memoryGb = context.allocatedMemoryMb / 1024
    const isResourceLow = context.allocatedMemoryMb < 6144 || context.systemCpuCores <= 6
    const isResourceMedium =
        !isResourceLow && context.allocatedMemoryMb < 10240 && context.systemCpuCores <= 12

    if (isResourceLow) {
        reasonChain.push(
            reason(gcReasonMessages.lowResources, {
                memory: Math.round(memoryGb),
                cores: context.systemCpuCores,
            }),
        )
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    if (isResourceMedium) {
        reasonChain.push(
            reason(gcReasonMessages.mediumResources, {
                memory: Math.round(memoryGb),
                cores: context.systemCpuCores,
            }),
        )
        reasonChain.push('→ Shenandoah')
        return { resolvedStrategy: 'shenandoah', reasonChain }
    }

    reasonChain.push(
        reason(gcReasonMessages.highResources, {
            memory: Math.round(memoryGb),
            cores: context.systemCpuCores,
        }),
    )

    if (javaVersion < 21) {
        reasonChain.push(reason(gcReasonMessages.zgcNonGenerational))
        reasonChain.push('→ Shenandoah')
        return { resolvedStrategy: 'shenandoah', reasonChain }
    }

    if (context.allocatedMemoryMb >= 10240 && context.systemCpuCores > 12) {
        reasonChain.push(reason(gcReasonMessages.ampleMemoryHighCores))
        reasonChain.push('→ ZGC')
        return { resolvedStrategy: 'zgc', reasonChain }
    }

    reasonChain.push(reason(gcReasonMessages.belowZgcRecommendation))
    reasonChain.push('→ Shenandoah')
    return { resolvedStrategy: 'shenandoah', reasonChain }
}

export function getResolvedStrategyName(strategyId: ResolvedGcStrategyId): string {
    const names: Record<ResolvedGcStrategyId, string> = {
        'g1gc-mojang': 'Mojang G1GC',
        pcl: 'PCL',
        shenandoah: 'Shenandoah',
        zgc: 'ZGC',
    }
    return names[strategyId]
}

/* ----------------------------------------------------------------- gc-presets.ts */

const GC_WIKI_URL = 'https://docs.oracle.com/en/java/javase/21/gctuning/introduction.html'
const G1GC_DOCS_URL =
    'https://docs.oracle.com/en/java/javase/21/gctuning/garbage-collector-implementation.html'
const SHENANDOAH_DOCS_URL = 'https://wiki.openjdk.org/display/shenandoah/Main'
const ZGC_DOCS_URL = 'https://wiki.openjdk.org/display/zgc/Main'

function createGcPresets(gcContext?: GcContext): JavaArgumentPreset[] {
    const autoResolution = gcContext ? resolveAutoGcStrategy(gcContext) : null
    const autoResolvedName = autoResolution
        ? getResolvedStrategyName(autoResolution.resolvedStrategy)
        : 'Mojang G1GC'

    return [
        {
            id: 'gc-auto',
            group: 'gc',
            title: javaPresetMessages.gcAutoTitle,
            description: javaPresetMessages.gcAutoDescription,
            args: AUTO_GC_PRESET_ARG,
            resolveArgs: () => AUTO_GC_PRESET_ARG,
            detect: (currentArgs) => currentArgs.includes(AUTO_GC_PRESET_ARG),
            link: GC_WIKI_URL,
            autoResolvedName,
            autoReasonChain: autoResolution?.reasonChain,
        },
        {
            id: 'gc-g1gc-mojang',
            group: 'gc',
            title: javaPresetMessages.gcG1gcMojangTitle,
            description: javaPresetMessages.gcG1gcMojangDescription,
            args: GC_STRATEGY_DEFINITIONS['g1gc-mojang'].baseArgs,
            resolveArgs: () => GC_STRATEGY_DEFINITIONS['g1gc-mojang'].baseArgs,
            detect: GC_STRATEGY_DEFINITIONS['g1gc-mojang'].detect,
            link: G1GC_DOCS_URL,
        },
        {
            id: 'gc-pcl',
            group: 'gc',
            title: javaPresetMessages.gcPclTitle,
            description: javaPresetMessages.gcPclDescription,
            args: GC_STRATEGY_DEFINITIONS.pcl.baseArgs,
            resolveArgs: () => GC_STRATEGY_DEFINITIONS.pcl.baseArgs,
            detect: GC_STRATEGY_DEFINITIONS.pcl.detect,
            link: SHENANDOAH_DOCS_URL,
        },
        {
            id: 'gc-shenandoah',
            group: 'gc',
            title: javaPresetMessages.gcShenandoahTitle,
            description: javaPresetMessages.gcShenandoahDescription,
            args: GC_STRATEGY_DEFINITIONS.shenandoah.baseArgs,
            resolveArgs: () => GC_STRATEGY_DEFINITIONS.shenandoah.baseArgs,
            detect: GC_STRATEGY_DEFINITIONS.shenandoah.detect,
            link: SHENANDOAH_DOCS_URL,
        },
        {
            id: 'gc-zgc',
            group: 'gc',
            title: javaPresetMessages.gcZgcTitle,
            description: javaPresetMessages.gcZgcDescription,
            args: GC_STRATEGY_DEFINITIONS.zgc.buildArgs(gcContext),
            resolveArgs: (context) => GC_STRATEGY_DEFINITIONS.zgc.buildArgs(context),
            detect: GC_STRATEGY_DEFINITIONS.zgc.detect,
            link: ZGC_DOCS_URL,
        },
    ]
}

/* ------------------------------------------------------ java-argument-presets.ts */

const JAVA_ARGUMENT_PRESET_GROUP_TITLES: Record<string, MessageDescriptor> = {
    gc: javaPresetMessages.gcGroupTitle,
    auth: javaPresetMessages.authGroupTitle,
}

const JAVA_ARGUMENT_PRESETS: JavaArgumentPreset[] = [
    {
        id: 'mojang-auth-mirror',
        group: 'auth',
        title: javaPresetMessages.authMirrorTitle,
        description: javaPresetMessages.authMirrorDescription,
        args: FALLEN_AUTH_PROXY_JAVA_ARGS_STRING,
        link: FALLEN_AUTH_PROXY_BLOG_URL,
    },
]

export function getJavaArgumentPresets(gcContext?: GcContext): JavaArgumentPreset[] {
    return [...JAVA_ARGUMENT_PRESETS, ...createGcPresets(gcContext)]
}

export interface JavaArgumentPresetGroup {
    group: string
    title: MessageDescriptor
    presets: JavaArgumentPreset[]
}

export function getPresetsByGroup(presets: JavaArgumentPreset[]): JavaArgumentPresetGroup[] {
    const groups = new Map<string, JavaArgumentPreset[]>()
    for (const preset of presets) {
        const group = preset.group
        if (!groups.has(group)) {
            groups.set(group, [])
        }
        groups.get(group)!.push(preset)
    }
    return Array.from(groups, ([group, groupPresets]) => ({
        group,
        title: JAVA_ARGUMENT_PRESET_GROUP_TITLES[group] ?? JAVA_ARGUMENT_PRESET_GROUP_TITLES.auth,
        presets: groupPresets,
    }))
}

/* -------------------------------------------------------------------- context.ts */

export function collectGcContext(
    allocatedMemoryMb: number,
    loader: InstanceLoader | null,
    systemCpuCores: number,
    javaMajorVersion?: number | null,
    modCount?: number,
): GcContext {
    return {
        javaMajorVersion: javaMajorVersion ?? null,
        allocatedMemoryMb,
        systemCpuCores,
        systemLogicalProcessors: systemCpuCores,
        modCount: modCount ?? 0,
        loader: loader ?? 'vanilla',
    }
}

export function extractJavaMajorVersion(
    parsedVersion: string | number | null | undefined,
): number | null {
    if (parsedVersion === null || parsedVersion === undefined) return null
    if (typeof parsedVersion === 'number') {
        return Number.isNaN(parsedVersion) ? null : parsedVersion
    }
    if (typeof parsedVersion === 'string') {
        const match = parsedVersion.match(/^(?:1\.)?(\d+)/)
        if (!match) return null
        const num = parseInt(match[1], 10)
        return Number.isNaN(num) ? null : num
    }
    return null
}
