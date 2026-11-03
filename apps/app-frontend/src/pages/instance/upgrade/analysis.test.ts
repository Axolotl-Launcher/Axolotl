import { expect, it } from 'vitest'

import type {
    InstanceUpgradePlan,
    InstanceUpgradeTargetEnvironment,
} from '@/helpers/instance-upgrade'

import {
    actionableWarningContentIds,
    AUTOMATIC_FABRIC_LOADER_VERSION,
    automaticFabricLoaderTargetAvailable,
    availablePredefinedStrategies,
    captureInitialUpgradeBlockingIssues,
    commitUpgradePlanSelection,
    compatibilitySummary,
    confirmDependencyReleaseSlots,
    confirmSelectionReleaseSlots,
    confirmSolutionGroups,
    confirmTargetLoaderLabel,
    confirmUpgradeOptions,
    contentIdentityKeys,
    customConstraintsEqual,
    editableUpgradeRoots,
    fabricLoaderVersionForTarget,
    fabricUpgradeLoaderVersions,
    groupUpgradeIssues,
    inferShaderRuntime,
    isSharedUpgradeInstance,
    newerStableGameVersions,
    preserveFabricLoaderSelection,
    resolveConfirmDependencyReleases,
    resolveUpgradePlanSelection,
    sanitizeMinecraftDisplayTitle,
    setFixedConstraint,
    shouldReuseUpgradePlan,
    solutionSummary,
    upgradeContentDisplayMetadata,
    upgradeResolutionPresentation,
    upgradeTargetsEqual,
} from './analysis.ts'

function issue(code: string, contentId: string | null, projectId: string | null, message = code) {
    return {
        code,
        message,
        contentId,
        provider: projectId ? 'modrinth' : null,
        projectId,
        conflictingProjectId: null,
        dependencyRequirements: [],
    }
}

function planItem(contentId: string, projectId: string | null = contentId) {
    return {
        contentId,
        relativePath: `mods/${contentId}.jar`,
        projectType: 'mod',
        provider: projectId ? 'modrinth' : null,
        projectId,
        currentReleaseId: 'old',
        currentEnabled: true,
        autoDependency: false,
        status: 'already_compatible',
        resolution: {
            contentId,
            action: 'upgrade',
            allowPrerelease: false,
            confirmedPrereleaseDependencies: [],
        },
        candidateReleaseIds: [],
    }
}

it('newer stable versions follow metadata order without numeric parsing', () => {
    const result = newerStableGameVersions(
        [
            { version: '26.1.2', version_type: 'release', date: '', major: false },
            { version: '26.1-beta', version_type: 'snapshot', date: '', major: false },
            { version: '26.1', version_type: 'release', date: '', major: true },
            { version: '1.21.8', version_type: 'release', date: '', major: false },
            { version: '1.21.7', version_type: 'release', date: '', major: false },
        ],
        '1.21.8',
    )

    expect(result).toEqual({ currentFound: true, versions: ['26.1.2', '26.1'] })
})

it('upgrade target equality uses semantic environment fields', () => {
    const target = {
        gameVersion: '26.2',
        modLoader: 'fabric',
        modLoaderVersion: '0.18.4',
        shaderRuntime: 'iris',
    } as const
    expect(upgradeTargetsEqual(target, { ...target })).toBe(true)
    expect(upgradeTargetsEqual(target, { ...target, gameVersion: '26.1' })).toBe(false)
    expect(upgradeTargetsEqual(target, { ...target, modLoaderVersion: '1' })).toBe(false)
})

it('matching instance and semantic target reuse existing plan without replacing state', () => {
    const target = {
        gameVersion: '26.2',
        modLoader: 'fabric',
        modLoaderVersion: null,
        shaderRuntime: 'iris',
    } as const
    const plan = {
        id: 'plan-one',
        instanceId: 'instance-one',
        targetEnvironment: target,
        items: [{ resolution: { action: 'keep' } }],
        customConstraints: [{ contentId: 'root', versionId: 'fixed' }],
    } as InstanceUpgradePlan

    expect(shouldReuseUpgradePlan('instance-one', plan, { ...target })).toBe(true)
    expect(plan.id).toBe('plan-one')
    expect(plan.items[0].resolution.action).toBe('keep')
    expect(plan.customConstraints[0].versionId).toBe('fixed')
    expect(shouldReuseUpgradePlan('instance-one', plan, { ...target, gameVersion: '26.1' })).toBe(
        false,
    )
    expect(shouldReuseUpgradePlan('instance-two', plan, target)).toBe(false)
    expect(shouldReuseUpgradePlan('instance-one', null, target)).toBe(false)
})

