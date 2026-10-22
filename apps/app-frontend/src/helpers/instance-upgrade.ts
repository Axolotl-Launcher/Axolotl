import { invoke } from '@tauri-apps/api/core'

import type { InstallJobSnapshot } from './install'
import type { InstanceLoader } from './types'

export type ContentProvider = 'modrinth' | 'curseforge' | 'local'
export type InstanceUpgradeProjectType =
    'mod' | 'datapack' | 'resourcepack' | 'shaderpack' | 'schematic' | 'worldsave'
export type ShaderRuntime = 'iris' | 'opti_fine' | 'none' | 'unknown'
export type InstanceUpgradeItemStatus =
    | 'upgrade_available'
    | 'already_compatible'
    | 'no_compatible_release'
    | 'prerelease_only'
    | 'unidentified'
    | 'dependency_conflict'
    | 'missing_required_dependency'
    | 'incompatible_dependency'
    | 'unsupported_content_type'
    | 'no_compatible_shader_runtime'
    | 'shader_runtime_missing'
    | 'shader_runtime_unknown'
export type InstanceUpgradeAction = 'upgrade' | 'keep' | 'disable'
export type InstanceUpgradeIssueCode =
    | 'prerelease_only'
    | 'unidentified'
    | 'dependency_conflict'
    | 'missing_required_dependency'
    | 'incompatible_dependency'
    | 'unsupported_content_type'
    | 'no_compatible_release'
    | 'no_compatible_shader_runtime'
    | 'shader_runtime_missing'
    | 'shader_runtime_unknown'
    | 'search_limit_reached'
    | 'keep_incompatible'
export type InstanceUpgradeDependencyChangeKind = 'add' | 'upgrade' | 'keep' | 'remove'
export type InstanceUpgradeSolutionKind = 'newest' | 'minimal_change' | 'custom'
export type InstanceUpgradeSolutionChoice = InstanceUpgradeSolutionKind
export type SharedUpgradeMode = 'direct' | 'copy_and_upgrade'
export type InstanceUpgradeExternalChangeKind = 'added' | 'removed' | 'modified'

export interface InstanceUpgradeTargetEnvironment {
    gameVersion: string
    modLoader: InstanceLoader
    modLoaderVersion: string | null
    shaderRuntime: ShaderRuntime
}

export interface InstanceUpgradePrereleaseConfirmation {
    provider: ContentProvider
    projectId: string
    versionId: string
}

export interface InstanceUpgradeResolution {
    contentId: string
    action: InstanceUpgradeAction
    allowPrerelease: boolean
    confirmedPrereleaseDependencies: InstanceUpgradePrereleaseConfirmation[]
}

export interface InstanceUpgradeResolutionResult {
    contentId: string
    code: string | null
    message: string | null
}

export interface InstanceUpgradeResolutionBatchResult {
    plan: InstanceUpgradePlan
    requestedCount: number
    applied: InstanceUpgradeResolutionResult[]
    skipped: InstanceUpgradeResolutionResult[]
    failed: InstanceUpgradeResolutionResult[]
}

export interface InstanceUpgradePlanItem {
    contentId: string
    relativePath: string
    projectType: InstanceUpgradeProjectType
    provider: ContentProvider | null
    projectId: string | null
    currentReleaseId: string | null
    currentEnabled: boolean
    autoDependency: boolean
    status: InstanceUpgradeItemStatus
    resolution: InstanceUpgradeResolution
    candidateReleaseIds: string[]
}

export interface InstanceUpgradeDependencyRequirement {
    rootContentId: string
    rootProvider: ContentProvider
    rootProjectId: string
    parentProvider: ContentProvider
    parentProjectId: string
    parentReleaseId: string
    dependencyProvider: ContentProvider
    dependencyProjectId: string
    requiredReleaseId: string | null
    candidateReleaseId: string | null
}

export interface InstanceUpgradeIssue {
    code: InstanceUpgradeIssueCode
    message: string
    contentId: string | null
    provider: ContentProvider | null
    projectId: string | null
    conflictingProjectId: string | null
    dependencyRequirements: InstanceUpgradeDependencyRequirement[]
}

