import { reactive, type Ref } from 'vue'

import { loadProject, type ProjectDetail, projectIcon, type ProjectVersion } from '../../../data'
import { formatPreviewMessage } from '../../../lib/i18n'
import { instanceUpgradeNotices } from '../../../lib/post-upgrade-notice'
import { currentPreview, previewDomain, type PreviewState } from '../../../lib/state'
import { contentFolderFor, type ContentItem } from '../../../lib/state/downloads'
import type { GameInstance } from '../../../lib/types'
import { getLoaderVersions } from '../settings/installation/metadata'
import { instanceContent } from '../state'
import {
    applyUpgradeSelection,
    runUpgradeTransaction,
    upgradeSourceRevision as fingerprint,
} from './execution'
import type { prepareInstanceCopy } from './instance-copy'
import type {
    InstanceUpgradeDisplayNames,
    InstanceUpgradeFixedConstraint,
    InstanceUpgradeIssue,
    InstanceUpgradePlan,
    InstanceUpgradePlanItem,
    InstanceUpgradeResolution,
    InstanceUpgradeResult,
    InstanceUpgradeSolution,
    InstanceUpgradeSolutionKind,
    InstanceUpgradeTargetEnvironment,
    SharedUpgradeMode,
} from './types'
import { compareSemanticVersions } from './version-compatibility'

export type * from './types'

export interface InstallJobSnapshot {
    job_id: string
    kind: 'upgrade_unmanaged_instance'
    status: 'running' | 'succeeded' | 'failed' | 'canceled'
    instance_id: string
    source_instance_id: string
    title: string
    phase: string
    progress: number
    error: string | null
    upgrade_result: InstanceUpgradeResult | null
    created: string
    finished: string | null
}
interface PlanRecord {
    plan: InstanceUpgradePlan
    files: ContentItem[]
    revision: string
    projects: Map<string, ProjectDetail | null>
}
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value))

function matches(version: ProjectVersion, target: InstanceUpgradeTargetEnvironment, type: string) {
    return (
        version.game_versions.includes(target.gameVersion) &&
        (type !== 'mod' || version.loaders.includes(target.modLoader))
    )
}