it('plan selection skips planner for matching target and calls it once for a confirmed change', async () => {
    const target: InstanceUpgradeTargetEnvironment = {
        gameVersion: '26.2',
        modLoader: 'fabric',
        modLoaderVersion: '0.18.4',
        shaderRuntime: 'iris',
    }
    const existing = {
        id: 'plan-one',
        instanceId: 'instance-one',
        targetEnvironment: target,
    } as InstanceUpgradePlan
    let calls = 0
    const planner = async (_instanceId: string, nextTarget: InstanceUpgradeTargetEnvironment) => {
        calls += 1
        return {
            ...existing,
            id: 'plan-two',
            targetEnvironment: nextTarget,
        } as InstanceUpgradePlan
    }

    const reused = await resolveUpgradePlanSelection(
        'instance-one',
        existing,
        { ...target },
        planner,
    )
    expect(calls).toBe(0)
    expect(reused.plan).toBe(existing)
    expect(reused.reused).toBe(true)

    const replanned = await resolveUpgradePlanSelection(
        'instance-one',
        existing,
        { ...target, modLoaderVersion: '0.18.5' },
        planner,
    )
    expect(calls).toBe(1)
    expect(replanned.plan.id).toBe('plan-two')
    expect(replanned.reused).toBe(false)
})

it('failed replan preserves existing plan and authoritative target', async () => {
    const oldTarget = {
        gameVersion: '26.1',
        modLoader: 'fabric',
        modLoaderVersion: '0.18.4',
        shaderRuntime: 'iris',
    } as const
    const attemptedTarget = { ...oldTarget, gameVersion: '26.2', modLoaderVersion: '0.18.5' }
    const oldPlan = {
        id: 'old-plan',
        instanceId: 'instance-one',
        targetEnvironment: oldTarget,
    } as InstanceUpgradePlan
    let authoritativePlan = oldPlan
    let authoritativeTarget = oldTarget

    await expect(
        commitUpgradePlanSelection(
            'instance-one',
            oldPlan,
            attemptedTarget,
            async () => {
                throw new Error('planning failed')
            },
            (plan) => (authoritativePlan = plan),
            (target) => (authoritativeTarget = target as typeof oldTarget),
        ),
    ).rejects.toThrow(/planning failed/)
    expect(authoritativePlan).toBe(oldPlan)
    expect(authoritativeTarget).toBe(oldTarget)
})

it('Fabric loader choices exclude downgrades using numeric semantic comparison', () => {
    expect(fabricUpgradeLoaderVersions('0.18.4', ['0.18.6', '0.18.5', '0.18.4', '0.18.3'])).toEqual(
        ['0.18.6', '0.18.5', '0.18.4'],
    )
    expect(fabricUpgradeLoaderVersions('0.18.9', ['0.18.10', '0.18.9'])).toEqual([
        '0.18.10',
        '0.18.9',
    ])
    expect(fabricUpgradeLoaderVersions('custom', ['0.19.0'])).toEqual([])
})

it('Fabric loader pending selection preserves valid exact values and maps target values', () => {
    expect(preserveFabricLoaderSelection('0.18.5', ['0.18.5'])).toBe('0.18.5')
    expect(preserveFabricLoaderSelection('0.18.5', ['0.18.6'])).toBe(
        AUTOMATIC_FABRIC_LOADER_VERSION,
    )
    expect(preserveFabricLoaderSelection(AUTOMATIC_FABRIC_LOADER_VERSION, [])).toBe(
        AUTOMATIC_FABRIC_LOADER_VERSION,
    )
    expect(fabricLoaderVersionForTarget(AUTOMATIC_FABRIC_LOADER_VERSION)).toBe(null)
    expect(fabricLoaderVersionForTarget('0.18.5')).toBe('0.18.5')
    expect(automaticFabricLoaderTargetAvailable(true, true, [])).toBe(false)
    expect(automaticFabricLoaderTargetAvailable(true, true, ['0.18.5'])).toBe(true)
    expect(automaticFabricLoaderTargetAvailable(false, true, [])).toBe(true)
    expect(automaticFabricLoaderTargetAvailable(true, false, [])).toBe(true)
})

