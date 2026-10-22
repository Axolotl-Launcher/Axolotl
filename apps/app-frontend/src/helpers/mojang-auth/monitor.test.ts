import assert from 'node:assert/strict'
import test from 'node:test'

import {
    createMojangAuthMonitor,
    type MojangAuthConfiguration,
    officialMojangServicesReachable,
} from './monitor.ts'

const healthy = () =>
    ['account', 'session', 'services', 'profiles'].map((service) => ({ service, reachable: true }))
function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (error: unknown) => void
    const promise = new Promise<T>((yes, no) => {
        resolve = yes
        reject = no
    })
    return { promise, resolve, reject }
}
function fixture(source: MojangAuthConfiguration['source'] = 'auto') {
    let configuration = { source, key: source as string }
    let enabled = true
    let probes = 0
    const applied: Array<[string, boolean]> = []
    const published: boolean[] = []
    const deps = {
        configuration: async () => configuration,
        probeOfficial: async () => {
            probes++
            return healthy()
        },
        probeMirror: async () => healthy(),
        applySource: async (source: string, mirror: boolean) => {
            applied.push([source, mirror])
            return true
        },
        probeSelected: async () => {},
        enabled: () => enabled,
        publish: (reachable: boolean) => published.push(reachable),
    }
    const monitor = createMojangAuthMonitor(deps)
    return {
        monitor,
        deps,
        applied,
        published,
        probes: () => probes,
        configure: (next: MojangAuthConfiguration) => {
            configuration = next
        },
        enable: (next: boolean) => {
            enabled = next
        },
    }
}

test('requires core authentication services without depending on auxiliary hosts', () => {
    assert.equal(officialMojangServicesReachable(healthy()), true)
    assert.equal(
        officialMojangServicesReachable([...healthy(), { service: 'auth', reachable: false }]),
        true,
    )
    assert.equal(
        officialMojangServicesReachable(healthy().filter((s) => s.service !== 'session')),
        false,
    )
    assert.equal(
        officialMojangServicesReachable(
            healthy().filter((s) => ['session', 'services'].includes(s.service)),
        ),
        true,
    )
    assert.equal(officialMojangServicesReachable([]), false)
})

test('startup and periodic callers share one slow background probe', async () => {
    const f = fixture()
    const response = deferred<ReturnType<typeof healthy>>()
    f.deps.probeOfficial = () => response.promise
    const first = f.monitor.run()
    await Promise.resolve()
    assert.equal(f.monitor.run(), first)
    assert.deepEqual(f.applied, [])
    response.resolve(healthy())
    assert.equal(await first, true)
    assert.deepEqual(f.applied, [['auto', false]])
    assert.deepEqual(f.published, [true])
})

test('falls back on one failed official round and returns to official on a later successful check', async () => {
    const f = fixture('official_preferred')
    f.deps.probeOfficial = async () => healthy().map((status) => ({ ...status, reachable: false }))
    await f.monitor.run()
    assert.deepEqual(f.applied, [['official_preferred', true]])
    f.deps.probeOfficial = async () => healthy()
    await f.monitor.run()
    assert.deepEqual(f.applied.at(-1), ['official_preferred', false])
})

test('explicit choices never probe official hosts or switch to another source', async () => {
    for (const source of ['official_only', 'mirror_preferred'] as const) {
        const f = fixture(source)
        await f.monitor.run()
        assert.equal(f.probes(), 0)
        assert.deepEqual(f.applied, [[source, source === 'mirror_preferred']])
    }
})

test('a settings change during a slow probe discards its result and reconciles the latest choice', async () => {
    const f = fixture()
    const response = deferred<ReturnType<typeof healthy>>()
    f.deps.probeOfficial = () => response.promise
    const pending = f.monitor.run()
    await Promise.resolve()
    await Promise.resolve()
    f.configure({ source: 'official_only', key: 'new' })
    response.resolve([])
    await pending
    assert.deepEqual(f.applied, [['official_only', false]])
    assert.deepEqual(f.published, [true])
})

test('failed stale probes still allow a queued configuration change to run', async () => {
    const f = fixture()
    const response = deferred<ReturnType<typeof healthy>>()
    f.deps.probeOfficial = () => response.promise
    const pending = f.monitor.run()
    await Promise.resolve()
    await Promise.resolve()
    f.configure({ source: 'mirror_preferred', key: 'new' })
    f.monitor.invalidate()
    response.reject(new Error('old request failed'))
    await pending
    assert.deepEqual(f.applied, [['mirror_preferred', true]])
})

