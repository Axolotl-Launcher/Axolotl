import { expect, it } from 'vitest'

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
    favorites: 'Favorites',
}

it('browse project tabs keep favorites after servers and preserve the route context', () => {
    const tabs = createBrowseProjectTabs(labels, '?i=instance-id')

    expect(tabs.map((tab) => tab.label)).toEqual([
        'Modpacks',
        'Mods',
        'Resource Packs',
        'Data Packs',
        'Maps',
        'Shaders',
        'Servers',
        'Favorites',
    ])
    expect(tabs.at(-1)?.href).toBe('/browse/favorites?i=instance-id')
    expect(tabs.at(-1)?.onboardingId).toBe('browse-favorites-tab')
})

it('browse project tabs preserve content-context visibility while keeping favorites available', () => {
    const tabs = createBrowseProjectTabs(labels, '', {
        modpacks: false,
        mods: false,
        datapacks: false,
        servers: false,
    })

    expect(tabs.find((tab) => tab.label === 'Modpacks')?.shown).toBe(false)
    expect(tabs.find((tab) => tab.label === 'Mods')?.shown).toBe(false)
    expect(tabs.find((tab) => tab.label === 'Data Packs')?.shown).toBe(false)
    expect(tabs.find((tab) => tab.label === 'Servers')?.shown).toBe(false)
    expect(tabs.find((tab) => tab.label === 'Favorites')?.shown).toBe(true)
})

it('browse project tab visibility follows the selected instance capabilities', () => {
    expect(supportsDataPacks('1.12.2')).toBe(false)
    expect(supportsDataPacks('1.13')).toBe(true)

    expect(
        getBrowseProjectTabOptions({
            instance: { game_version: '1.12.2', loader: 'vanilla' },
            hasInstanceContext: true,
        }),
    ).toEqual({ modpacks: false, mods: false, datapacks: false, servers: false })
    expect(
        getBrowseProjectTabOptions({
            instance: { game_version: '1.20.1', loader: 'fabric' },
            hasInstanceContext: true,
            isServerInstance: true,
        }),
    ).toEqual({ modpacks: false, mods: true, datapacks: false, servers: false })
})