function createUpgradeDomain(preview: PreviewState) {
    const plans = new Map<string, PlanRecord>()
    const jobs = reactive(new Map<string, InstallJobSnapshot>())
    const stops = new Map<string, () => void>()
    const flows = new Map<string, unknown>()
    let counter = 0
    const id = () => `upgrade-${Date.now().toString(36)}-${++counter}`

    async function project(record: PlanRecord, projectId: string) {
        if (!record.projects.has(projectId))
            record.projects.set(projectId, await loadProject(projectId).catch(() => null))
        return record.projects.get(projectId) ?? null
    }
    function issue(
        item: InstanceUpgradePlanItem,
        code: InstanceUpgradeIssue['code'],
    ): InstanceUpgradeIssue {
        return {
            code,
            message: formatPreviewMessage({
                id: `instance.upgrade.warning.${code.replaceAll('_', '-')}`,
                defaultMessage: item.relativePath,
            }),
            contentId: item.contentId,
            provider: item.provider,
            projectId: item.projectId,
            conflictingProjectId: null,
            dependencyRequirements: [],
        }
    }

    async function solve(record: PlanRecord, kind: InstanceUpgradeSolutionKind) {
        const plan = record.plan
        const solution: InstanceUpgradeSolution = {
            kind,
            selections: [],
            dependencyChanges: [],
            warnings: [],
        }
        const blockers: InstanceUpgradeIssue[] = []
        const selected = new Map<
            string,
            { version: ProjectVersion; item: InstanceUpgradePlanItem }
        >()
        for (const item of plan.items) {
            const detail = item.projectId ? await project(record, item.projectId) : null
            const candidates = (detail?.versions ?? []).filter(
                (version) =>
                    matches(version, plan.targetEnvironment, item.projectType) &&
                    (version.version_type === 'release' || item.resolution.allowPrerelease),
            )
            const installed = candidates.find((version) => version.id === item.currentReleaseId)
            const constraint =
                kind === 'custom'
                    ? plan.customConstraints.find((entry) => entry.contentId === item.contentId)
                    : null
            const target = constraint
                ? candidates.find((version) => version.id === constraint.versionId)
                : kind === 'minimal_change' && installed
                  ? installed
                  : candidates[0]
            const action =
                item.resolution.action === 'upgrade' && target?.id === item.currentReleaseId
                    ? 'keep'
                    : item.resolution.action
            if (action === 'upgrade' && !target)
                blockers.push(
                    issue(
                        item,
                        constraint
                            ? 'dependency_conflict'
                            : item.status === 'prerelease_only'
                              ? 'prerelease_only'
                              : item.status === 'unidentified'
                                ? 'unidentified'
                                : 'no_compatible_release',
                    ),
                )
            if (action === 'keep' && !installed && item.currentEnabled)
                solution.warnings.push(
                    issue(item, item.provider === 'local' ? 'unidentified' : 'keep_incompatible'),
                )
            solution.selections.push({
                contentId: item.contentId,
                provider: item.provider,
                projectId: item.projectId,
                currentReleaseId: item.currentReleaseId,
                targetReleaseId:
                    action === 'upgrade' ? (target?.id ?? null) : item.currentReleaseId,
                action,
                enabled: action !== 'disable' && item.currentEnabled,
            })
            const effective =
                action === 'upgrade'
                    ? target
                    : detail?.versions.find((entry) => entry.id === item.currentReleaseId)
            if (item.projectId && action !== 'disable' && item.currentEnabled && effective)
                selected.set(item.projectId, { version: effective, item })
        }
        const queue = [...selected.entries()]
        const visited = new Set<string>()
        while (queue.length) {
            const [projectId, current] = queue.shift()!
            const key = `${projectId}:${current.version.id}`
            if (visited.has(key)) continue
            visited.add(key)
            for (const dependency of current.version.dependencies ?? []) {
                const dependencyId = dependency.project_id
                if (!dependencyId) {
                    if (dependency.dependency_type === 'required')
                        blockers.push(issue(current.item, 'missing_required_dependency'))
                    continue
                }
                const present = selected.get(dependencyId)
                if (
                    dependency.dependency_type === 'incompatible' &&
                    present &&
                    (!dependency.version_id || dependency.version_id === present.version.id)
                ) {
                    blockers.push({
                        ...issue(current.item, 'incompatible_dependency'),
                        conflictingProjectId: dependencyId,
                    })
                    continue
                }
                if (dependency.dependency_type !== 'required') continue
                if (present) {
                    if (dependency.version_id && dependency.version_id !== present.version.id)
                        blockers.push({
                            ...issue(current.item, 'dependency_conflict'),
                            conflictingProjectId: dependencyId,
                        })
                    continue
                }
                const existing = plan.items.find((entry) => entry.projectId === dependencyId)
                if (existing) {
                    blockers.push({
                        ...issue(current.item, 'missing_required_dependency'),
                        conflictingProjectId: dependencyId,
                    })
                    continue
                }
                const dependencyProject = await project(record, dependencyId)
                const version = dependencyProject?.versions.find(
                    (entry) =>
                        matches(entry, plan.targetEnvironment, 'mod') &&
                        (!dependency.version_id || entry.id === dependency.version_id) &&
                        (entry.version_type === 'release' ||
                            current.item.resolution.allowPrerelease),
                )
                if (!version) {
                    blockers.push({
                        ...issue(current.item, 'missing_required_dependency'),
                        conflictingProjectId: dependencyId,
                    })
                    continue
                }
                selected.set(dependencyId, { version, item: current.item })
                queue.push([dependencyId, { version, item: current.item }])
                solution.dependencyChanges.push({
                    existingContentId: null,
                    provider: 'modrinth',
                    projectId: dependencyId,
                    currentReleaseId: null,
                    targetReleaseId: version.id,
                    kind: 'add',
                    enabled: true,
                })
            }
        }
        return { solution, blockers }
    }

    async function recalculate(record: PlanRecord, kind: InstanceUpgradeSolutionKind = 'newest') {
        const newest = await solve(record, 'newest')
        const minimal = await solve(record, 'minimal_change')
        const selected =
            kind === 'custom'
                ? await solve(record, 'custom')
                : kind === 'minimal_change'
                  ? minimal
                  : newest
        Object.assign(record.plan, {
            newestSolution: newest.blockers.length ? null : newest.solution,
            minimalChangeSolution: minimal.blockers.length ? null : minimal.solution,
            selectedSolution: selected.blockers.length ? null : selected.solution,
            blockingIssues: selected.blockers,
            warnings: selected.solution.warnings,
            dependencyChanges: selected.solution.dependencyChanges,
        })
        return clone(record.plan)
    }
    function get(planId: string) {
        const record = plans.get(planId)
        if (!record)
            throw new Error('Upgrade plan is no longer available. Choose the target again.')
        return record
    }
    async function plan(instanceId: string, target: InstanceUpgradeTargetEnvironment) {
        const instance = preview.instances.get(instanceId)
        if (!instance) throw new Error('Instance not found')
        if (
            instance.install_stage !== 'installed' ||
            preview.processes.processes.value.some((entry) => entry.instance.id === instanceId)
        )
            throw new Error('Stop the instance before upgrading.')
        await preview.downloads.ensureContents()
        const files = clone(
            (preview.downloads.contents.value[instanceId] ?? []).filter((item) => !item.removed),
        )
        const record: PlanRecord = {
            files,
            revision: fingerprint(instance, preview.downloads.contents.value[instanceId] ?? []),
            projects: new Map(),
            plan: {
                id: id(),
                instanceId,
                sourceRevision: 0,
                sourceFiles: [],
                sourceEnvironment: {
                    gameVersion: instance.game_version,
                    modLoader: instance.loader,
                    modLoaderVersion: instance.loader_version ?? null,
                    shaderRuntime: target.shaderRuntime,
                },
                targetEnvironment: clone(target),
                items: [],
                dependencyChanges: [],
                warnings: [],
                blockingIssues: [],
                newestSolution: null,
                minimalChangeSolution: null,
                selectedSolution: null,
                customConstraints: [],
            },
        }
        for (const file of files) {
            const detail = file.project_id ? await project(record, file.project_id) : null
            const candidates = (detail?.versions ?? []).filter((entry) =>
                matches(entry, target, file.project_type),
            )
            const stable = candidates.filter((entry) => entry.version_type === 'release')
            const status = !detail
                ? 'unidentified'
                : !candidates.length
                  ? 'no_compatible_release'
                  : !stable.length
                    ? 'prerelease_only'
                    : stable[0].id === file.version_id
                      ? 'already_compatible'
                      : 'upgrade_available'
            record.plan.items.push({
                contentId: file.id,
                relativePath: `${file.folder ?? contentFolderFor(file.project_type)}/${file.file_name}`,
                projectType: (file.project_type === 'shader'
                    ? 'shaderpack'
                    : file.project_type) as InstanceUpgradePlanItem['projectType'],
                provider: file.project_id ? 'modrinth' : 'local',
                projectId: file.project_id,
                currentReleaseId: file.version_id,
                currentEnabled: file.enabled,
                autoDependency: false,
                status,
                resolution: {
                    contentId: file.id,
                    action: file.enabled ? 'upgrade' : 'disable',
                    allowPrerelease: false,
                    confirmedPrereleaseDependencies: [],
                },
                candidateReleaseIds: candidates.map((entry) => entry.id),
            })
        }
        plans.set(record.plan.id, record)
        return recalculate(record)
    }
    async function resolve(planId: string, resolutions: InstanceUpgradeResolution[]) {
        const record = get(planId)
        for (const resolution of resolutions) {
            const item = record.plan.items.find((entry) => entry.contentId === resolution.contentId)
            if (!item) throw new Error('Content item no longer exists')
            item.resolution = clone(resolution)
        }
        return recalculate(record, record.plan.selectedSolution?.kind)
    }
    function duplicate(
        source: GameInstance,
        name: string,
        files: ContentItem[],
        prepared: Awaited<ReturnType<typeof prepareInstanceCopy>>,
    ) {
        const created = preview.instances.create({
            name,
            gameVersion: source.game_version,
            loader: source.loader,
            loaderVersion: source.loader_version ?? null,
            iconPath: source.icon_path,
        })
        const identity = { id: created.id, path: created.path, name: created.name }
        Object.assign(created, clone(source), identity, {
            symlink_target: null,
            link: null,
            install_stage: 'installed',
        })
        created.loader_components = created.loader_components.map((entry) => ({
            ...entry,
            instanceId: created.id,
        }))
        preview.downloads.contents.value = {
            ...preview.downloads.contents.value,
            [created.id]: clone(files),
        }
        prepared.apply(created)
        return created
    }
    function changedFile(
        previous: ContentItem | undefined,
        detail: ProjectDetail,
        version: ProjectVersion,
    ): ContentItem {
        const file = version.files.find((entry) => entry.primary) ?? version.files[0]
        return {
            ...previous,
            id: previous?.id ?? `${detail.id}-${version.id}`,
            project_id: detail.id,
            version_id: version.id,
            version_number: version.version_number,
            name: detail.title,
            icon: projectIcon(detail.id),
            file_name: file?.filename ?? version.name,
            size: file?.size ?? 0,
            enabled: true,
            project_type: detail.project_type,
            installed_at: new Date().toISOString(),
            folder: previous?.folder ?? contentFolderFor(detail.project_type),
        }
    }
    async function execute(
        planId: string,
        backup: boolean,
        mode: SharedUpgradeMode,
        names: InstanceUpgradeDisplayNames,
    ) {
        const record = get(planId)
        const plan = clone(record.plan)
        const source = preview.instances.get(plan.instanceId)
        if (!source || !plan.selectedSolution || plan.blockingIssues.length)
            throw new Error('Resolve compatibility issues first.')
        const active = [...jobs.values()].find(
            (job) => job.source_instance_id === source.id && job.status === 'running',
        )
        if (active) return active
        if (
            source.install_stage !== 'installed' ||
            preview.processes.processes.value.some((entry) => entry.instance.id === source.id)
        )
            throw new Error('Stop the instance before upgrading.')
        const manifest =
            source.loader === 'vanilla'
                ? null
                : await getLoaderVersions(
                      source.loader === 'neoforge' ? 'neo' : source.loader,
                      plan.targetEnvironment.gameVersion,
                  )
        const loaders = (manifest?.gameVersions[0]?.loaders ?? []).filter((entry) => {
            if (source.loader !== 'fabric' || !source.loader_version) return true
            const comparison = compareSemanticVersions(entry.id, source.loader_version)
            return comparison === null || comparison >= 0
        })
        const loaderVersion =
            plan.targetEnvironment.modLoaderVersion ??
            loaders.find((entry) => entry.stable)?.id ??
            loaders[0]?.id
        if (
            plan.targetEnvironment.modLoaderVersion &&
            !loaders.some((entry) => entry.id === plan.targetEnvironment.modLoaderVersion)
        )
            throw new Error(
                'The selected loader is unavailable for the target version. Choose the target again.',
            )
        if (source.loader !== 'vanilla' && !loaderVersion)
            throw new Error('No compatible loader version is available for this Minecraft version.')
        const prepared = await (
            await import('./instance-copy')
        ).prepareInstanceCopy(preview, source)
        if (
            fingerprint(source, preview.downloads.contents.value[source.id] ?? []) !==
            record.revision
        )
            throw new Error('Instance content changed. Check compatibility again before upgrading.')
        const solution = plan.selectedSolution
        const output = record.files.map((file) => {
            const selection = solution.selections.find((entry) => entry.contentId === file.id)
            return applyUpgradeSelection(file, selection, () => {
                const detail = record.projects.get(selection!.projectId!)!
                return changedFile(
                    file,
                    detail,
                    detail.versions.find((entry) => entry.id === selection!.targetReleaseId)!,
                )
            })
        })
        for (const change of solution.dependencyChanges) {
            const detail = record.projects.get(change.projectId)!
            output.push(
                changedFile(
                    undefined,
                    detail,
                    detail.versions.find((entry) => entry.id === change.targetReleaseId)!,
                ),
            )
        }
        const job = reactive<InstallJobSnapshot>({
            job_id: id(),
            kind: 'upgrade_unmanaged_instance',
            status: 'running',
            instance_id: source.id,
            source_instance_id: source.id,
            title: source.name,
            phase: backup ? 'backup' : 'prepare',
            progress: 0,
            error: null,
            upgrade_result: null,
            created: new Date().toISOString(),
            finished: null,
        })
        jobs.set(job.job_id, job)
        source.install_stage = 'minecraft_installing'
        const stop = runUpgradeTransaction({
            clock: preview.clock,
            sourceIsCurrent: () =>
                preview.instances.get(source.id) === source &&
                fingerprint(source, preview.downloads.contents.value[source.id] ?? []) ===
                    record.revision &&
                (prepared?.isCurrent() ?? true),
            progress: (fraction, elapsed) => {
                job.progress = fraction
                job.phase =
                    elapsed < 900
                        ? backup
                            ? 'backup'
                            : 'prepare'
                        : elapsed < 2200
                          ? 'minecraft'
                          : elapsed < 3400
                            ? 'content'
                            : 'apply'
            },
            commit: () => {
                const original = { ...source, install_stage: 'installed' as const }
                const backupInstance =
                    backup && mode === 'direct'
                        ? duplicate(
                              original,
                              names.backup ?? `${source.name} (Pre-upgrade backup)`,
                              record.files,
                              prepared!,
                          )
                        : null
                const target =
                    mode === 'copy_and_upgrade'
                        ? duplicate(
                              original,
                              names.copy ?? `${source.name} (Upgraded copy)`,
                              record.files,
                              prepared!,
                          )
                        : source
                target.game_version = plan.targetEnvironment.gameVersion
                target.loader_version = loaderVersion ?? undefined
                target.loader_components = loaderVersion
                    ? [
                          {
                              instanceId: target.id,
                              kind: target.loader,
                              version: loaderVersion,
                              role: 'primary',
                          },
                      ]
                    : []
                prepared.syncContent(target, record.files, output)
                target.modified = new Date().toISOString()
                target.install_stage = 'installed'
                if (names.shouldAutoRename && names.upgradedTarget)
                    target.name = names.upgradedTarget
                preview.downloads.contents.value = {
                    ...preview.downloads.contents.value,
                    [target.id]: output,
                }
                source.install_stage = 'installed'
                const result: InstanceUpgradeResult = {
                    planId,
                    sourceInstanceId: source.id,
                    targetInstanceId: target.id,
                    backupInstanceId: backupInstance?.id ?? null,
                    sourceEnvironment: plan.sourceEnvironment,
                    targetEnvironment: {
                        ...plan.targetEnvironment,
                        modLoaderVersion: loaderVersion ?? null,
                    },
                    solution,
                    compatibilityWarnings: solution.warnings,
                    compatibilityWarningDetails: solution.warnings.map((warning) => ({
                        code: warning.code,
                        relativePath:
                            plan.items.find((item) => item.contentId === warning.contentId)
                                ?.relativePath ?? null,
                        contentId: warning.contentId,
                        provider: warning.provider,
                        projectId: warning.projectId,
                        conflictingProjectId: warning.conflictingProjectId,
                    })),
                    externalChanges: [],
                    skippedDueToExternalConflict: [],
                }
                instanceUpgradeNotices(preview).set(target.id, {
                    targetGameVersion: target.game_version,
                    warnings: solution.warnings.map((warning) => {
                        const file = output.find((item) => item.id === warning.contentId)
                        return {
                            fileName: file?.file_name ?? warning.contentId ?? '',
                            projectId: file?.project_id,
                            versionId: file?.version_id,
                        }
                    }),
                })
                job.instance_id = target.id
                job.upgrade_result = result
                job.status = 'succeeded'
                job.finished = new Date().toISOString()
                void instanceContent(preview).reload(target.id)
            },
            failed: (error) => {
                source.install_stage = 'installed'
                job.error = error instanceof Error ? error.message : String(error)
                job.status = 'failed'
                job.finished = new Date().toISOString()
            },
            finished: () => stops.delete(job.job_id),
        })
        stops.set(job.job_id, stop)
        return job
    }
    function cancel(jobId: string) {
        const job = jobs.get(jobId)
        if (!job || job.status !== 'running') return
        stops.get(jobId)?.()
        stops.delete(jobId)
        job.status = 'canceled'
        job.finished = new Date().toISOString()
        const source = preview.instances.get(job.source_instance_id)
        if (source) source.install_stage = 'installed'
    }
    return {
        value: { plans, jobs, flows, plan, get, resolve, recalculate, execute, cancel },
        dispose: () => {
            for (const stop of stops.values()) stop()
        },
    }
}

