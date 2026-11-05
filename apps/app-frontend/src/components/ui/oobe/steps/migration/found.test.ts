import { describe, expect, it } from 'vitest'

import { folderKey, groupFound, minecraftRootOf } from './found'

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

    it('compares folders by either separator and in any case', () => {
        expect(folderKey('C:\\Games\\.Minecraft\\')).toBe(folderKey('c:/games/.minecraft'))
    })
})