test('offline and disposal invalidate in-flight results and prevent new work', async () => {
    for (const dispose of [false, true]) {
        const f = fixture()
        const response = deferred<ReturnType<typeof healthy>>()
        f.deps.probeOfficial = () => response.promise
        const pending = f.monitor.run()
        await Promise.resolve()
        await Promise.resolve()
        if (dispose) f.monitor.dispose()
        else {
            f.enable(false)
            f.monitor.invalidate()
        }
        response.resolve(healthy())
        await pending
        assert.deepEqual(f.applied, [])
        assert.deepEqual(f.published, [])
        assert.equal(await f.monitor.run(), null)
    }
})

test('probe and backend failures recover and unchanged settings do not need a new round', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () => {
        throw new Error('IPC failed')
    }
    await assert.rejects(f.monitor.run(), /IPC failed/)
    assert.deepEqual(f.applied, [])
    f.deps.probeOfficial = async () => healthy()
    f.deps.probeSelected = async () => {
        throw new Error('unreachable')
    }
    assert.equal(await f.monitor.run(), false)
    f.deps.probeSelected = async () => {}
    assert.equal(await f.monitor.run(), true)
    assert.equal(await f.monitor.configurationChanged(), false)
    f.configure({ source: 'auto', key: 'new-proxy' })
    assert.equal(await f.monitor.configurationChanged(), true)
})

test('backend rejection of a stale source retries with the latest configuration', async () => {
    const f = fixture()
    let calls = 0
    f.deps.applySource = async (source, mirror) => {
        if (++calls === 1) {
            f.configure({ source: 'official_only', key: 'explicit' })
            return false
        }
        f.applied.push([source, mirror])
        return true
    }
    await f.monitor.run()
    assert.deepEqual(f.applied, [['official_only', false]])
})

test('settings changes while applying a source suppress the old connectivity probe', async () => {
    const f = fixture('official_only')
    const response = deferred<boolean>()
    const started = deferred<void>()
    let calls = 0
    f.deps.applySource = async (source, mirror) => {
        f.applied.push([source, mirror])
        if (++calls === 1) {
            started.resolve()
            return response.promise
        }
        return true
    }
    const pending = f.monitor.run()
    await started.promise
    f.configure({ source: 'mirror_preferred', key: 'new' })
    f.monitor.invalidate()
    response.resolve(true)
    await pending
    assert.deepEqual(f.applied, [
        ['official_only', false],
        ['mirror_preferred', true],
    ])
    assert.deepEqual(f.published, [true])
})

test('offline during a selected-source probe suppresses its result and reconnect starts a fresh round', async () => {
    const f = fixture('official_only')
    const response = deferred<void>()
    const started = deferred<void>()
    f.deps.probeSelected = () => {
        started.resolve()
        return response.promise
    }
    const pending = f.monitor.run()
    await started.promise
    f.enable(false)
    f.monitor.invalidate()
    response.reject(new Error('old failure'))
    await pending
    assert.deepEqual(f.published, [])
    f.enable(true)
    f.monitor.invalidate()
    f.deps.probeSelected = async () => {}
    await f.monitor.run()
    assert.deepEqual(f.published, [true])
})

test('never selects an unreachable mirror and recovers to official later', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () => []
    f.deps.probeMirror = async () => []
    f.deps.probeSelected = async () => {
        throw new Error('unreachable')
    }
    assert.equal(await f.monitor.run(), false)
    assert.deepEqual(f.applied, [['auto', false]])
    f.deps.probeOfficial = async () => healthy()
    f.deps.probeSelected = async () => {}
    assert.equal(await f.monitor.run(), true)
    assert.deepEqual(f.applied.at(-1), ['auto', false])
})

test('does not probe a mirror when core official services work', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () =>
        healthy().filter((s) => ['session', 'services'].includes(s.service))
    f.deps.probeMirror = async () => {
        throw new Error('must not probe')
    }
    assert.equal(await f.monitor.run(), true)
    assert.deepEqual(f.applied, [['auto', false]])
})

test('a network change during a mirror probe discards the old decision', async () => {
    const f = fixture()
    const response = deferred<ReturnType<typeof healthy>>()
    const started = deferred<void>()
    f.deps.probeOfficial = async () => []
    f.deps.probeMirror = () => {
        started.resolve()
        return response.promise
    }
    const pending = f.monitor.run()
    await started.promise
    f.configure({ source: 'official_only', key: 'new-network' })
    f.monitor.invalidate()
    response.resolve(healthy())
    await pending
    assert.deepEqual(f.applied, [['official_only', false]])
})

test('uses a healthy mirror when Minecraft Services fails despite a working session', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () =>
        healthy().map((s) => ({
            ...s,
            reachable: s.service !== 'services',
        }))
    assert.equal(await f.monitor.run(), true)
    assert.deepEqual(f.applied, [['auto', true]])
})

test('a mirror with only a working session is not a usable authentication fallback', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () => []
    f.deps.probeMirror = async () => [{ service: 'session', reachable: true }]
    await f.monitor.run()
    assert.deepEqual(f.applied, [['auto', false]])
})