export interface InstanceUpgradeDependencyChange {
    existingContentId: string | null
    provider: ContentProvider
    projectId: string
    currentReleaseId: string | null
    targetReleaseId: string | null
    kind: InstanceUpgradeDependencyChangeKind
    enabled: boolean
}

export interface InstanceUpgradeSourceFile {
    relativePath: string
    sha1: string
    size: number
    enabled: boolean
}

export interface InstanceUpgradeSelection {
    contentId: string
    provider: ContentProvider | null
    projectId: string | null
    currentReleaseId: string | null
    targetReleaseId: string | null
    action: InstanceUpgradeAction
    enabled: boolean
}

export interface InstanceUpgradeSolution {
    kind: InstanceUpgradeSolutionKind
    selections: InstanceUpgradeSelection[]
    dependencyChanges: InstanceUpgradeDependencyChange[]
    warnings: InstanceUpgradeIssue[]
}

export interface InstanceUpgradeFixedConstraint {
    contentId: string
    provider: ContentProvider
    projectId: string
    versionId: string
}

export interface InstanceUpgradePlan {
    id: string
    instanceId: string
    sourceRevision: number
    sourceFiles: InstanceUpgradeSourceFile[]
    sourceEnvironment: InstanceUpgradeTargetEnvironment
    targetEnvironment: InstanceUpgradeTargetEnvironment
    items: InstanceUpgradePlanItem[]
    dependencyChanges: InstanceUpgradeDependencyChange[]
    warnings: InstanceUpgradeIssue[]
    blockingIssues: InstanceUpgradeIssue[]
    newestSolution: InstanceUpgradeSolution | null
    minimalChangeSolution: InstanceUpgradeSolution | null
    selectedSolution: InstanceUpgradeSolution | null
    customConstraints: InstanceUpgradeFixedConstraint[]
}

export interface InstanceUpgradeExternalChange {
    relativePath: string
    kind: InstanceUpgradeExternalChangeKind
}

export interface InstanceUpgradeCompatibilityWarning {
    code: InstanceUpgradeIssueCode
    relativePath: string | null
    contentId: string | null
    provider: ContentProvider | null
    projectId: string | null
    conflictingProjectId: string | null
}

export interface InstanceUpgradeDisplayNames {
    backup: string | null
    copy: string | null
    upgradedTarget: string | null
    shouldAutoRename: boolean
}

export interface InstanceUpgradeResult {
    planId: string
    sourceInstanceId: string
    targetInstanceId: string
    backupInstanceId: string | null
    sourceEnvironment?: InstanceUpgradeTargetEnvironment | null
    targetEnvironment?: InstanceUpgradeTargetEnvironment | null
    solution: InstanceUpgradeSolution
    compatibilityWarnings: InstanceUpgradeIssue[]
    compatibilityWarningDetails?: InstanceUpgradeCompatibilityWarning[]
    externalChanges: InstanceUpgradeExternalChange[]
    skippedDueToExternalConflict: string[]
}

interface UpgradePlanBenchSample {
    run: number
    operation: string
    durationMs: number
    success: boolean
    error?: string
}

const upgradePlanBenchSamples: UpgradePlanBenchSample[] = []

function percentile(values: number[], p: number): number | null {
    if (!values.length) return null
    const sorted = [...values].sort((left, right) => left - right)
    return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * p) - 1)]
}

function upgradePlanBenchSummary() {
    const successful = upgradePlanBenchSamples.filter((sample) => sample.success)
    const durations = successful.map((sample) => sample.durationMs)
    const warmDurations = durations.slice(1)
    const average = (values: number[]) =>
        values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : null
    const summary = {
        runs: upgradePlanBenchSamples.length,
        successfulRuns: successful.length,
        coldMs: durations[0] ?? null,
        avgMs: average(durations),
        minMs: durations.length ? Math.min(...durations) : null,
        maxMs: durations.length ? Math.max(...durations) : null,
        warmAvgMs: average(warmDurations),
        warmMinMs: warmDurations.length ? Math.min(...warmDurations) : null,
        warmMaxMs: warmDurations.length ? Math.max(...warmDurations) : null,
        p50Ms: percentile(durations, 0.5),
        p95Ms: percentile(durations, 0.95),
    }
    console.table(upgradePlanBenchSamples)
    console.table(summary)
    return summary
}

