import type { ContentItem } from '@modrinth/ui/src/layouts/shared/content-tab/types.ts'
import type { GameVersionTag } from '@modrinth/utils'

import type { GameInstance } from '../../../lib/types'
import type {
    InstanceUpgradeDependencyChange,
    InstanceUpgradeFixedConstraint,
    InstanceUpgradeIssue,
    InstanceUpgradePlan,
    InstanceUpgradePlanItem,
    InstanceUpgradeSolution,
    InstanceUpgradeTargetEnvironment,
    ShaderRuntime,
} from './backend'
import type { InstanceContentSnapshot, InstanceContentSnapshotItem } from './content'
import { compareSemanticVersions } from './version-compatibility.ts'

export const AUTOMATIC_FABRIC_LOADER_VERSION = '__automatic__'

export interface UpgradeVersionTargets {
    currentFound: boolean
    versions: string[]
}

export interface CompatibilitySummary {
    updates: number
    keptOrCompatible: number
    disabled: number
    dependencyChanges: number
    needsAttention: number
}

export interface SolutionSummary {
    upgraded: number
    kept: number
    disabled: number
    dependencyAdditions: number
    dependencyUpdates: number
    dependencyRemovals: number
}

export interface ConfirmSolutionGroups {
    updated: InstanceUpgradeSolution['selections']
    kept: InstanceUpgradeSolution['selections']
    disabled: InstanceUpgradeSolution['selections']
    dependencyChanges: InstanceUpgradeDependencyChange[]
}

export interface ConfirmUpgradeOptions {
    effectiveMode: 'direct' | 'copy_and_upgrade' | null
    createFullBackup: boolean
    canStart: boolean
}

export interface ConfirmReleaseSlots {
    currentReleaseId: string | null
    targetReleaseId: string | null
}

export interface ConfirmResolvedReleaseSlots extends ConfirmReleaseSlots {
    current: string | null
    target: string | null
}

export interface UpgradeContentIssueGroup {
    item: InstanceUpgradePlanItem
    blockingIssues: InstanceUpgradeIssue[]
    warnings: InstanceUpgradeIssue[]
    startedBlocking: boolean
    currentlyBlocking: boolean
}

export interface UpgradeIssueGroups {
    blocking: UpgradeContentIssueGroup[]
    warnings: UpgradeContentIssueGroup[]
    noIssues: UpgradeContentIssueGroup[]
    globalBlockingIssues: InstanceUpgradeIssue[]
    globalWarnings: InstanceUpgradeIssue[]
}

export interface UpgradeContentDisplayMetadata {
    title: string
    iconUrl: string | null
    currentVersion: string | null
}

export type InitialUpgradeBlockingIssues = Record<string, InstanceUpgradeIssue[]>

export interface UpgradeResolutionPresentation {
    selectedAction: 'upgrade' | 'keep' | 'disable' | null
    showUndo: boolean
}