it('shared upgrade detection uses established link or external target metadata', () => {
    expect(isSharedUpgradeInstance({ link: { type: 'shared_instance' } } as never)).toBe(true)
    expect(isSharedUpgradeInstance({ symlink_target: 'D:/Minecraft' } as never)).toBe(true)
    expect(
        isSharedUpgradeInstance({ linked_dot_minecraft: 'D:/Minecraft/.minecraft' } as never),
    ).toBe(true)
    expect(isSharedUpgradeInstance({ link: null, symlink_target: null } as never)).toBe(false)
})

it('unknown current version exposes stable releases conservatively', () => {
    const result = newerStableGameVersions(
        [
            { version: '26.1', version_type: 'release', date: '', major: true },
            { version: '26.1-beta', version_type: 'snapshot', date: '', major: false },
        ],
        'custom',
    )

    expect(result).toEqual({ currentFound: false, versions: ['26.1'] })
})

it('compatibility summary uses selected solution and changed dependencies', () => {
    const plan = {
        blockingIssues: [{ code: 'dependency_conflict' }, { code: 'prerelease_only' }],
        selectedSolution: {
            selections: [
                { action: 'upgrade', currentReleaseId: 'old', targetReleaseId: 'new' },
                { action: 'keep', currentReleaseId: 'same', targetReleaseId: 'same' },
                { action: 'disable', currentReleaseId: 'off', targetReleaseId: null },
            ],
            dependencyChanges: [{ kind: 'add' }, { kind: 'keep' }, { kind: 'remove' }],
        },
    } as InstanceUpgradePlan

    expect(compatibilitySummary(plan)).toEqual({
        updates: 1,
        keptOrCompatible: 1,
        disabled: 1,
        dependencyChanges: 2,
        needsAttention: 2,
    })
})

it('shader runtime inference uses exact loader component and provider identity', () => {
    const instance = {
        loader: 'fabric',
        loader_components: [],
    } as never
    const snapshot = {
        items: [
            {
                projectType: 'mod',
                provider: 'modrinth',
                providerProjectId: 'YL57xq9U',
                content: null,
            },
        ],
    } as never

    expect(inferShaderRuntime(instance, snapshot)).toBe('iris')
    expect(
        inferShaderRuntime({ ...instance, loader_components: [{ kind: 'optifine' }] }, undefined),
    ).toBe('opti_fine')
    expect(inferShaderRuntime(instance, undefined)).toBe('unknown')
})

it('solution summary separates root and dependency changes', () => {
    const summary = solutionSummary({
        kind: 'newest',
        selections: [
            {
                contentId: 'a',
                provider: 'modrinth',
                projectId: 'a',
                currentReleaseId: '1',
                targetReleaseId: '2',
                action: 'upgrade',
                enabled: true,
            },
            {
                contentId: 'b',
                provider: 'modrinth',
                projectId: 'b',
                currentReleaseId: '1',
                targetReleaseId: '1',
                action: 'keep',
                enabled: true,
            },
            {
                contentId: 'c',
                provider: 'modrinth',
                projectId: 'c',
                currentReleaseId: '1',
                targetReleaseId: null,
                action: 'disable',
                enabled: false,
            },
        ],
        dependencyChanges: [
            {
                existingContentId: null,
                provider: 'modrinth',
                projectId: 'd',
                currentReleaseId: null,
                targetReleaseId: '1',
                kind: 'add',
                enabled: true,
            },
            {
                existingContentId: 'e',
                provider: 'modrinth',
                projectId: 'e',
                currentReleaseId: '1',
                targetReleaseId: '2',
                kind: 'upgrade',
                enabled: true,
            },
            {
                existingContentId: 'f',
                provider: 'modrinth',
                projectId: 'f',
                currentReleaseId: '1',
                targetReleaseId: null,
                kind: 'remove',
                enabled: false,
            },
        ],
        warnings: [],
    })
    expect(summary).toEqual({
        upgraded: 1,
        kept: 1,
        disabled: 1,
        dependencyAdditions: 1,
        dependencyUpdates: 1,
        dependencyRemovals: 1,
    })
})

