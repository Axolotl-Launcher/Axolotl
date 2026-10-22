import assert from 'node:assert/strict'
import { test } from 'node:test'

import { classifyServerLogLine, summarizeServerExit } from './log-parse.ts'
import { computeServerStatus } from './status.ts'
import {
    pickFabricInstallerVersion,
    requiredJavaMajorVersion,
    resolveServerJar,
    type PaperBuildsResponse,
    latestStablePaperBuild,
} from './server-types.ts'

test('maps legacy game versions to their required Java major', () => {
    assert.equal(requiredJavaMajorVersion('1.21.4'), 21)
    assert.equal(requiredJavaMajorVersion('1.20.5'), 21)
    assert.equal(requiredJavaMajorVersion('1.20.4'), 17)
    assert.equal(requiredJavaMajorVersion('1.17.1'), 17)
    assert.equal(requiredJavaMajorVersion('1.16.5'), 8)
    assert.equal(requiredJavaMajorVersion('1.12.2'), 8)
})

test('maps year-based game versions to Java 25', () => {
    assert.equal(requiredJavaMajorVersion('26.2'), 25)
    assert.equal(requiredJavaMajorVersion('26.3'), 25)
    assert.equal(requiredJavaMajorVersion('26.3-snapshot-6'), 25)
    assert.equal(requiredJavaMajorVersion('26.1'), 25)
    assert.equal(requiredJavaMajorVersion('26w14a'), 25)
    assert.equal(requiredJavaMajorVersion('25w46a'), 21)
    assert.equal(requiredJavaMajorVersion('unknown'), 25)
})

test('detects the eula notice in server logs', () => {
    assert.equal(
        classifyServerLogLine('[ServerMain/ERROR]: Failed to start the minecraft server')
            .eulaRequired,
        undefined,
    )
    assert.equal(
        classifyServerLogLine(
            '[ServerMain/ERROR]: You need to agree to the EULA in order to run the server. Go to eula.txt for more info.',
        ).eulaRequired,
        true,
    )
})

test('detects a fully started server', () => {
    assert.equal(
        classifyServerLogLine('[Server thread/INFO]: Done (3.542s)! For help, type "help"').started,
        true,
    )
})

test('first-run eula exit is not a crash', () => {
    const lines = [
        'Starting minecraft server version 1.21',
        'You need to agree to the EULA in order to run the server.',
    ]
    assert.deepEqual(summarizeServerExit(lines, 1), { crashed: false, eulaRequired: true })
    assert.deepEqual(summarizeServerExit(['Done (1.0s)!'], 0), {
        crashed: false,
        eulaRequired: false,
    })
    assert.deepEqual(summarizeServerExit(['Exception in thread "main"'], 1), {
        crashed: true,
        eulaRequired: false,
    })
})

test('resolves vanilla server jar from version info', () => {
    const jar = resolveServerJar('vanilla', {
        gameVersion: '1.21.4',
        vanillaVersionInfo: {
            downloads: {
                server: { sha1: 'abc', size: 100, url: 'https://example.com/server.jar' },
            },
        },
    })
    assert.equal(jar?.url, 'https://example.com/server.jar')
    assert.equal(jar?.filename, 'server.jar')
})

test('resolves fabric server launcher url', () => {
    const jar = resolveServerJar('fabric', {
        gameVersion: '1.21.4',
        loaderVersion: '0.16.9',
        installerVersion: '1.0.3',
    })
    assert.equal(
        jar?.url,
        'https://meta.fabricmc.net/v2/versions/loader/1.21.4/0.16.9/1.0.3/server/jar',
    )
    assert.equal(jar?.filename, 'fabric-server.jar')
})

test('fabric server jar requires an installer version', () => {
    assert.equal(
        resolveServerJar('fabric', { gameVersion: '1.21.4', loaderVersion: '0.16.9' }),
        null,
    )
})

test('picks the newest fabric installer version', () => {
    assert.equal(pickFabricInstallerVersion([{ version: '1.1.2', stable: true }]), '1.1.2')
    assert.equal(pickFabricInstallerVersion([]), null)
})

test('resolves paper server jar from the newest stable fill build', () => {
    const builds: PaperBuildsResponse = [
        {
            id: 112,
            channel: 'STABLE',
            downloads: {
                'server:default': {
                    name: 'paper-26.2-112.jar',
                    url: 'https://fill-data.papermc.io/v1/objects/abc/paper-26.2-112.jar',
                },
            },
        },
        {
            id: 113,
            channel: 'EXPERIMENTAL',
            downloads: {
                'server:default': {
                    name: 'paper-26.2-113.jar',
                    url: 'https://fill-data.papermc.io/v3/x',
                },
            },
        },
    ]
    const build = latestStablePaperBuild(builds)
    assert.equal(build?.id, 112)
    const jar = resolveServerJar('paper', { gameVersion: '26.2', paperBuild: build ?? undefined })
    assert.equal(jar?.url, 'https://fill-data.papermc.io/v1/objects/abc/paper-26.2-112.jar')
    assert.equal(jar?.filename, 'server.jar')

    assert.equal(latestStablePaperBuild([]), null)
    assert.equal(
        resolveServerJar('paper', {
            gameVersion: '26.2',
            paperBuild: { id: 1, channel: 'STABLE', downloads: {} },
        }),
        null,
    )
})

test('installer-based types resolve to null until implemented', () => {
    assert.equal(resolveServerJar('forge', { gameVersion: '1.21.4' }), null)
    assert.equal(resolveServerJar('neoforge', { gameVersion: '1.21.4' }), null)
})

test('computes server status precedence', () => {
    const base = {
        manifest: { id: 'a' },
        isRunning: false,
        isStarting: false,
        lastExitWasCrash: false,
        eulaAccepted: false,
        eulaFileExists: false,
    }
    assert.equal(computeServerStatus(base), 'created')
    assert.equal(computeServerStatus({ ...base, eulaFileExists: true }), 'eula_pending')
    assert.equal(computeServerStatus({ ...base, eulaAccepted: true }), 'ready')
    assert.equal(computeServerStatus({ ...base, isStarting: true }), 'starting')
    assert.equal(computeServerStatus({ ...base, isRunning: true }), 'running')
    assert.equal(
        computeServerStatus({ ...base, lastExitWasCrash: true, eulaAccepted: true }),
        'crashed',
    )
})
