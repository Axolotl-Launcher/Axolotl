import { expect, it } from 'vitest'

import {
    createMojangAuthMonitor,
    type MojangAuthConfiguration,
    type MojangProxy,
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
    const applied: Array<[string, string | null]> = []
    const published: boolean[] = []
    const deps: Parameters<typeof createMojangAuthMonitor>[0] = {
        configuration: async () => configuration,
        probeOfficial: async () => {
            probes++
            return healthy()
        },
        probeProxy: async () => healthy(),
        applySource: async (source, proxy) => {
            applied.push([source, proxy])
            return true
        },
        probeSelected: async () => {},
        enabled: () => enabled,
        publish: (reachable) => published.push(reachable),
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

it('requires core authentication services without depending on auxiliary hosts', () => {
    expect(officialMojangServicesReachable(healthy())).toBe(true)
    expect(
        officialMojangServicesReachable([...healthy(), { service: 'auth', reachable: false }]),
    ).toBe(true)
    expect(officialMojangServicesReachable(healthy().filter((s) => s.service !== 'session'))).toBe(
        false,
    )
    expect(
        officialMojangServicesReachable(
            healthy().filter((s) => ['session', 'services'].includes(s.service)),
        ),
    ).toBe(true)
    expect(officialMojangServicesReachable([])).toBe(false)
})

it('startup and periodic callers share one slow background probe', async () => {
    const f = fixture()
    const response = deferred<ReturnType<typeof healthy>>()
    f.deps.probeOfficial = () => response.promise
    const first = f.monitor.run()
    await Promise.resolve()
    expect(f.monitor.run()).toBe(first)
    expect(f.applied).toEqual([])
    response.resolve(healthy())
    expect(await first).toBe(true)
    expect(f.applied).toEqual([['auto', null]])
    expect(f.published).toEqual([true])
})

it('falls back on one failed official round and returns to official on a later successful check', async () => {
    const f = fixture('official_preferred')
    f.deps.probeOfficial = async () => healthy().map((status) => ({ ...status, reachable: false }))
    await f.monitor.run()
    expect(f.applied).toEqual([['official_preferred', 'fallen']])
    f.deps.probeOfficial = async () => healthy()
    await f.monitor.run()
    expect(f.applied.at(-1)).toEqual(['official_preferred', null])
})

it('explicit choices never probe official hosts or switch to another source', async () => {
    const explicit: Array<[MojangAuthConfiguration['source'], string | null]> = [
        ['official_only', null],
        ['mirror_preferred', 'fallen'],
        ['sakurin_preferred', 'sakurin'],
    ]
    for (const [source, proxy] of explicit) {
        const f = fixture(source)
        await f.monitor.run()
        expect(f.probes()).toBe(0)
        expect(f.applied).toEqual([[source, proxy]])
    }
})

it('a settings change during a slow probe discards its result and reconciles the latest choice', async () => {
    const f = fixture()
    const response = deferred<ReturnType<typeof healthy>>()
    f.deps.probeOfficial = () => response.promise
    const pending = f.monitor.run()
    await Promise.resolve()
    await Promise.resolve()
    f.configure({ source: 'official_only', key: 'new' })
    response.resolve([])
    await pending
    expect(f.applied).toEqual([['official_only', null]])
    expect(f.published).toEqual([true])
})

it('failed stale probes still allow a queued configuration change to run', async () => {
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
    expect(f.applied).toEqual([['mirror_preferred', 'fallen']])
})

it('offline and disposal invalidate in-flight results and prevent new work', async () => {
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
        expect(f.applied).toEqual([])
        expect(f.published).toEqual([])
        expect(await f.monitor.run()).toBe(null)
    }
})

it('probe and backend failures recover and unchanged settings do not need a new round', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () => {
        throw new Error('IPC failed')
    }
    await expect(f.monitor.run()).rejects.toThrow(/IPC failed/)
    expect(f.applied).toEqual([])
    f.deps.probeOfficial = async () => healthy()
    f.deps.probeSelected = async () => {
        throw new Error('unreachable')
    }
    expect(await f.monitor.run()).toBe(false)
    f.deps.probeSelected = async () => {}
    expect(await f.monitor.run()).toBe(true)
    expect(await f.monitor.configurationChanged()).toBe(false)
    f.configure({ source: 'auto', key: 'new-proxy' })
    expect(await f.monitor.configurationChanged()).toBe(true)
})