export const upgradeDomain = (preview = currentPreview()) =>
    previewDomain(preview, 'instance-upgrade', createUpgradeDomain)
export const plan_instance_upgrade = (id: string, target: InstanceUpgradeTargetEnvironment) =>
    upgradeDomain().plan(id, target)
export const get_instance_upgrade_plan = async (id: string) => {
    const record = upgradeDomain().get(id)
    const preview = currentPreview()
    const instance = preview.instances.get(record.plan.instanceId)
    if (
        !instance ||
        fingerprint(instance, preview.downloads.contents.value[instance.id] ?? []) !==
            record.revision
    )
        throw new Error('Instance content changed. Check compatibility again.')
    return clone(record.plan)
}
export const discard_instance_upgrade_plan = async (id: string) => {
    upgradeDomain().plans.delete(id)
}
export const update_instance_upgrade_resolution = (
    id: string,
    resolution: InstanceUpgradeResolution,
) => upgradeDomain().resolve(id, [resolution])
export async function update_instance_upgrade_resolutions(
    id: string,
    resolutions: InstanceUpgradeResolution[],
) {
    return {
        plan: await upgradeDomain().resolve(id, resolutions),
        requestedCount: resolutions.length,
        applied: resolutions.map((entry) => ({
            contentId: entry.contentId,
            code: null,
            message: null,
        })),
        skipped: [],
        failed: [],
    }
}
export async function reset_instance_upgrade_resolution(id: string, contentId: string) {
    return update_instance_upgrade_resolution(id, {
        contentId,
        action: 'upgrade',
        allowPrerelease: false,
        confirmedPrereleaseDependencies: [],
    })
}
export async function select_instance_upgrade_solution(
    id: string,
    kind: InstanceUpgradeSolutionKind,
) {
    return upgradeDomain().recalculate(upgradeDomain().get(id), kind)
}
export async function resolve_custom_instance_upgrade_solution(
    id: string,
    constraints: InstanceUpgradeFixedConstraint[],
) {
    const record = upgradeDomain().get(id)
    record.plan.customConstraints = clone(constraints)
    return upgradeDomain().recalculate(record, 'custom')
}
export async function submitInstanceUpgrade(
    input: {
        planId: string
        createFullBackup: boolean
        sharedUpgradeMode: SharedUpgradeMode
        displayNames: InstanceUpgradeDisplayNames
        instanceId: string
    },
    lock: Ref<boolean>,
) {
    if (lock.value) return null
    lock.value = true
    try {
        return {
            job: await upgradeDomain().execute(
                input.planId,
                input.createFullBackup,
                input.sharedUpgradeMode,
                input.displayNames,
            ),
        }
    } finally {
        lock.value = false
    }
}
export async function get_many(ids: string[]) {
    return ids
        .map((id) => currentPreview().instances.get(id))
        .filter((item): item is GameInstance => !!item)
}
