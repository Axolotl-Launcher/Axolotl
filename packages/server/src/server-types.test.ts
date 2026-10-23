import assert from 'node:assert/strict'
import test from 'node:test'

import { isServerTypeSupported, neoForgeInstaller, SERVER_TYPES } from './server-types.ts'

test('NeoForge is an installable server type with a selectable loader version', () => {
    assert.equal(isServerTypeSupported('neoforge'), true)
    assert.equal(SERVER_TYPES.neoforge.installMode, 'installer')
    assert.equal(SERVER_TYPES.neoforge.needsLoaderVersion, true)
})

test('NeoForge installer coordinates cover modern, beta and legacy releases', () => {
    assert.deepEqual(neoForgeInstaller('1.21.1', '21.1.172'), {
        url: 'https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.172/neoforge-21.1.172-installer.jar',
        filename: 'neoforge-21.1.172-installer.jar',
    })
    assert.equal(
        neoForgeInstaller('26.1', '26.1.0.19-beta').filename,
        'neoforge-26.1.0.19-beta-installer.jar',
    )
    assert.deepEqual(neoForgeInstaller('1.20.1', '47.1.106'), {
        url: 'https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar',
        filename: 'forge-1.20.1-47.1.106-installer.jar',
    })
})
