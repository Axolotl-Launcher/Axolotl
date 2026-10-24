import assert from 'node:assert/strict'
import test from 'node:test'

import {
    AUTOMATIC_FABRIC_LOADER_VERSION,
    automaticFabricLoaderTargetAvailable,
    confirmUpgradeOptions,
    fabricUpgradeLoaderVersions,
    newerStableGameVersions,
    preserveFabricLoaderSelection,
    resolveUpgradePlanSelection,
    solutionSummary,
} from './analysis.ts'
import { filterBulkResolutionIds, upgradeProgressModel } from './flow-controls.ts'
import { isCurrentUpgradeSelectPlanning } from './planning-navigation.ts'
import type { InstanceUpgradePlan, InstanceUpgradeTargetEnvironment } from './types.ts'
import { paginateUpgradeWarnings } from './upgrade-warning.ts'

test('upgrade candidates follow release order and never offer the current release', () => {
    const metadata = [
        { version: '26.3-snapshot', version_type: 'snapshot', date: '' },
        { version: '26.2', version_type: 'release', date: '' },
        { version: '1.21.11', version_type: 'release', date: '' },
        { version: '1.21.1', version_type: 'release', date: '' },
    ]
    assert.deepEqual(newerStableGameVersions(metadata, '1.21.1'), {
        currentFound: true,
        versions: ['26.2', '1.21.11'],
    })
    assert.deepEqual(newerStableGameVersions(metadata, '26.2').versions, [])
})

test('Fabric choices forbid downgrade and invalidate a selection unavailable for the target', () => {
    assert.deepEqual(fabricUpgradeLoaderVersions('0.19.2', ['0.19.5', '0.19.2', '0.18.4']), [
        '0.19.5',
        '0.19.2',
    ])
    assert.equal(
        preserveFabricLoaderSelection('0.18.4', ['0.19.5']),
        AUTOMATIC_FABRIC_LOADER_VERSION,
    )
    assert.equal(automaticFabricLoaderTargetAvailable(true, true, []), false)
})

test('shared upgrades require an explicit mode and copy upgrades do not make a second backup', () => {
    assert.equal(confirmUpgradeOptions(true, null, true).canStart, false)
    assert.deepEqual(confirmUpgradeOptions(true, 'copy_and_upgrade', true), {
        effectiveMode: 'copy_and_upgrade',
        createFullBackup: false,
        canStart: true,
    })
    assert.deepEqual(confirmUpgradeOptions(false, null, true), {
        effectiveMode: 'direct',
        createFullBackup: true,
        canStart: true,
    })
})

test('returning to target selection reuses the same plan only for the same instance and environment', async () => {
    const environment: InstanceUpgradeTargetEnvironment = {
        gameVersion: '26.2',
        modLoader: 'fabric',
        modLoaderVersion: null,
        shaderRuntime: 'none',
    }
    const plan = {
        id: 'plan',
        instanceId: 'original',
        targetEnvironment: environment,
    } as InstanceUpgradePlan
    let requests = 0
    const build = async () => {
        requests++
        return plan
    }
    assert.equal(
        (await resolveUpgradePlanSelection('original', plan, environment, build)).reused,
        true,
    )
    assert.equal(requests, 0)
    assert.equal(
        (await resolveUpgradePlanSelection('other', plan, environment, build)).reused,
        false,
    )
    assert.equal(requests, 1)
})

test('confirmation reports chosen changes and bulk resolution skips already resolved items', () => {
    const changes = [
        { contentId: 'a', action: 'keep' as const },
        { contentId: 'b', action: 'disable' as const },
    ]
    assert.deepEqual(filterBulkResolutionIds(changes, 'keep'), ['b'])
    assert.deepEqual(
        solutionSummary({
            kind: 'custom',
            selections: changes as never,
            dependencyChanges: [{ kind: 'add' }] as never,
            warnings: [],
        }),
        {
            upgraded: 0,
            kept: 1,
            disabled: 1,
            dependencyAdditions: 1,
            dependencyUpdates: 0,
            dependencyRemovals: 0,
        },
    )
})

test('result pagination clamps after filtering and the last route completes the stepper', () => {
    assert.deepEqual(paginateUpgradeWarnings([], 4), {
        items: [],
        page: 1,
        pageCount: 1,
        start: 0,
        end: 0,
        total: 0,
    })
    assert.equal(upgradeProgressModel('/instance/example/upgrade/result').complete, true)
    assert.equal(upgradeProgressModel('/instance/example/upgrade/confirm').currentIndex, 3)
})

test('the preview selection route accepts the current plan and rejects navigation races', () => {
    assert.equal(
        isCurrentUpgradeSelectPlanning(false, 1, 1, 'InstanceUpgradeSelect', 'a', 'a'),
        true,
    )
    assert.equal(
        isCurrentUpgradeSelectPlanning(false, 1, 2, 'InstanceUpgradeSelect', 'a', 'a'),
        false,
    )
    assert.equal(
        isCurrentUpgradeSelectPlanning(false, 1, 1, 'InstanceUpgradeCompatibility', 'a', 'a'),
        false,
    )
})
