import assert from 'node:assert/strict'
import test from 'node:test'

import type { PreviewClock } from '../../../lib/clock.ts'
import type { ContentItem } from '../../../lib/state/downloads.ts'
import type { GameInstance } from '../../../lib/types.ts'
import { applyUpgradeSelection, runUpgradeTransaction, upgradeSourceRevision } from './execution.ts'
import type { InstanceUpgradeSelection } from './types.ts'

function clock() {
    const tasks = new Set<(delta: number) => void>()
    return {
        every: (task: (delta: number) => void) => {
            tasks.add(task)
            return () => tasks.delete(task)
        },
        after: () => () => {},
        dispose: () => tasks.clear(),
        advance: (delta: number) => {
            for (const task of [...tasks]) task(delta)
        },
        get active() {
            return tasks.size
        },
    } satisfies PreviewClock & { advance: (delta: number) => void; active: number }
}
const source = () =>
    ({
        id: 'source',
        game_version: '1.21.1',
        loader: 'fabric',
        loader_version: '0.19.2',
        loader_components: [],
        link: null,
        symlink_target: null,
        install_stage: 'installed',
    }) as GameInstance
const file = () =>
    ({ id: 'mod', version_id: 'old', file_name: 'old.jar', enabled: false }) as ContentItem

test('canceling at the apply stage preserves source and releases the scheduled task', () => {
    const timer = clock()
    const instance = source()
    let commits = 0
    let finishes = 0
    const cancel = runUpgradeTransaction({
        clock: timer,
        sourceIsCurrent: () => true,
        progress: () => {},
        commit: () => {
            commits++
            instance.game_version = '26.2'
        },
        failed: assert.fail,
        finished: () => finishes++,
    })
    timer.advance(4100)
    cancel()
    timer.advance(2000)
    assert.equal(commits, 0)
    assert.equal(instance.game_version, '1.21.1')
    assert.equal(timer.active, 0)
    assert.equal(finishes, 1)
})

test('completion checks freshness before creating a backup or applying any content', () => {
    const timer = clock()
    const instance = source()
    const files = [file()]
    const revision = upgradeSourceRevision(instance, files)
    let commits = 0
    let failure: unknown
    runUpgradeTransaction({
        clock: timer,
        sourceIsCurrent: () => upgradeSourceRevision(instance, files) === revision,
        progress: () => {},
        commit: () => commits++,
        failed: (error) => {
            failure = error
        },
        finished: () => {},
    })
    timer.advance(2000)
    instance.link = { type: 'shared_instance', shared_instance_id: 'linked' }
    timer.advance(2200)
    assert.equal(commits, 0)
    assert.match(String(failure), /No changes were applied/)
    assert.equal(timer.active, 0)
})

test('a finished transaction commits exactly once even if canceled or advanced again', () => {
    const timer = clock()
    const instance = source()
    let commits = 0
    const cancel = runUpgradeTransaction({
        clock: timer,
        sourceIsCurrent: () => true,
        progress: () => {},
        commit: () => {
            commits++
            instance.game_version = '26.2'
        },
        failed: assert.fail,
        finished: () => {},
    })
    timer.advance(4200)
    timer.advance(4200)
    cancel()
    assert.equal(commits, 1)
    assert.equal(instance.game_version, '26.2')
})

test('updating a disabled item preserves the selected enabled state', () => {
    const previous = file()
    const updated = applyUpgradeSelection(
        previous,
        { action: 'upgrade', enabled: false } as InstanceUpgradeSelection,
        () => ({ ...previous, file_name: 'new.jar', version_id: 'new', enabled: true }),
    )
    assert.equal(updated.file_name, 'new.jar')
    assert.equal(updated.enabled, false)
    assert.equal(previous.version_id, 'old')
})

test('busy stage alone does not invalidate a plan, but components and virtual files do', () => {
    const instance = source()
    const files = [file()]
    const revision = upgradeSourceRevision(instance, files)
    instance.install_stage = 'minecraft_installing'
    assert.equal(upgradeSourceRevision(instance, files), revision)
    instance.loader_components = [{ instanceId: 'source', kind: 'optifabric', role: 'adjunct' }]
    assert.notEqual(upgradeSourceRevision(instance, files), revision)
    instance.loader_components = []
    files[0].enabled = true
    assert.notEqual(upgradeSourceRevision(instance, files), revision)
})