it('backend rejection of a stale source retries with the latest configuration', async () => {
    const f = fixture()
    let calls = 0
    f.deps.applySource = async (source, proxy) => {
        if (++calls === 1) {
            f.configure({ source: 'official_only', key: 'explicit' })
            return false
        }
        f.applied.push([source, proxy])
        return true
    }
    await f.monitor.run()
    expect(f.applied).toEqual([['official_only', null]])
})

it('settings changes while applying a source suppress the old connectivity probe', async () => {
    const f = fixture('official_only')
    const response = deferred<boolean>()
    const started = deferred<void>()
    let calls = 0
    f.deps.applySource = async (source, proxy) => {
        f.applied.push([source, proxy])
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
    expect(f.applied).toEqual([
        ['official_only', null],
        ['mirror_preferred', 'fallen'],
    ])
    expect(f.published).toEqual([true])
})

it('offline during a selected-source probe suppresses its result and reconnect starts a fresh round', async () => {
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
    expect(f.published).toEqual([])
    f.enable(true)
    f.monitor.invalidate()
    f.deps.probeSelected = async () => {}
    await f.monitor.run()
    expect(f.published).toEqual([true])
})

it('never selects an unreachable mirror and recovers to official later', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () => []
    f.deps.probeProxy = async () => []
    f.deps.probeSelected = async () => {
        throw new Error('unreachable')
    }
    expect(await f.monitor.run()).toBe(false)
    expect(f.applied).toEqual([['auto', null]])
    f.deps.probeOfficial = async () => healthy()
    f.deps.probeSelected = async () => {}
    expect(await f.monitor.run()).toBe(true)
    expect(f.applied.at(-1)).toEqual(['auto', null])
})

it('does not probe a mirror when core official services work', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () =>
        healthy().filter((s) => ['session', 'services'].includes(s.service))
    f.deps.probeProxy = async () => {
        throw new Error('must not probe')
    }
    expect(await f.monitor.run()).toBe(true)
    expect(f.applied).toEqual([['auto', null]])
})

it('a network change during a mirror probe discards the old decision', async () => {
    const f = fixture()
    const response = deferred<ReturnType<typeof healthy>>()
    const started = deferred<void>()
    f.deps.probeOfficial = async () => []
    f.deps.probeProxy = () => {
        started.resolve()
        return response.promise
    }
    const pending = f.monitor.run()
    await started.promise
    f.configure({ source: 'official_only', key: 'new-network' })
    f.monitor.invalidate()
    response.resolve(healthy())
    await pending
    expect(f.applied).toEqual([['official_only', null]])
})

it('uses a healthy mirror when Minecraft Services fails despite a working session', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () =>
        healthy().map((s) => ({
            ...s,
            reachable: s.service !== 'services',
        }))
    expect(await f.monitor.run()).toBe(true)
    expect(f.applied).toEqual([['auto', 'fallen']])
})

it('a mirror with only a working session is not a usable authentication fallback', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () => []
    f.deps.probeProxy = async () => [{ service: 'session', reachable: true }]
    await f.monitor.run()
    expect(f.applied).toEqual([['auto', null]])
})

it('falls through to the second proxy when the first is unreachable', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () => []
    const probed: MojangProxy[] = []
    f.deps.probeProxy = async (proxy: MojangProxy) => {
        probed.push(proxy)
        return proxy === 'sakurin' ? healthy() : []
    }
    expect(await f.monitor.run()).toBe(true)
    expect(probed).toEqual(['fallen', 'sakurin'])
    expect(f.applied).toEqual([['auto', 'sakurin']])
})

it('stops at the first reachable proxy instead of probing the rest', async () => {
    const f = fixture()
    f.deps.probeOfficial = async () => []
    const probed: MojangProxy[] = []
    f.deps.probeProxy = async (proxy: MojangProxy) => {
        probed.push(proxy)
        return healthy()
    }
    await f.monitor.run()
    expect(probed).toEqual(['fallen'])
    expect(f.applied).toEqual([['auto', 'fallen']])
})

it('an explicit Sakurin choice applies it without probing any proxy', async () => {
    const f = fixture('sakurin_preferred')
    f.deps.probeProxy = async () => {
        throw new Error('must not probe')
    }
    expect(await f.monitor.run()).toBe(true)
    expect(f.applied).toEqual([['sakurin_preferred', 'sakurin']])
})
