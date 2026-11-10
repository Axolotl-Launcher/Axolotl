import assert from 'node:assert/strict'
import { afterEach, test } from 'node:test'

import { loadMinecraftDirectories } from './minecraft-directories.ts'

afterEach(() => {
    Reflect.deleteProperty(globalThis, 'localStorage')
})

test('loads configured Minecraft roots, modes and old string entries once per path', () => {
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: {
            getItem: () =>
                JSON.stringify([
                    ' /one ',
                    { path: '/two', mode: 'shared' },
                    { path: '/three', mode: 'isolated' },
                    { path: '/two', mode: 'automatic' },
                    null,
                    {},
                    { path: '  ' },
                ]),
        },
    })
    assert.deepEqual(loadMinecraftDirectories(), [
        { path: '/one', mode: 'automatic' },
        { path: '/two', mode: 'automatic' },
        { path: '/three', mode: 'isolated' },
    ])
})

test('unavailable, malformed and non-array storage yields no roots', () => {
    assert.deepEqual(loadMinecraftDirectories(), [])
    for (const raw of ['{', '{}', 'null']) {
        Object.defineProperty(globalThis, 'localStorage', {
            configurable: true,
            value: { getItem: () => raw },
        })
        assert.deepEqual(loadMinecraftDirectories(), [])
    }
})