async function benchmark_upgrade_plan<T>(operation: string, action: () => Promise<T>): Promise<T> {
    const startedAt = performance.now()
    try {
        const result = await action()
        if (import.meta.env.DEV) record_upgrade_plan_bench(operation, startedAt, true)
        return result
    } catch (error) {
        if (import.meta.env.DEV) {
            record_upgrade_plan_bench(
                operation,
                startedAt,
                false,
                error instanceof Error ? error.message : String(error),
            )
        }
        throw error
    }
}

function record_upgrade_plan_bench(
    operation: string,
    startedAt: number,
    success: boolean,
    error?: string,
) {
    const sample: UpgradePlanBenchSample = {
        run: upgradePlanBenchSamples.length + 1,
        operation,
        durationMs: Math.round((performance.now() - startedAt) * 100) / 100,
        success,
        error,
    }
    upgradePlanBenchSamples.push(sample)
    console.log(
        `[upgrade-plan-bench] #${sample.run} ${operation}: ${sample.durationMs.toFixed(2)} ms`,
    )
}

if (import.meta.env.DEV) {
    Object.assign(globalThis, {
        __AXOLOTL_UPGRADE_PLAN_BENCH__: {
            samples: upgradePlanBenchSamples,
            reset() {
                upgradePlanBenchSamples.length = 0
                console.log('[upgrade-plan-bench] reset')
            },
            summary: upgradePlanBenchSummary,
        },
    })
}

export async function plan_instance_upgrade(
    instanceId: string,
    targetEnvironment: InstanceUpgradeTargetEnvironment,
): Promise<InstanceUpgradePlan> {
    return await benchmark_upgrade_plan('plan', () =>
        invoke('plugin:instance|instance_plan_upgrade', { instanceId, targetEnvironment }),
    )
}

export async function select_instance_upgrade_solution(
    planId: string,
    choice: InstanceUpgradeSolutionChoice,
): Promise<InstanceUpgradePlan> {
    return await invoke('plugin:instance|instance_select_upgrade_solution', { planId, choice })
}

export async function update_instance_upgrade_resolution(
    planId: string,
    resolution: InstanceUpgradeResolution,
): Promise<InstanceUpgradePlan> {
    return await benchmark_upgrade_plan('update-resolution', () =>
        invoke('plugin:instance|instance_update_upgrade_resolution', { planId, resolution }),
    )
}

export async function update_instance_upgrade_resolutions(
    planId: string,
    resolutions: InstanceUpgradeResolution[],
): Promise<InstanceUpgradeResolutionBatchResult> {
    return await benchmark_upgrade_plan('update-resolutions', () =>
        invoke('plugin:instance|instance_update_upgrade_resolutions', {
            planId,
            resolutions,
        }),
    )
}

export async function reset_instance_upgrade_resolution(
    planId: string,
    contentId: string,
): Promise<InstanceUpgradePlan> {
    return await benchmark_upgrade_plan('reset-resolution', () =>
        invoke('plugin:instance|instance_reset_upgrade_resolution', {
            planId,
            contentId,
        }),
    )
}

export async function resolve_custom_instance_upgrade_solution(
    planId: string,
    fixedConstraints: InstanceUpgradeFixedConstraint[],
): Promise<InstanceUpgradePlan> {
    return await invoke('plugin:instance|instance_resolve_custom_upgrade_solution', {
        planId,
        fixedConstraints,
    })
}

export async function execute_instance_upgrade(
    planId: string,
    createFullBackup: boolean,
    sharedUpgradeMode: SharedUpgradeMode,
    displayNames: InstanceUpgradeDisplayNames,
): Promise<InstallJobSnapshot> {
    return await invoke('plugin:instance|instance_execute_upgrade', {
        planId,
        createFullBackup,
        sharedUpgradeMode,
        displayNames,
    })
}

export async function discard_instance_upgrade_plan(planId: string): Promise<void> {
    await invoke('plugin:instance|instance_discard_upgrade_plan', { planId })
}

export async function get_instance_upgrade_plan(planId: string): Promise<InstanceUpgradePlan> {
    return await invoke('plugin:instance|instance_get_upgrade_plan', { planId })
}