export function normalizeUpgradePath(path: string): string {
    return path.replaceAll('\\', '/').replace(/\/+/g, '/').replace(/^\.\//, '')
}

export function upgradeTargetsEqual(
    left: InstanceUpgradeTargetEnvironment,
    right: InstanceUpgradeTargetEnvironment,
): boolean {
    return (
        left.gameVersion === right.gameVersion &&
        left.modLoader === right.modLoader &&
        left.modLoaderVersion === right.modLoaderVersion &&
        left.shaderRuntime === right.shaderRuntime
    )
}

export function shouldReuseUpgradePlan(
    instanceId: string,
    plan: InstanceUpgradePlan | null,
    target: InstanceUpgradeTargetEnvironment | null,
): boolean {
    return (
        plan !== null &&
        target !== null &&
        plan.instanceId === instanceId &&
        upgradeTargetsEqual(plan.targetEnvironment, target)
    )
}

export async function resolveUpgradePlanSelection(
    instanceId: string,
    existingPlan: InstanceUpgradePlan | null,
    target: InstanceUpgradeTargetEnvironment,
    planUpgrade: (
        instanceId: string,
        target: InstanceUpgradeTargetEnvironment,
    ) => Promise<InstanceUpgradePlan>,
): Promise<{ plan: InstanceUpgradePlan; reused: boolean }> {
    if (shouldReuseUpgradePlan(instanceId, existingPlan, target)) {
        return { plan: existingPlan!, reused: true }
    }
    return { plan: await planUpgrade(instanceId, target), reused: false }
}

export async function commitUpgradePlanSelection(
    instanceId: string,
    existingPlan: InstanceUpgradePlan | null,
    target: InstanceUpgradeTargetEnvironment,
    planUpgrade: (
        instanceId: string,
        target: InstanceUpgradeTargetEnvironment,
    ) => Promise<InstanceUpgradePlan>,
    commitPlan: (plan: InstanceUpgradePlan) => void,
    commitTarget: (target: InstanceUpgradeTargetEnvironment) => void,
): Promise<{ plan: InstanceUpgradePlan; reused: boolean }> {
    const result = await resolveUpgradePlanSelection(instanceId, existingPlan, target, planUpgrade)
    if (!result.reused) commitPlan(result.plan)
    commitTarget(result.plan.targetEnvironment)
    return result
}

export function fabricUpgradeLoaderVersions(
    currentVersion: string | null | undefined,
    availableVersions: readonly string[],
): string[] {
    if (!currentVersion || compareSemanticVersions(currentVersion, currentVersion) === null)
        return []
    return availableVersions.filter((version) => {
        const comparison = compareSemanticVersions(version, currentVersion)
        return comparison !== null && comparison >= 0
    })
}

export function preserveFabricLoaderSelection(
    selectedVersion: string,
    availableVersions: readonly string[],
): string {
    return selectedVersion === AUTOMATIC_FABRIC_LOADER_VERSION ||
        availableVersions.includes(selectedVersion)
        ? selectedVersion
        : AUTOMATIC_FABRIC_LOADER_VERSION
}

export function fabricLoaderVersionForTarget(selectedVersion: string): string | null {
    return selectedVersion === AUTOMATIC_FABRIC_LOADER_VERSION ? null : selectedVersion
}

export function automaticFabricLoaderTargetAvailable(
    metadataLoaded: boolean,
    currentVersionComparable: boolean,
    availableVersions: readonly string[],
): boolean {
    return !metadataLoaded || !currentVersionComparable || availableVersions.length > 0
}

export function isSharedUpgradeInstance(instance: GameInstance): boolean {
    return instance.link?.type === 'shared_instance' || Boolean(instance.symlink_target)
}

export function contentIdentityKeys(item: {
    contentId?: string | null
    relativePath?: string | null
    instanceEntryId?: string | null
    instanceMemberId?: string | null
    instanceFileId?: string | null
    id?: string | null
    file_path?: string | null
}): string[] {
    return [
        item.contentId,
        item.instanceEntryId,
        item.instanceMemberId,
        item.instanceFileId,
        item.id,
        item.relativePath ? normalizeUpgradePath(item.relativePath) : null,
        item.file_path ? normalizeUpgradePath(item.file_path) : null,
    ].filter((value): value is string => Boolean(value))
}

const ACTIONABLE_WARNING_CODES = new Set<InstanceUpgradeIssue['code']>([
    'unidentified',
    'unsupported_content_type',
    'prerelease_only',
    'no_compatible_release',
    'no_compatible_shader_runtime',
    'shader_runtime_missing',
    'shader_runtime_unknown',
    'keep_incompatible',
])

function issueIdentity(issue: InstanceUpgradeIssue): string {
    const requirements = issue.dependencyRequirements
        .map((requirement) =>
            [
                requirement.rootContentId,
                requirement.parentProvider,
                requirement.parentProjectId,
                requirement.parentReleaseId,
                requirement.dependencyProvider,
                requirement.dependencyProjectId,
                requirement.requiredReleaseId ?? '',
                requirement.candidateReleaseId ?? '',
            ].join(':'),
        )
        .sort()
        .join('|')
    return [
        issue.code,
        issue.provider ?? '',
        issue.projectId ?? '',
        issue.conflictingProjectId ?? '',
        requirements,
    ].join(':')
}

function issueContentId(
    issue: InstanceUpgradeIssue,
    itemsById: Map<string, InstanceUpgradePlanItem>,
    itemsByProject: Map<string, InstanceUpgradePlanItem | null>,
): string | null {
    if (issue.contentId && itemsById.has(issue.contentId)) return issue.contentId
    if (!issue.projectId) return null
    const providerProject = `${issue.provider ?? ''}:${issue.projectId}`
    return itemsByProject.get(providerProject)?.contentId ?? null
}

export function captureInitialUpgradeBlockingIssues(
    plan: InstanceUpgradePlan,
): InitialUpgradeBlockingIssues {
    return Object.fromEntries(
        groupUpgradeIssues(plan).blocking.map((group) => [
            group.item.contentId,
            group.blockingIssues,
        ]),
    )
}

export function groupUpgradeIssues(
    plan: InstanceUpgradePlan,
    initialBlockingIssues: InitialUpgradeBlockingIssues = {},
): UpgradeIssueGroups {
    const itemsById = new Map(plan.items.map((item) => [item.contentId, item]))
    const itemsByProject = new Map<string, InstanceUpgradePlanItem | null>()
    for (const item of plan.items) {
        if (!item.projectId) continue
        const key = `${item.provider ?? ''}:${item.projectId}`
        itemsByProject.set(key, itemsByProject.has(key) ? null : item)
    }

    const blockingByContent = new Map<string, Map<string, InstanceUpgradeIssue>>()
    const warningByContent = new Map<string, Map<string, InstanceUpgradeIssue>>()
    const globalBlockingIssues: InstanceUpgradeIssue[] = []
    const globalWarnings: InstanceUpgradeIssue[] = []

    function collect(
        issue: InstanceUpgradeIssue,
        byContent: Map<string, Map<string, InstanceUpgradeIssue>>,
        global: InstanceUpgradeIssue[],
    ) {
        const contentId = issueContentId(issue, itemsById, itemsByProject)
        if (!contentId) {
            global.push(issue)
            return
        }
        const issues = byContent.get(contentId) ?? new Map<string, InstanceUpgradeIssue>()
        const key = issueIdentity(issue)
        const existing = issues.get(key)
        if (!existing || (existing.contentId === null && issue.contentId !== null))
            issues.set(key, issue)
        byContent.set(contentId, issues)
    }

    for (const issue of plan.blockingIssues) collect(issue, blockingByContent, globalBlockingIssues)
    for (const issue of plan.warnings) collect(issue, warningByContent, globalWarnings)

    const blocking: UpgradeContentIssueGroup[] = []
    const warnings: UpgradeContentIssueGroup[] = []
    const noIssues: UpgradeContentIssueGroup[] = []
    for (const item of plan.items) {
        const itemBlocking = [...(blockingByContent.get(item.contentId)?.values() ?? [])]
        const blockingKeys = new Set(itemBlocking.map(issueIdentity))
        const itemWarnings = [...(warningByContent.get(item.contentId)?.values() ?? [])].filter(
            (issue) => !blockingKeys.has(issueIdentity(issue)),
        )
        const initialIssues = initialBlockingIssues[item.contentId] ?? []
        const startedBlocking = initialIssues.length > 0
        const currentlyBlocking = itemBlocking.length > 0
        const contextualWarnings = currentlyBlocking
            ? itemWarnings
            : [
                  ...new Map(
                      [...initialIssues, ...itemWarnings].map((issue) => [
                          issueIdentity(issue),
                          issue,
                      ]),
                  ).values(),
              ]
        const group = {
            item,
            blockingIssues: itemBlocking,
            warnings: contextualWarnings,
            startedBlocking,
            currentlyBlocking,
        }
        if (currentlyBlocking || startedBlocking) blocking.push(group)
        else if (itemWarnings.length) warnings.push(group)
        else noIssues.push(group)
    }

    return { blocking, warnings, noIssues, globalBlockingIssues, globalWarnings }
}

export function actionableWarningContentIds(groups: UpgradeIssueGroups): string[] {
    return groups.warnings
        .filter((group) => group.warnings.some((issue) => ACTIONABLE_WARNING_CODES.has(issue.code)))
        .map((group) => group.item.contentId)
}

export function upgradeContentDisplayMetadata(
    item: InstanceUpgradePlanItem,
    contentItem?: ContentItem,
    snapshotItem?: InstanceContentSnapshotItem,
): UpgradeContentDisplayMetadata {
    const fallbackPath = snapshotItem?.expectedRelativePath ?? item.relativePath
    const fallbackName = fallbackPath.split('/').pop() ?? fallbackPath
    return {
        title: sanitizeMinecraftDisplayTitle(
            contentItem?.project.title ?? snapshotItem?.content?.project.title ?? fallbackName,
        ),
        iconUrl: contentItem?.project.icon_url ?? snapshotItem?.content?.project.icon_url ?? null,
        currentVersion:
            contentItem?.version?.version_number ??
            snapshotItem?.content?.version?.version_number ??
            item.currentReleaseId,
    }
}

export function sanitizeMinecraftDisplayTitle(title: string): string {
    return title.replace(/§[0-9a-fk-or]/gi, '')
}

export function upgradeResolutionPresentation(
    kind: 'two-option' | 'single-prerelease',
    resolution: InstanceUpgradePlanItem['resolution'],
): UpgradeResolutionPresentation {
    if (kind === 'single-prerelease') {
        return {
            selectedAction: resolution.allowPrerelease ? 'upgrade' : null,
            showUndo: resolution.allowPrerelease,
        }
    }
    return {
        selectedAction:
            resolution.action === 'keep' || resolution.action === 'disable'
                ? resolution.action
                : null,
        showUndo: false,
    }
}

export function availablePredefinedStrategies(plan: InstanceUpgradePlan) {
    return [
        ...(plan.newestSolution ? (['newest'] as const) : []),
        ...(plan.minimalChangeSolution ? (['minimal_change'] as const) : []),
    ]
}

const IRIS_MODRINTH_PROJECT_ID = 'YL57xq9U'

export function inferShaderRuntime(
    instance: GameInstance,
    snapshot: InstanceContentSnapshot | undefined,
): ShaderRuntime {
    if (
        instance.loader === 'optifine' ||
        instance.loader_components.some((component) => component.kind === 'optifine')
    ) {
        return 'opti_fine'
    }
    if (!snapshot) return 'unknown'

    const hasIris = snapshot.items.some(
        (item) =>
            (item.provider === 'modrinth' && item.providerProjectId === IRIS_MODRINTH_PROJECT_ID) ||
            item.content?.provider_refs.some(
                (reference) =>
                    reference.provider === 'modrinth' &&
                    reference.project_id === IRIS_MODRINTH_PROJECT_ID,
            ),
    )
    if (hasIris) return 'iris'

    const hasUnresolvedModIdentity = snapshot.items.some(
        (item) =>
            item.projectType === 'mod' &&
            (item.provider !== 'modrinth' || item.providerProjectId === null),
    )
    return hasUnresolvedModIdentity ? 'unknown' : 'none'
}

export function newerStableGameVersions(
    metadata: GameVersionTag[],
    currentVersion: string,
): UpgradeVersionTargets {
    const currentIndex = metadata.findIndex((version) => version.version === currentVersion)
    const candidates = currentIndex === -1 ? metadata : metadata.slice(0, currentIndex)
    return {
        currentFound: currentIndex !== -1,
        versions: candidates
            .filter(
                (version) =>
                    version.version_type === 'release' && version.version !== currentVersion,
            )
            .map((version) => version.version),
    }
}

function summarizeSelections(solution: InstanceUpgradeSolution) {
    return solution.selections.reduce(
        (summary, selection) => {
            if (selection.action === 'disable') summary.disabled += 1
            else if (
                selection.action === 'upgrade' &&
                selection.targetReleaseId !== null &&
                selection.targetReleaseId !== selection.currentReleaseId
            ) {
                summary.updates += 1
            } else summary.keptOrCompatible += 1
            return summary
        },
        { updates: 0, keptOrCompatible: 0, disabled: 0 },
    )
}

export function solutionSummary(solution: InstanceUpgradeSolution): SolutionSummary {
    const selections = summarizeSelections(solution)
    return {
        upgraded: selections.updates,
        kept: selections.keptOrCompatible,
        disabled: selections.disabled,
        dependencyAdditions: solution.dependencyChanges.filter((change) => change.kind === 'add')
            .length,
        dependencyUpdates: solution.dependencyChanges.filter((change) => change.kind === 'upgrade')
            .length,
        dependencyRemovals: solution.dependencyChanges.filter((change) => change.kind === 'remove')
            .length,
    }
}

export function confirmSolutionGroups(solution: InstanceUpgradeSolution): ConfirmSolutionGroups {
    return {
        updated: solution.selections.filter(
            (selection) =>
                selection.action === 'upgrade' &&
                selection.targetReleaseId !== null &&
                selection.targetReleaseId !== selection.currentReleaseId,
        ),
        kept: solution.selections.filter(
            (selection) =>
                selection.action !== 'disable' &&
                (selection.action !== 'upgrade' ||
                    selection.targetReleaseId === null ||
                    selection.targetReleaseId === selection.currentReleaseId),
        ),
        disabled: solution.selections.filter((selection) => selection.action === 'disable'),
        dependencyChanges: solution.dependencyChanges.filter((change) => change.kind !== 'keep'),
    }
}

export function confirmUpgradeOptions(
    sharedInstance: boolean,
    sharedMode: 'direct' | 'copy_and_upgrade' | null,
    directFullBackupPreference: boolean,
): ConfirmUpgradeOptions {
    const effectiveMode = sharedInstance ? sharedMode : 'direct'
    return {
        effectiveMode,
        createFullBackup: effectiveMode === 'copy_and_upgrade' ? false : directFullBackupPreference,
        canStart: !sharedInstance || sharedMode !== null,
    }
}

export function confirmSelectionReleaseSlots(
    selection: InstanceUpgradeSolution['selections'][number],
): ConfirmReleaseSlots {
    return {
        currentReleaseId: selection.currentReleaseId,
        targetReleaseId:
            selection.action === 'upgrade' &&
            selection.targetReleaseId !== selection.currentReleaseId
                ? selection.targetReleaseId
                : null,
    }
}

export function confirmDependencyReleaseSlots(
    change: InstanceUpgradeDependencyChange,
): ConfirmReleaseSlots {
    return {
        currentReleaseId: change.currentReleaseId,
        targetReleaseId:
            change.targetReleaseId !== change.currentReleaseId ? change.targetReleaseId : null,
    }
}

export function resolveConfirmDependencyReleases(
    change: InstanceUpgradeDependencyChange,
    resolveLabel: (releaseId: string | null, slot: 'current' | 'target') => string | null,
): ConfirmResolvedReleaseSlots {
    const releases = confirmDependencyReleaseSlots(change)
    return {
        ...releases,
        current: resolveLabel(releases.currentReleaseId, 'current'),
        target: resolveLabel(releases.targetReleaseId, 'target'),
    }
}

export function confirmTargetLoaderLabel(
    loaderLabel: string,
    loader: InstanceUpgradeTargetEnvironment['modLoader'],
    version: string | null,
    automaticLabel: string,
): string {
    if (version) return `${loaderLabel} ${version}`
    return loader === 'vanilla' ? loaderLabel : `${loaderLabel} (${automaticLabel})`
}

function normalizedConstraints(constraints: InstanceUpgradeFixedConstraint[]) {
    return constraints
        .map((constraint) => ({
            contentId: constraint.contentId,
            provider: constraint.provider,
            projectId: constraint.projectId,
            versionId: constraint.versionId,
        }))
        .sort((left, right) => left.contentId.localeCompare(right.contentId))
}

export function customConstraintsEqual(
    left: InstanceUpgradeFixedConstraint[],
    right: InstanceUpgradeFixedConstraint[],
): boolean {
    return (
        JSON.stringify(normalizedConstraints(left)) === JSON.stringify(normalizedConstraints(right))
    )
}

export function setFixedConstraint(
    constraints: InstanceUpgradeFixedConstraint[],
    constraint: InstanceUpgradeFixedConstraint | null,
    contentId: string,
): InstanceUpgradeFixedConstraint[] {
    const withoutContent = constraints.filter((current) => current.contentId !== contentId)
    return normalizedConstraints(constraint ? [...withoutContent, constraint] : withoutContent)
}

export function editableUpgradeRoots(plan: InstanceUpgradePlan) {
    return plan.items.filter(
        (item) =>
            !item.autoDependency &&
            (item.provider === 'modrinth' || item.provider === 'curseforge') &&
            item.projectId !== null &&
            (item.candidateReleaseIds.length > 0 ||
                plan.customConstraints.some(
                    (constraint) => constraint.contentId === item.contentId,
                )),
    )
}

export function compatibilitySummary(plan: InstanceUpgradePlan): CompatibilitySummary {
    const content = plan.selectedSolution
        ? summarizeSelections(plan.selectedSolution)
        : plan.items.reduce(
              (summary, item) => {
                  if (item.resolution.action === 'disable') summary.disabled += 1
                  else if (item.status === 'upgrade_available') summary.updates += 1
                  else if (
                      item.status === 'already_compatible' ||
                      item.resolution.action === 'keep'
                  ) {
                      summary.keptOrCompatible += 1
                  }
                  return summary
              },
              { updates: 0, keptOrCompatible: 0, disabled: 0 },
          )
    const dependencyChanges = (
        plan.selectedSolution?.dependencyChanges ?? plan.dependencyChanges
    ).filter((change) => change.kind !== 'keep').length

    return {
        ...content,
        dependencyChanges,
        needsAttention: plan.blockingIssues.length,
    }
}