it('confirm detail groups follow authoritative selection actions', () => {
    const solution = {
        kind: 'custom',
        selections: [
            { contentId: 'update', action: 'upgrade', currentReleaseId: '1', targetReleaseId: '2' },
            { contentId: 'same', action: 'upgrade', currentReleaseId: '1', targetReleaseId: '1' },
            { contentId: 'keep', action: 'keep', currentReleaseId: '1', targetReleaseId: '1' },
            {
                contentId: 'disable',
                action: 'disable',
                currentReleaseId: '1',
                targetReleaseId: null,
            },
        ],
        dependencyChanges: [
            { kind: 'add' },
            { kind: 'upgrade' },
            { kind: 'keep' },
            { kind: 'remove' },
        ],
        warnings: [],
    } as never
    const groups = confirmSolutionGroups(solution)

    expect(groups.updated.map((item) => item.contentId)).toEqual(['update'])
    expect(groups.kept.map((item) => item.contentId)).toEqual(['same', 'keep'])
    expect(groups.disabled.map((item) => item.contentId)).toEqual(['disable'])
    expect(groups.dependencyChanges.map((item) => item.kind)).toEqual(['add', 'upgrade', 'remove'])
})

it('confirm upgrade options require shared mode and suppress redundant copy backup', () => {
    expect(confirmUpgradeOptions(false, null, true)).toEqual({
        effectiveMode: 'direct',
        createFullBackup: true,
        canStart: true,
    })
    expect(confirmUpgradeOptions(true, null, true)).toEqual({
        effectiveMode: null,
        createFullBackup: true,
        canStart: false,
    })
    expect(confirmUpgradeOptions(true, 'direct', false)).toEqual({
        effectiveMode: 'direct',
        createFullBackup: false,
        canStart: true,
    })
    expect(confirmUpgradeOptions(true, 'copy_and_upgrade', true)).toEqual({
        effectiveMode: 'copy_and_upgrade',
        createFullBackup: false,
        canStart: true,
    })
})

it('confirm release slots keep current and target changelogs independent', () => {
    expect(
        confirmSelectionReleaseSlots({
            action: 'upgrade',
            currentReleaseId: 'current',
            targetReleaseId: 'target',
        } as never),
    ).toEqual({ currentReleaseId: 'current', targetReleaseId: 'target' })
    expect(
        confirmSelectionReleaseSlots({
            action: 'keep',
            currentReleaseId: 'current',
            targetReleaseId: 'current',
        } as never),
    ).toEqual({ currentReleaseId: 'current', targetReleaseId: null })
    expect(
        confirmDependencyReleaseSlots({
            currentReleaseId: 'current',
            targetReleaseId: 'target',
        } as never),
    ).toEqual({ currentReleaseId: 'current', targetReleaseId: 'target' })
})

it('dependency detail resolves deterministic current and target release slots', () => {
    const cases = [
        {
            change: { kind: 'upgrade', currentReleaseId: 'old', targetReleaseId: 'new' },
            expected: {
                currentReleaseId: 'old',
                targetReleaseId: 'new',
                current: 'old-label',
                target: 'new-label',
            },
        },
        {
            change: { kind: 'add', currentReleaseId: null, targetReleaseId: 'new' },
            expected: {
                currentReleaseId: null,
                targetReleaseId: 'new',
                current: null,
                target: 'new-label',
            },
        },
        {
            change: { kind: 'remove', currentReleaseId: 'old', targetReleaseId: null },
            expected: {
                currentReleaseId: 'old',
                targetReleaseId: null,
                current: 'old-label',
                target: null,
            },
        },
    ]
    for (const { change, expected } of cases) {
        expect(
            resolveConfirmDependencyReleases(change as never, (releaseId) =>
                releaseId ? `${releaseId}-label` : null,
            ),
        ).toEqual(expected)
    }
})

it('confirm target loader label shows explicit version or honest automatic policy', () => {
    expect(confirmTargetLoaderLabel('Fabric', 'fabric', '0.18.4', 'automatic')).toBe(
        'Fabric 0.18.4',
    )
    expect(confirmTargetLoaderLabel('Fabric', 'fabric', null, 'automatic')).toBe(
        'Fabric (automatic)',
    )
    expect(confirmTargetLoaderLabel('Vanilla', 'vanilla', null, 'automatic')).toBe('Vanilla')
})

