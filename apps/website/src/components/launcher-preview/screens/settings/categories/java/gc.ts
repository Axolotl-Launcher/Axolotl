import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'

import type { InstanceLoader } from '../../../../lib/types'
import { messages } from '../../messages'
import { PREVIEW_CPU_THREADS } from './mac'

/**
 * The app's GC strategy helpers and Java argument presets
 * (apps/app-frontend/src/helpers/gc/{types,strategies,auto-selector,gc-presets,context}.ts,
 * helpers/java-argument-presets.ts, helpers/java-arguments.ts, helpers/gc-notice.ts).
 */

/* ---- helpers/java-arguments.ts ---------------------------------------------- */

export const FALLEN_AUTH_PROXY_BLOG_URL =
    'https://blog.fallenbreath.me/zh-CN/2025/minecraft-service-proxy'

export const FALLEN_AUTH_PROXY_JAVA_ARGS = [
    '-Dminecraft.api.auth.host=https://auth.msp.fallenbreath.me',
    '-Dminecraft.api.account.host=https://account.msp.fallenbreath.me',
    '-Dminecraft.api.session.host=https://session.msp.fallenbreath.me',
    '-Dminecraft.api.services.host=https://services.msp.fallenbreath.me',
    '-Dminecraft.api.profiles.host=https://profiles.msp.fallenbreath.me',
]

export const FALLEN_AUTH_PROXY_JAVA_ARGS_STRING = FALLEN_AUTH_PROXY_JAVA_ARGS.join(' ')

export const AUTO_GC_PRESET_ARG = '@axolotl:gc:auto'

/* ---- helpers/gc/types.ts ----------------------------------------------------- */

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

