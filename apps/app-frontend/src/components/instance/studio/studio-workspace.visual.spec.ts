import { afterEach, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'

import { useStudioTree } from './useStudioTree'
import { useStudioWatcher } from './useStudioWatcher'

const scopes: ReturnType<typeof effectScope>[] = []
afterEach(() => {
    scopes.splice(0).forEach((scope) => scope.stop())
    vi.useRealTimers()
})
function withinScope<T>(setup: () => T): T {
    const scope = effectScope()
    scopes.push(scope)
    return scope.run(setup)!
}
function deferred<T>() {
    let resolve!: (value: T) => void
    const promise = new Promise<T>((done) => {
        resolve = done
    })
    return { promise, resolve }
}
const file = (name: string) => ({ name, isDirectory: false })

it('retains loaded collapsed directories and sorts the visible tree after a refresh', async () => {
    const read = vi.fn(async (path: string) =>
        path ? [file('item2'), file('item10')] : [file('z'), { name: 'config', isDirectory: true }],
    )
    const tree = withinScope(() => useStudioTree(read, vi.fn()))
    await tree.loadRoot()
    await tree.toggleDirectory(tree.rootNodes.value[0])
    expect(tree.visibleNodes.value.map((node) => node.name)).toEqual([
        'config',
        'item2',
        'item10',
        'z',
    ])
    await tree.toggleDirectory(tree.rootNodes.value[0])
    await tree.refreshTree()
    expect(tree.rootNodes.value[0]).toMatchObject({ loaded: true, expanded: false })
    expect(tree.visibleNodes.value.map((node) => node.name)).toEqual(['config', 'z'])
    await tree.toggleDirectory(tree.rootNodes.value[0])
    expect(tree.visibleNodes.value).toHaveLength(4)
})

it('does not let an older refresh overwrite the latest directory response', async () => {
    const older = deferred<ReturnType<typeof file>[]>()
    const newer = deferred<ReturnType<typeof file>[]>()
    const read = vi
        .fn()
        .mockResolvedValueOnce([file('initial')])
        .mockReturnValueOnce(older.promise)
        .mockReturnValueOnce(newer.promise)
    const tree = withinScope(() => useStudioTree(read, vi.fn()))
    await tree.loadRoot()
    const first = tree.refreshTree()
    const second = tree.refreshTree()
    newer.resolve([file('new')])
    await second
    older.resolve([file('old')])
    await first
    expect(tree.visibleNodes.value.map((node) => node.name)).toEqual(['new'])
})

it('invalidates pending root loads when the workspace resets or unmounts', async () => {
    const pending = deferred<ReturnType<typeof file>[]>()
    const tree = withinScope(() => useStudioTree(() => pending.promise, vi.fn()))
    const loading = tree.loadRoot()
    tree.reset()
    scopes[0].stop()
    pending.resolve([file('old-workspace')])
    await loading
    expect(tree.rootNodes.value).toEqual([])
})

function watcherServices() {
    return {
        listen: vi.fn(async (_handler: unknown) => vi.fn()),
        register: vi.fn(async (_id: string) => 'registration'),
        unregister: vi.fn(async (_id: string, _registration: string) => {}),
        watch: vi.fn(async (_path: string, _handler: (paths: string[]) => void) => vi.fn()),
    }
}

it('finishes loading when a refresh supersedes the initial root load', async () => {
    const old = deferred<ReturnType<typeof file>[]>()
    const read = vi
        .fn()
        .mockReturnValueOnce(old.promise)
        .mockResolvedValueOnce([file('fresh')])
    const tree = withinScope(() => useStudioTree(read, vi.fn()))
    const initial = tree.loadRoot()
    await tree.refreshTree()
    old.resolve([file('old')])
    await initial
    expect(tree.treeLoading.value).toBe(false)
    expect(tree.visibleNodes.value.map((node) => node.name)).toEqual(['fresh'])
})

it('keeps an expansion requested while a refresh is in flight', async () => {
    const refresh = deferred<{ name: string; isDirectory: boolean }[]>()
    const read = vi
        .fn()
        .mockResolvedValueOnce([{ name: 'config', isDirectory: true }])
        .mockReturnValueOnce(refresh.promise)
        .mockResolvedValue([file('child')])
    const tree = withinScope(() => useStudioTree(read, vi.fn()))
    await tree.loadRoot()
    const refreshing = tree.refreshTree()
    await tree.toggleDirectory(tree.rootNodes.value[0])
    refresh.resolve([{ name: 'config', isDirectory: true }])
    await refreshing
    expect(tree.visibleNodes.value.map((node) => node.name)).toEqual(['config', 'child'])
})

it('serializes file change batches so older reads cannot overwrite a newer response', async () => {
    vi.useFakeTimers()
    const pending = deferred<void>()
    const services = watcherServices()
    const onChange = vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue(undefined)
    const watcher = withinScope(() => useStudioWatcher(services, onChange, vi.fn()))
    await watcher.start({ kind: 'workspace', path: 'E:/work' })
    const receive = services.watch.mock.calls[0][1]
    receive(['E:/work/config'])
    await vi.advanceTimersByTimeAsync(150)
    receive(['E:/work/config'])
    await vi.advanceTimersByTimeAsync(150)
    expect(onChange).toHaveBeenCalledOnce()
    pending.resolve(undefined)
    await vi.advanceTimersByTimeAsync(0)
    expect(onChange).toHaveBeenCalledTimes(2)
})

it('releases a late old workspace registration without stopping the new watcher', async () => {
    const pending = deferred<() => void>()
    const oldRelease = vi.fn()
    const newRelease = vi.fn()
    const services = watcherServices()
    services.watch.mockReturnValueOnce(pending.promise).mockResolvedValueOnce(newRelease)
    const watcher = withinScope(() => useStudioWatcher(services, vi.fn(), vi.fn()))
    const old = watcher.start({ kind: 'workspace', path: 'E:/old' })
    await vi.waitFor(() => expect(services.watch).toHaveBeenCalledOnce())
    await watcher.start({ kind: 'workspace', path: 'E:/new' })
    pending.resolve(oldRelease)
    await old
    expect(oldRelease).toHaveBeenCalledOnce()
    expect(newRelease).not.toHaveBeenCalled()
    await watcher.stop()
    expect(newRelease).toHaveBeenCalledOnce()
})

it('batches matching instance events and cancels batches and subscriptions on unmount', async () => {
    vi.useFakeTimers()
    const services = watcherServices()
    const unlisten = vi.fn()
    services.listen.mockResolvedValue(unlisten)
    const onChange = vi.fn(async (_paths: string[]) => {})
    const watcher = withinScope(() => useStudioWatcher(services, onChange, vi.fn()))
    await watcher.start({ kind: 'instance', id: 'one' })
    const receive = services.listen.mock.calls[0][0] as (event: {
        instanceId: string
        registrationId: string
        paths: string[]
    }) => void
    receive({ instanceId: 'other', registrationId: 'registration', paths: ['ignored'] })
    receive({ instanceId: 'one', registrationId: 'stale', paths: ['ignored'] })
    receive({ instanceId: 'one', registrationId: 'registration', paths: ['a', 'b'] })
    receive({ instanceId: 'one', registrationId: 'registration', paths: ['a', 'c'] })
    await vi.advanceTimersByTimeAsync(150)
    expect(onChange).toHaveBeenCalledExactlyOnceWith(['a', 'b', 'c'])
    receive({ instanceId: 'one', registrationId: 'registration', paths: ['late'] })
    scopes[0].stop()
    await vi.advanceTimersByTimeAsync(150)
    expect(onChange).toHaveBeenCalledOnce()
    expect(unlisten).toHaveBeenCalledOnce()
    expect(services.unregister).toHaveBeenCalledExactlyOnceWith('one', 'registration')
})

it('releases a pending instance registration after unmount', async () => {
    const pending = deferred<string>()
    const services = watcherServices()
    const unlisten = vi.fn()
    services.listen.mockResolvedValue(unlisten)
    services.register.mockReturnValue(pending.promise)
    const watcher = withinScope(() => useStudioWatcher(services, vi.fn(), vi.fn()))
    const starting = watcher.start({ kind: 'instance', id: 'one' })
    await vi.waitFor(() => expect(services.register).toHaveBeenCalledOnce())
    scopes[0].stop()
    pending.resolve('late')
    await starting
    expect(unlisten).toHaveBeenCalledOnce()
    expect(services.unregister).toHaveBeenCalledExactlyOnceWith('one', 'late')
})

it('releases the event subscription when native registration fails', async () => {
    const services = watcherServices()
    const release = vi.fn()
    const failure = new Error('registration failed')
    services.listen.mockResolvedValue(release)
    services.register.mockRejectedValue(failure)
    const onError = vi.fn()
    const watcher = withinScope(() => useStudioWatcher(services, vi.fn(), onError))
    await watcher.start({ kind: 'instance', id: 'one' })
    expect(release).toHaveBeenCalledOnce()
    expect(onError).toHaveBeenCalledExactlyOnceWith(failure)
    expect(services.unregister).not.toHaveBeenCalled()
})

it('drops queued old-workspace batches and filters unrelated native paths', async () => {
    vi.useFakeTimers()
    const services = watcherServices()
    const pending = deferred<void>()
    const onChange = vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue(undefined)
    const watcher = withinScope(() => useStudioWatcher(services, onChange, vi.fn()))
    await watcher.start({ kind: 'workspace', path: 'E:/old' })
    const old = services.watch.mock.calls[0][1]
    old(['E:\\old\\a', 'E:/outside/file', 'E:/old'])
    await vi.advanceTimersByTimeAsync(150)
    expect(onChange).toHaveBeenCalledExactlyOnceWith(['a'])
    old(['E:/old/b'])
    await vi.advanceTimersByTimeAsync(150)
    await watcher.start({ kind: 'workspace', path: 'E:/new' })
    const fresh = services.watch.mock.calls[1][1]
    old(['E:/old/stale'])
    fresh(['E:/new/current'])
    pending.resolve(undefined)
    await vi.advanceTimersByTimeAsync(150)
    expect(onChange.mock.calls).toEqual([[['a']], [['current']]])
})