it('fixed constraints replace and remove by physical content without duplicates', () => {
    const first = {
        contentId: 'a',
        provider: 'modrinth' as const,
        projectId: 'project',
        versionId: 'one',
    }
    const replaced = setFixedConstraint([first], { ...first, versionId: 'two' }, 'a')
    expect(replaced).toEqual([{ ...first, versionId: 'two' }])
    expect(setFixedConstraint(replaced, null, 'a')).toEqual([])
    expect(customConstraintsEqual(replaced, [{ ...first, versionId: 'two' }])).toBe(true)
})

it('editable roots exclude automatic dependencies', () => {
    const root = {
        contentId: 'root',
        autoDependency: false,
        provider: 'modrinth',
        projectId: 'root',
        candidateReleaseIds: ['one'],
    }
    const dependency = { ...root, contentId: 'dependency', autoDependency: true }
    const plan = { items: [root, dependency], customConstraints: [] } as InstanceUpgradePlan
    expect(editableUpgradeRoots(plan).map((item) => item.contentId)).toEqual(['root'])
})

it('unavailable minimal solution is not selectable', () => {
    const newestSolution = { kind: 'newest', selections: [], dependencyChanges: [], warnings: [] }
    expect(
        availablePredefinedStrategies({
            newestSolution,
            minimalChangeSolution: null,
        } as InstanceUpgradePlan),
    ).toEqual(['newest'])
})

it('issue grouping gives blocking precedence and includes every content once', () => {
    const plan = {
        items: [planItem('blocked'), planItem('warned'), planItem('clear')],
        blockingIssues: [issue('dependency_conflict', 'blocked', 'blocked')],
        warnings: [
            issue('keep_incompatible', 'blocked', 'blocked'),
            issue('keep_incompatible', 'warned', 'warned'),
        ],
    } as InstanceUpgradePlan
    const groups = groupUpgradeIssues(plan)

    expect(groups.blocking.map((group) => group.item.contentId)).toEqual(['blocked'])
    expect(groups.warnings.map((group) => group.item.contentId)).toEqual(['warned'])
    expect(groups.noIssues.map((group) => group.item.contentId)).toEqual(['clear'])
    expect(
        new Set(
            [...groups.blocking, ...groups.warnings, ...groups.noIssues].map(
                (group) => group.item.contentId,
            ),
        ).size,
    ).toBe(3)
})

it('initial blockers stay in blocking presentation without duplication after resolution', () => {
    const initialPlan = {
        items: [planItem('voxy'), planItem('clear')],
        blockingIssues: [issue('prerelease_only', 'voxy', 'voxy')],
        warnings: [],
    } as InstanceUpgradePlan
    const initial = captureInitialUpgradeBlockingIssues(initialPlan)
    const resolvedPlan = {
        ...initialPlan,
        blockingIssues: [],
        warnings: [issue('keep_incompatible', 'voxy', 'voxy')],
    } as InstanceUpgradePlan
    const groups = groupUpgradeIssues(resolvedPlan, initial)

    expect(groups.blocking.map((group) => group.item.contentId)).toEqual(['voxy'])
    expect(groups.blocking[0].currentlyBlocking).toBe(false)
    expect(groups.blocking[0].warnings.length).toBe(2)
    expect(groups.warnings.length).toBe(0)
    expect(groups.noIssues.map((group) => group.item.contentId)).toEqual(['clear'])
})

it('resolution presentation follows authoritative plan resolution rules', () => {
    const resolution = planItem('item').resolution
    expect(upgradeResolutionPresentation('two-option', resolution)).toEqual({
        selectedAction: null,
        showUndo: false,
    })
    expect(upgradeResolutionPresentation('two-option', { ...resolution, action: 'keep' })).toEqual({
        selectedAction: 'keep',
        showUndo: false,
    })
    expect(
        upgradeResolutionPresentation('two-option', { ...resolution, action: 'disable' }),
    ).toEqual({ selectedAction: 'disable', showUndo: false })
    expect(upgradeResolutionPresentation('single-prerelease', resolution)).toEqual({
        selectedAction: null,
        showUndo: false,
    })
    expect(
        upgradeResolutionPresentation('single-prerelease', {
            ...resolution,
            allowPrerelease: true,
        }),
    ).toEqual({ selectedAction: 'upgrade', showUndo: true })
})

