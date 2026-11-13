import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { effectScope, ref } from 'vue'

import { useInstanceGroups } from './useInstanceGroups'

const backend = vi.hoisted(() => ({ listen: vi.fn(), list: vi.fn() }))
vi.mock('@/helpers/events', () => ({ instance_groups_listener: backend.listen }))
vi.mock('@modrinth/ui', () => ({ injectNotificationManager: () => ({ handleError: vi.fn() }) }))
vi.mock('@/helpers/instance-groups', () => ({
    list_groups: backend.list,
    create_group: vi.fn(),
    delete_group: vi.fn(),
    rename_group: vi.fn(),
    set_group_order: vi.fn(),
    update_group_memberships: vi.fn(),
    MAX_INSTANCE_GROUP_NAME_LENGTH: 100,
}))

const scopes: ReturnType<typeof effectScope>[] = []
afterEach(() => {
    scopes.splice(0).forEach((scope) => scope.stop())
    vi.resetAllMocks()
})

function start() {
    const scope = effectScope()
    scopes.push(scope)
    const groups = scope.run(() => useInstanceGroups(ref([])))!
    return { scope, groups }
}

it('keeps only live scope listeners after repeated mounts', async () => {
    const listeners = new Set<() => void>()
    backend.list.mockResolvedValue([])
    backend.listen.mockImplementation(async (callback: () => void) => {
        listeners.add(callback)
        return () => listeners.delete(callback)
    })
    for (let i = 0; i < 3; i++) {
        const { scope } = start()
        await flushPromises()
        expect(listeners.size).toBe(1)
        scope.stop()
        expect(listeners.size).toBe(0)
    }
    start()
    await flushPromises()
    const before = backend.list.mock.calls.length
    listeners.forEach((callback) => callback())
    await flushPromises()
    expect(backend.list.mock.calls.length).toBe(before + 1)
})

it('publishes empty groups returned by a group-change refresh', async () => {
    let refresh!: () => void
    backend.list.mockResolvedValueOnce([])
    backend.list.mockResolvedValueOnce([{ id: 'empty', name: 'Empty' }])
    backend.listen.mockImplementation(async (callback: () => void) => {
        refresh = callback
        return () => {}
    })
    const { groups } = start()
    await flushPromises()
    expect(groups.libraryGroups.value).toEqual([])

    refresh()
    await flushPromises()

    expect(groups.libraryGroups.value).toEqual([{ id: 'empty', name: 'Empty' }])
})

it('ignores a stale group read that resolves after a newer one', async () => {
    let resolveInitial!: (groups: { id: string; name: string }[]) => void
    let resolveRefresh!: (groups: { id: string; name: string }[]) => void
    let refresh!: () => void
    backend.list.mockImplementationOnce(() => new Promise((resolve) => (resolveInitial = resolve)))
    backend.list.mockImplementationOnce(() => new Promise((resolve) => (resolveRefresh = resolve)))
    backend.listen.mockImplementation(async (callback: () => void) => {
        refresh = callback
        return () => {}
    })
    const { groups } = start()
    await flushPromises()

    refresh()
    await flushPromises()
    resolveRefresh([{ id: 'new', name: 'New' }])
    await flushPromises()
    expect(groups.libraryGroups.value).toEqual([{ id: 'new', name: 'New' }])

    resolveInitial([])
    await flushPromises()
    expect(groups.libraryGroups.value).toEqual([{ id: 'new', name: 'New' }])
    expect(groups.libraryGroupsLoaded.value).toBe(true)
})

it('releases a subscription that finishes registering after disposal', async () => {
    let finish!: (cleanup: () => void) => void
    const cleanup = vi.fn()
    backend.list.mockResolvedValue([])
    backend.listen.mockReturnValue(new Promise((resolve) => (finish = resolve)))
    const { scope } = start()
    scope.stop()
    finish(cleanup)
    await flushPromises()
    expect(cleanup).toHaveBeenCalledOnce()
})

it('does not publish an in-flight read into a disposed scope', async () => {
    let finish!: (groups: { id: string; name: string }[]) => void
    backend.list.mockReturnValue(new Promise((resolve) => (finish = resolve)))
    backend.listen.mockResolvedValue(() => {})
    const { scope, groups } = start()
    scope.stop()
    finish([{ id: 'a', name: 'A' }])
    await flushPromises()
    expect(groups.libraryGroups.value).toEqual([])
    expect(groups.libraryGroupsLoaded.value).toBe(false)
})
