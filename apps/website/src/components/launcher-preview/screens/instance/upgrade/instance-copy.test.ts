import assert from 'node:assert/strict'
import test from 'node:test'

import { type InstanceCopySnapshot, rebindInstanceCopy } from './instance-copy.ts'

test('full copies preserve edited worlds, files and settings with independent ownership', () => {
    const original = {
        worlds: [{ instance_id: 'source', path: 'World', name: 'My world', type: 'singleplayer' }],
        levels: [
            [
                'World',
                {
                    seed: '9223372036854775807',
                    game_rules: [{ key: 'keepInventory', value: 'true' }],
                },
            ],
        ],
        settings: {
            custom_env_vars: [['JAVA_TOOL_OPTIONS', '-Duser.language=zh']],
            game_dir_override: '/shared/game',
            linked_dot_minecraft: '/shared/.minecraft',
        },
        files: {
            kind: 'dir',
            name: 'source',
            children: [{ kind: 'file', name: 'options.txt', text: 'fov:90' }],
        },
        logs: [{ filename: 'latest.log', lines: ['Original log'] }],
        screenshots: [
            {
                id: 'shot-1',
                instance_id: 'source',
                instance_name: 'Original',
                path: '/launcher-preview/shot.webp',
            },
        ],
        rollbacks: { item: { fileName: 'mod.jar', versionId: 'previous' } },
        coreComponents: [
            { component: { id: 'core', fileName: 'custom.jar' }, entries: ['Main.class'] },
        ],
    } as unknown as InstanceCopySnapshot
    const copy = rebindInstanceCopy(original, { id: 'backup', name: 'Backup' })
    assert.equal(copy.worlds[0].instance_id, 'backup')
    assert.equal(copy.levels[0][1].seed, '9223372036854775807')
    assert.equal(copy.settings.game_dir_override, null)
    assert.equal(copy.settings.linked_dot_minecraft, null)
    assert.equal(copy.screenshots[0].id, 'backup:shot-1')
    assert.equal(copy.screenshots[0].path, original.screenshots[0].path)
    copy.worlds[0].name = 'Changed copy'
    copy.levels[0][1].game_rules[0].value = 'false'
    copy.logs[0].lines.push('New copy log')
    copy.settings.custom_env_vars![0][1] = 'changed'
    copy.coreComponents[0].entries.push('Other.class')
    assert.equal(original.worlds[0].name, 'My world')
    assert.equal(original.levels[0][1].game_rules[0].value, 'true')
    assert.deepEqual(original.logs[0].lines, ['Original log'])
    assert.equal(original.settings.custom_env_vars![0][1], '-Duser.language=zh')
    assert.deepEqual(original.coreComponents[0].entries, ['Main.class'])
})