it('root and content forms of one issue coalesce on exact project identity', () => {
    const plan = {
        items: [planItem('content', 'project')],
        blockingIssues: [
            issue('no_compatible_release', null, 'project', 'root form'),
            issue('no_compatible_release', 'content', 'project', 'content form'),
        ],
        warnings: [],
    } as InstanceUpgradePlan
    const groups = groupUpgradeIssues(plan)

    expect(groups.blocking[0].blockingIssues.length).toBe(1)
    expect(groups.blocking[0].blockingIssues[0].message).toBe('content form')
})

it('unmapped and ambiguous project issues remain global', () => {
    const duplicate = planItem('duplicate-b', 'duplicate')
    const plan = {
        items: [planItem('duplicate-a', 'duplicate'), duplicate],
        blockingIssues: [issue('dependency_conflict', null, 'missing')],
        warnings: [issue('keep_incompatible', null, 'duplicate')],
    } as InstanceUpgradePlan
    const groups = groupUpgradeIssues(plan)

    expect(groups.globalBlockingIssues.length).toBe(1)
    expect(groups.globalWarnings.length).toBe(1)
    expect(groups.noIssues.length).toBe(2)
})

it('actionable warning filtering excludes global and informational conflicts', () => {
    const plan = {
        items: [planItem('actionable'), planItem('conflict')],
        blockingIssues: [],
        warnings: [
            issue('keep_incompatible', 'actionable', 'actionable'),
            issue('dependency_conflict', 'conflict', 'conflict'),
            issue('keep_incompatible', null, null),
        ],
    } as InstanceUpgradePlan

    expect(actionableWarningContentIds(groupUpgradeIssues(plan))).toEqual(['actionable'])
})

it('actionable warning count uses unique content rows', () => {
    const plan = {
        items: [planItem('one'), planItem('two')],
        blockingIssues: [],
        warnings: [
            issue('keep_incompatible', 'one', 'one'),
            issue('shader_runtime_unknown', 'one', 'one'),
            issue('unidentified', 'two', 'two'),
        ],
    } as InstanceUpgradePlan

    expect(actionableWarningContentIds(groupUpgradeIssues(plan)).length).toBe(2)
})

it('content display metadata prefers normalized content then snapshot then plan fallback', () => {
    const item = planItem('entry', 'plan-project') as never
    const snapshot = {
        expectedRelativePath: 'resourcepacks/file.zip',
        content: {
            project: { title: 'Snapshot title', icon_url: 'snapshot.png' },
            version: { version_number: 'snapshot-version' },
        },
    } as never
    const content = {
        project: { title: 'Resolved title', icon_url: 'resolved.png' },
        version: { version_number: 'resolved-version' },
    } as never

    expect(upgradeContentDisplayMetadata(item, content, snapshot)).toEqual({
        title: 'Resolved title',
        iconUrl: 'resolved.png',
        currentVersion: 'resolved-version',
    })
    expect(upgradeContentDisplayMetadata(item, undefined, snapshot)).toEqual({
        title: 'Snapshot title',
        iconUrl: 'snapshot.png',
        currentVersion: 'snapshot-version',
    })
    expect(upgradeContentDisplayMetadata(item).title).toBe('entry.jar')
})

it('local content identity joins by normalized path when entry ids are absent', () => {
    expect(contentIdentityKeys({ relativePath: 'resourcepacks\\pack.zip' })).toEqual([
        'resourcepacks/pack.zip',
    ])
    expect(
        contentIdentityKeys({ instanceEntryId: 'entry', relativePath: 'resourcepacks/pack.zip' }),
    ).toEqual(['entry', 'resourcepacks/pack.zip'])
})

it('Minecraft formatting codes are removed from display title only', () => {
    const item = planItem('identity')
    item.relativePath = 'resourcepacks/§9§lExample §rPack.zip'
    const originalPath = item.relativePath
    expect(sanitizeMinecraftDisplayTitle('§9§lExample §rPack')).toBe('Example Pack')
    expect(upgradeContentDisplayMetadata(item).title).toBe('Example Pack.zip')
    expect(item.relativePath).toBe(originalPath)
    expect(item.contentId).toBe('identity')
})
