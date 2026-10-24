import type { InstanceLoader } from '../../../lib/types'

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
