import assert from 'node:assert/strict'
import test from 'node:test'

import { contentCompatibility } from './compatibility.ts'
import { compatibleVersions } from './content-types.ts'

test('installed release compatibility reacts to the upgraded game and loader', () => {
    const release = { gameVersions: ['1.21.1'], loaders: ['fabric'] }
    assert.equal(
        contentCompatibility({ project_type: 'mod' }, release, {
            game_version: '1.21.1',
            loader: 'fabric',
        }).gameVersion,
        'compatible',
    )
    const after = contentCompatibility({ project_type: 'mod' }, release, {
        game_version: '1.21.11',
        loader: 'neoforge',
    })
    assert.equal(after.gameVersion, 'incompatible')
    assert.equal(after.loader, 'incompatible')
})

test('missing declarations remain unknown and data packs are loader independent', () => {
    const instance = { game_version: '1.21.11', loader: 'vanilla' as const }
    assert.equal(
        contentCompatibility({ project_type: 'mod' }, null, instance).gameVersion,
        'unknown',
    )
    assert.equal(contentCompatibility({ project_type: 'mod' }, null, instance).loader, 'unknown')
    assert.equal(
        contentCompatibility(
            { project_type: 'datapack' },
            { gameVersions: ['1.21.11'], loaders: ['datapack'] },
            instance,
        ).loader,
        'compatible',
    )
})

test('no matching release never offers an incompatible update as a fallback', () => {
    const project = {
        project_type: 'mod',
        versions: [
            {
                id: 'release',
                game_versions: ['1.21.1'],
                loaders: ['fabric'],
                date_published: '2026-01-01',
            },
        ],
    }
    const instance = { game_version: '1.21.11', loader: 'fabric' }
    assert.deepEqual(compatibleVersions(project as never, instance as never), [])
})
