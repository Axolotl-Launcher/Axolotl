import { expect, it } from 'vitest'
import { reactive } from 'vue'

import {
    clearUpgradeFlow,
    consumeUpgradeFlow,
    parkUpgradeFlow,
    peekUpgradeFlow,
    restoreUpgradeFlow,
    upgradeProjectPath,
} from '../../../helpers/upgrade-return-state.ts'
import type { UpgradeFlowSnapshot } from './flow.ts'

// The store only serializes routing identity; the plan and target fixtures below
// are intentionally partial DTOs.
function parkedSnapshot<T extends object>(snapshot: T): T & UpgradeFlowSnapshot {
    return snapshot as unknown as T & UpgradeFlowSnapshot
}

it('upgrade return snapshot is one-shot and instance-scoped', () => {
    clearUpgradeFlow()
    const snapshot = {
        instanceId: 'instance-a',
        returnFullPath: '/instance/instance-a/upgrade/compatibility',
        targetEnvironment: null,
        plan: null,
        createFullBackup: true,
        sharedUpgradeMode: null,
        activeJobId: null,
        result: null,
    }
    parkUpgradeFlow(snapshot)
    expect(consumeUpgradeFlow('instance-b', snapshot.returnFullPath)).toBe(null)
    expect(consumeUpgradeFlow('instance-a', snapshot.returnFullPath)).toEqual(snapshot)
    expect(consumeUpgradeFlow('instance-a', snapshot.returnFullPath)).toBe(null)
})

for (const route of ['compatibility', 'customize', 'confirm']) {
    it(`${route} return hydrates the parked plan before consuming it`, () => {
        clearUpgradeFlow()
        const snapshot = parkedSnapshot({
            instanceId: 'instance-a',
            returnFullPath: `/instance/instance-a/upgrade/${route}`,
            targetEnvironment: { gameVersion: '26.1.2' },
            plan: { id: 'same-plan' },
            createFullBackup: true,
            sharedUpgradeMode: null,
            activeJobId: null,
            result: null,
        })
        parkUpgradeFlow(snapshot)
        let hydratedPlanId: string | undefined
        const restored = restoreUpgradeFlow('instance-a', snapshot.returnFullPath, (value) => {
            hydratedPlanId = value.plan?.id
        })
        expect(hydratedPlanId).toBe('same-plan')
        expect(restored?.plan?.id).toBe('same-plan')
        expect(peekUpgradeFlow('instance-a')).toBe(null)
    })
}

it('confirm project return restores plan and confirm choices without replanning', () => {
    clearUpgradeFlow()
    const snapshot = parkedSnapshot({
        instanceId: 'instance-a',
        returnFullPath: '/instance/instance-a/upgrade/confirm',
        targetEnvironment: { gameVersion: '26.1.2' },
        plan: {
            id: 'same-plan',
            selectedSolution: { kind: 'custom' },
            customConstraints: [{ contentId: 'root', versionId: 'fixed' }],
        },
        createFullBackup: false,
        directFullBackupPreference: false,
        sharedUpgradeMode: 'direct',
        activeJobId: null,
        result: null,
    })
    parkUpgradeFlow(snapshot)
    const restored = restoreUpgradeFlow('instance-a', snapshot.returnFullPath, (value) => {
        expect(value.plan?.id).toBe('same-plan')
    })

    expect(restored?.plan?.id).toBe('same-plan')
    expect(restored?.targetEnvironment).toEqual(snapshot.targetEnvironment)
    expect(restored?.plan?.selectedSolution).toEqual(snapshot.plan.selectedSolution)
    expect(restored?.plan?.customConstraints).toEqual(snapshot.plan.customConstraints)
    expect(restored?.createFullBackup).toBe(false)
    expect(restored?.sharedUpgradeMode).toBe('direct')
})

it('failed hydration leaves the parked snapshot available', () => {
    clearUpgradeFlow()
    const snapshot = {
        instanceId: 'instance-a',
        returnFullPath: '/instance/instance-a/upgrade/compatibility',
        targetEnvironment: null,
        plan: null,
        createFullBackup: true,
        sharedUpgradeMode: null,
        activeJobId: null,
        result: null,
    }
    parkUpgradeFlow(snapshot)
    expect(() =>
        restoreUpgradeFlow('instance-a', snapshot.returnFullPath, () => {
            throw new Error('hydrate failed')
        }),
    ).toThrow()
    expect(peekUpgradeFlow('instance-a')).toEqual(snapshot)
})

it('confirm project title routes match trusted provider routes only', () => {
    expect(upgradeProjectPath('modrinth', 'P7dR8mSH')).toBe('/project/P7dR8mSH')
    expect(upgradeProjectPath('curseforge', '123')).toBe('/project/curseforge/123')
    expect(upgradeProjectPath('local', 'pack')).toBe(null)
    expect(upgradeProjectPath(null, 'unidentified')).toBe(null)
})

it('upgrade return snapshot detaches reactive flow DTOs', () => {
    clearUpgradeFlow()
    const snapshot = reactive({
        instanceId: 'reactive-instance',
        returnFullPath: '/instance/reactive-instance/upgrade/compatibility',
        targetEnvironment: null,
        plan: null,
        createFullBackup: true,
        sharedUpgradeMode: null,
        activeJobId: null,
        result: null,
    })
    parkUpgradeFlow(snapshot)
    snapshot.createFullBackup = false
    expect(peekUpgradeFlow('reactive-instance')?.createFullBackup).toBe(true)
    expect(consumeUpgradeFlow('wrong-instance', snapshot.returnFullPath)).toBe(null)
})
