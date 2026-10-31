import { expect, it } from 'vitest'

import {
    type GameDirectoryFields,
    hasMultipleGameDirectories,
    resolveGameDirectory,
} from './game-directory'

function instance(fields: Partial<GameDirectoryFields> = {}): GameDirectoryFields {
    return {
        linked_dot_minecraft: null,
        game_dir_override: null,
        symlink_target: null,
        ...fields,
    }
}

it('uses the direct link before ordinary external game directories', () => {
    const directory = resolveGameDirectory(
        instance({
            linked_dot_minecraft: String.raw`C:\Linked\.minecraft`,
            game_dir_override: String.raw`D:\Override\versions\1.21`,
            symlink_target: String.raw`E:\Symlink\.minecraft`,
        }),
    )

    expect(directory).toMatchObject({
        key: 'external:c:/linked/.minecraft',
        kind: 'external',
        path: String.raw`C:\Linked\.minecraft`,
        shortLabel: String.raw`Linked\.minecraft`,
    })
})

it('uses a game-directory override before a symlink target', () => {
    expect(
        resolveGameDirectory(
            instance({
                game_dir_override: 'D:\\Minecraft\\.minecraft\\versions\\1.20.1\\',
                symlink_target: String.raw`E:\Ignored\.minecraft`,
            }),
        ),
    ).toMatchObject({
        key: 'external:d:/minecraft/.minecraft',
        path: String.raw`D:\Minecraft\.minecraft`,
        shortLabel: String.raw`Minecraft\.minecraft`,
    })

    expect(
        resolveGameDirectory(
            instance({
                symlink_target: '/home/alex/.minecraft/',
            }),
        ),
    ).toMatchObject({
        key: 'external:/home/alex/.minecraft',
        path: '/home/alex/.minecraft',
        shortLabel: 'alex/.minecraft',
    })
})

it('groups self-managed instances together', () => {
    expect(resolveGameDirectory(instance())).toEqual({
        key: 'managed',
        kind: 'managed',
        path: null,
        shortLabel: null,
    })
})

it('normalizes Windows keys case-insensitively but keeps Unix keys case-sensitive', () => {
    const windowsUpper = resolveGameDirectory(
        instance({ game_dir_override: String.raw`C:\Games\.minecraft` }),
    )
    const windowsLower = resolveGameDirectory(
        instance({ game_dir_override: 'c:/games/.minecraft/' }),
    )
    const unixUpper = resolveGameDirectory(instance({ game_dir_override: '/Games/.minecraft' }))
    const unixLower = resolveGameDirectory(instance({ game_dir_override: '/games/.minecraft' }))

    expect(windowsUpper.key).toBe(windowsLower.key)
    expect(unixUpper.key).not.toBe(unixLower.key)
})

it('counts distinct game directories across the full instance collection', () => {
    expect(
        hasMultipleGameDirectories([
            instance(),
            instance({ game_dir_override: String.raw`D:\Minecraft\.minecraft` }),
        ]),
    ).toBe(true)
    expect(
        hasMultipleGameDirectories([
            instance({ game_dir_override: String.raw`D:\Minecraft\.minecraft` }),
            instance({ game_dir_override: 'd:/minecraft/.minecraft/' }),
        ]),
    ).toBe(false)
})