export interface GcStrategyDefinition {
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

/* ---- helpers/gc/strategies.ts ------------------------------------------------- */

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

/* ---- helpers/gc/auto-selector.ts ---------------------------------------------- */

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
        reasonChain.push(reason(messages.appJavaArgumentsGcReasonJavaVersionUnknown))
    }

    if (javaVersion === null || javaVersion < 15) {
        reasonChain.push(reason(messages.appJavaArgumentsGcReasonJavaTooOld))
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    if (context.allocatedMemoryMb < 4096) {
        reasonChain.push(
            reason(messages.appJavaArgumentsGcReasonInsufficientMemory, {
                allocated: Math.round(context.allocatedMemoryMb / 1024),
            }),
        )
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    if (context.systemCpuCores <= 4 && context.systemLogicalProcessors <= 8) {
        reasonChain.push(
            reason(messages.appJavaArgumentsGcReasonInsufficientCpu, {
                cores: context.systemCpuCores,
                threads: context.systemLogicalProcessors,
            }),
        )
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    if (context.modCount >= 200 && context.allocatedMemoryMb < 8192) {
        reasonChain.push(
            reason(messages.appJavaArgumentsGcReasonLargeModpackLowMemory, {
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
            reason(messages.appJavaArgumentsGcReasonLightweightInstance, {
                loader: context.loader,
                mods: context.modCount,
            }),
        )
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    reasonChain.push(
        reason(messages.appJavaArgumentsGcReasonHeavyInstance, {
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
            reason(messages.appJavaArgumentsGcReasonLowResources, {
                memory: Math.round(memoryGb),
                cores: context.systemCpuCores,
            }),
        )
        return { resolvedStrategy: 'g1gc-mojang', reasonChain }
    }

    if (isResourceMedium) {
        reasonChain.push(
            reason(messages.appJavaArgumentsGcReasonMediumResources, {
                memory: Math.round(memoryGb),
                cores: context.systemCpuCores,
            }),
        )
        reasonChain.push('→ Shenandoah')
        return { resolvedStrategy: 'shenandoah', reasonChain }
    }

    reasonChain.push(
        reason(messages.appJavaArgumentsGcReasonHighResources, {
            memory: Math.round(memoryGb),
            cores: context.systemCpuCores,
        }),
    )

    if (javaVersion < 21) {
        reasonChain.push(reason(messages.appJavaArgumentsGcReasonZgcNonGenerational))
        reasonChain.push('→ Shenandoah')
        return { resolvedStrategy: 'shenandoah', reasonChain }
    }

    if (context.allocatedMemoryMb >= 10240 && context.systemCpuCores > 12) {
        reasonChain.push(reason(messages.appJavaArgumentsGcReasonAmpleMemoryHighCores))
        reasonChain.push('→ ZGC')
        return { resolvedStrategy: 'zgc', reasonChain }
    }

    reasonChain.push(reason(messages.appJavaArgumentsGcReasonBelowZgcRecommendation))
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

/* ---- helpers/gc/gc-presets.ts ------------------------------------------------- */

const GC_WIKI_URL = 'https://docs.oracle.com/en/java/javase/21/gctuning/introduction.html'
const G1GC_DOCS_URL =
    'https://docs.oracle.com/en/java/javase/21/gctuning/garbage-collector-implementation.html'
const SHENANDOAH_DOCS_URL = 'https://wiki.openjdk.org/display/shenandoah/Main'
const ZGC_DOCS_URL = 'https://wiki.openjdk.org/display/zgc/Main'

export function createGcPresets(gcContext?: GcContext): JavaArgumentPreset[] {
    const autoResolution = gcContext ? resolveAutoGcStrategy(gcContext) : null
    const autoResolvedName = autoResolution
        ? getResolvedStrategyName(autoResolution.resolvedStrategy)
        : 'Mojang G1GC'

    return [
        {
            id: 'gc-auto',
            group: 'gc',
            title: messages.appJavaArgumentsPresetsGcAutoTitle,
            description: messages.appJavaArgumentsPresetsGcAutoDescription,
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
            title: messages.appJavaArgumentsPresetsGcG1gcMojangTitle,
            description: messages.appJavaArgumentsPresetsGcG1gcMojangDescription,
            args: GC_STRATEGY_DEFINITIONS['g1gc-mojang'].baseArgs,
            resolveArgs: () => GC_STRATEGY_DEFINITIONS['g1gc-mojang'].baseArgs,
            detect: GC_STRATEGY_DEFINITIONS['g1gc-mojang'].detect,
            link: G1GC_DOCS_URL,
        },
        {
            id: 'gc-pcl',
            group: 'gc',
            title: messages.appJavaArgumentsPresetsGcG1gcPclTitle,
            description: messages.appJavaArgumentsPresetsGcG1gcPclDescription,
            args: GC_STRATEGY_DEFINITIONS.pcl.baseArgs,
            resolveArgs: () => GC_STRATEGY_DEFINITIONS.pcl.baseArgs,
            detect: GC_STRATEGY_DEFINITIONS.pcl.detect,
            link: SHENANDOAH_DOCS_URL,
        },
        {
            id: 'gc-shenandoah',
            group: 'gc',
            title: messages.appJavaArgumentsPresetsGcShenandoahTitle,
            description: messages.appJavaArgumentsPresetsGcShenandoahDescription,
            args: GC_STRATEGY_DEFINITIONS.shenandoah.baseArgs,
            resolveArgs: () => GC_STRATEGY_DEFINITIONS.shenandoah.baseArgs,
            detect: GC_STRATEGY_DEFINITIONS.shenandoah.detect,
            link: SHENANDOAH_DOCS_URL,
        },
        {
            id: 'gc-zgc',
            group: 'gc',
            title: messages.appJavaArgumentsPresetsGcZgcTitle,
            description: messages.appJavaArgumentsPresetsGcZgcDescription,
            args: GC_STRATEGY_DEFINITIONS.zgc.buildArgs(gcContext),
            resolveArgs: (context) => GC_STRATEGY_DEFINITIONS.zgc.buildArgs(context),
            detect: GC_STRATEGY_DEFINITIONS.zgc.detect,
            link: ZGC_DOCS_URL,
        },
    ]
}

/* ---- helpers/java-argument-presets.ts ---------------------------------------- */

export const JAVA_ARGUMENT_PRESET_GROUP_TITLES: Record<string, MessageDescriptor> = {
    gc: messages.appJavaArgumentsPresetsGcGroupTitle,
    auth: messages.appJavaArgumentsPresetsAuthGroupTitle,
}

export const JAVA_ARGUMENT_PRESETS: JavaArgumentPreset[] = [
    {
        id: 'mojang-auth-mirror',
        group: 'auth',
        title: messages.appJavaArgumentsPresetsAuthMirrorTitle,
        description: messages.appJavaArgumentsPresetsAuthMirrorDescription,
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

/* ---- helpers/gc/context.ts --------------------------------------------------- */

/** `collectGcContext`: the webview's `navigator.hardwareConcurrency` is this Mac's. */
export async function collectGcContext(
    allocatedMemoryMb: number,
    loader: InstanceLoader | null,
    javaMajorVersion?: number | null,
    modCount?: number,
): Promise<GcContext> {
    const systemCpuCores = PREVIEW_CPU_THREADS
    const systemLogicalProcessors = systemCpuCores

    return {
        javaMajorVersion: javaMajorVersion ?? null,
        allocatedMemoryMb,
        systemCpuCores,
        systemLogicalProcessors,
        modCount: modCount ?? 0,
        loader: loader ?? 'vanilla',
    }
}

/* ---- helpers/gc-notice.ts ---------------------------------------------------- */

/** The GC report of the most recent launch (`GcLaunchReport`). */
export interface GcLaunchReport {
    chosen_strategy: string | null
    pruned_args: string[]
}

/**
 * The GC launch report from the most recent launch. The preview's launches do not
 * report one, so it stays empty, as it is before the first launch in the app.
 */
export const lastGcLaunchReport: { readonly value: GcLaunchReport | null } = Object.freeze({
    value: null,
})
