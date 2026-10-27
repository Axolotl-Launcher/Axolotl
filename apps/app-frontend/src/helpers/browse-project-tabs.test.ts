import assert from 'node:assert/strict'
import test from 'node:test'

import {
    createBrowseProjectTabs,
    getBrowseProjectTabOptions,
    supportsDataPacks,
} from './browse-project-tabs.ts'

const labels = {
    modpacks: 'Modpacks',
    mods: 'Mods',
    resourcepacks: 'Resource Packs',
    datapacks: 'Data Packs',
    maps: 'Maps',
    shaders: 'Shaders',
    servers: 'Servers',
    schematics: 'Litematica',
    favorites: 'Favorites',
}

test('browse project tabs keep favorites after servers and preserve the route context', () => {
    const tabs = createBrowseProjectTabs(labels, '?i=instance-id')

    assert.deepEqual(
        tabs.map((tab) => tab.label),
        [
            'Modpacks',
            'Mods',
            'Resource Packs',
            'Data Packs',
            'Maps',
            'Shaders',
            'Servers',
            'Litematica',
            'Favorites',
        ],
    )
    assert.equal(tabs.at(-1)?.href, '/browse/favorites?i=instance-id')
    assert.equal(tabs.at(-1)?.onboardingId, 'browse-favorites-tab')
})

test('browse project tabs preserve content-context visibility while keeping favorites available', () => {
    const tabs = createBrowseProjectTabs(labels, '', {
        modpacks: false,
        mods: false,
        datapacks: false,
        servers: false,
    })

    assert.equal(tabs.find((tab) => tab.label === 'Modpacks')?.shown, false)
    assert.equal(tabs.find((tab) => tab.label === 'Mods')?.shown, false)
    assert.equal(tabs.find((tab) => tab.label === 'Data Packs')?.shown, false)
    assert.equal(tabs.find((tab) => tab.label === 'Servers')?.shown, false)
    assert.equal(tabs.find((tab) => tab.label === 'Favorites')?.shown, true)
})

test('browse project tab visibility follows the selected instance capabilities', () => {
    assert.equal(supportsDataPacks('1.12.2'), false)
    assert.equal(supportsDataPacks('1.13'), true)

    assert.deepEqual(
        getBrowseProjectTabOptions({
            instance: { game_version: '1.12.2', loader: 'vanilla' },
            hasInstanceContext: true,
        }),
        { modpacks: false, mods: false, datapacks: false, servers: false },
    )
    assert.deepEqual(
        getBrowseProjectTabOptions({
            instance: { game_version: '1.20.1', loader: 'fabric' },
            hasInstanceContext: true,
            isServerInstance: true,
        }),
        { modpacks: false, mods: true, datapacks: false, servers: false },
    )
})
