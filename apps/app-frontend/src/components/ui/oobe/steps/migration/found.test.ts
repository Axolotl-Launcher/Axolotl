import { describe, expect, it } from 'vitest'

import { folderKey, groupFound, instancePathOf, minecraftRootOf } from './found'

describe('minecraftRootOf', () => {
    it('finds the folder a version folder sits in', () => {
        expect(minecraftRootOf('C:\\Games\\.minecraft\\versions\\1.21.4')).toBe(
            'C:\\Games\\.minecraft',
        )
        expect(minecraftRootOf('D:/mc/Versions/Fabric 1.20/')).toBe('D:/mc')
    })

    it('ignores instances other launchers keep in their own layout', () => {
        expect(minecraftRootOf('C:/PrismLauncher/instances/Skyblock')).toBeUndefined()
        expect(minecraftRootOf('C:/Games/.minecraft')).toBeUndefined()
    })
})

describe('groupFound', () => {
    const pcl = {
        name: 'PCL2CE',
        path: 'C:/Users/me/AppData/Roaming',
        instances: [
            { name: '.minecraft:1.21.4', path: 'C:/Games/.minecraft/versions/1.21.4' },
            { name: 'Mods:Fabric', path: 'D:/Mods/.minecraft/versions/Fabric' },
            { name: '.minecraft:1.20.1', path: 'C:\\Games\\.minecraft\\versions\\1.20.1' },
        ],
    }

    it('gives each .minecraft folder its own entry with its versions', () => {
        const groups = groupFound([pcl])
        expect(groups.map((group) => [group.root, group.instances.length])).toEqual([
            ['C:/Games/.minecraft', 2],
            ['D:/Mods/.minecraft', 1],
        ])
        expect(groups.every((group) => group.launcher === pcl)).toBe(true)
    })

    it('keeps instances outside a .minecraft folder under their launcher', () => {
        const prism = {
            name: 'PrismLauncher',
            path: 'C:/PrismLauncher',
            instances: [
                { name: 'Skyblock', path: 'C:/PrismLauncher/instances/Skyblock' },
                { name: 'Shared', path: 'C:/PrismLauncher/.minecraft/versions/Shared' },
            ],
        }
        const groups = groupFound([prism])
        expect(groups.map((group) => [group.key, group.root])).toEqual([
            ['root:c:/prismlauncher/.minecraft', 'C:/PrismLauncher/.minecraft'],
            ['launcher:PrismLauncher', undefined],
        ])
    })

    it('lists a folder two launchers share once', () => {
        const legacy = { ...pcl, name: 'PCL2' }
        const groups = groupFound([pcl, legacy])
        expect(groups).toHaveLength(2)
        expect(groups[0]!.instances).toHaveLength(2)
        expect(groups.every((group) => group.launcher === pcl)).toBe(true)
    })

    it('groups shared game directories by version metadata and keeps versions distinct', () => {
        const instances = ['1.20.1', '1.21.1'].map((name) => ({
            name,
            path: 'D:/Shared/.minecraft',
            version_path: `D:/Shared/.minecraft/versions/${name}`,
            compatible_mode: true,
        }))
        const groups = groupFound([{ ...pcl, instances }])
        expect(groups).toHaveLength(1)
        expect(groups[0]!.root).toBe('D:/Shared/.minecraft')
        expect(groups[0]!.instances.map(instancePathOf)).toEqual([
            'D:/Shared/.minecraft/versions/1.20.1',
            'D:/Shared/.minecraft/versions/1.21.1',
        ])
    })

    it('reserves the default game directory for the official launcher even if PCL reports it first', () => {
        const root = 'C:/Users/me/AppData/Roaming/.minecraft'
        const instances = [{ name: '1.21.1', path: `${root}/versions/1.21.1` }]
        const official = {
            name: 'MinecraftLauncher',
            launcherType: 'Generic',
            path: root,
            instances,
        }
        const groups = groupFound([{ ...pcl, instances }, official])
        expect(groups).toHaveLength(1)
        expect(groups[0]!.launcher).toBe(official)
        expect(groups[0]!.root).toBe(root)
        expect(groups[0]!.instances).toHaveLength(1)
    })

    it('shows an existing empty official directory as a directly usable folder', () => {
        const official = {
            name: 'MinecraftLauncher',
            path: 'C:/Users/me/AppData/Roaming/.minecraft',
            instances: [],
        }
        expect(groupFound([official])).toEqual([
            {
                key: `root:${folderKey(official.path)}`,
                launcher: official,
                root: official.path,
                instances: [],
            },
        ])
    })

    it('compares folders by either separator and in any case', () => {
        expect(folderKey('C:\\Games\\.Minecraft\\')).toBe(folderKey('c:/games/.minecraft'))
    })
})
