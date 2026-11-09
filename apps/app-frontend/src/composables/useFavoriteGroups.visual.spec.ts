import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'

import { useFavoriteGroups } from './useFavoriteGroups'

const fixture = vi.hoisted(() => ({
    list: vi.fn(),
    create: vi.fn(),
    rename: vi.fn(),
    remove: vi.fn(),
    update: vi.fn(),
    listener: null as null | (() => void),
    handleError: vi.fn(),
}))

vi.mock('@modrinth/ui', () => ({
    injectNotificationManager: () => ({ handleError: fixture.handleError }),
}))
vi.mock('@/helpers/events', () => ({
    favorite_groups_listener: async (callback: () => void) => {
        fixture.listener = callback
        return () => {
            fixture.listener = null
        }
    },
}))
vi.mock('@/helpers/favorite-groups', () => ({
    list_favorite_groups: fixture.list,
    create_favorite_group: fixture.create,
    rename_favorite_group: fixture.rename,
    delete_favorite_group: fixture.remove,
    update_favorite_group_memberships: fixture.update,
}))

const scopes: ReturnType<typeof effectScope>[] = []

function setup() {
    const scope = effectScope()
    const state = scope.run(() => useFavoriteGroups())!
    scopes.push(scope)
    return state
}

afterEach(() => {
    scopes.splice(0).forEach((scope) => scope.stop())
    vi.clearAllMocks()
    fixture.listener = null
})

const snapshot = {
    groups: [
        { id: 'a', name: 'Alpha' },
        { id: 'b', name: 'Beta' },
    ],
    memberships: [{ favoriteKey: 'modrinth:one', groupIds: ['a'] }],
}

it('loads groups and memberships and maps membership by favorite key', async () => {
    fixture.list.mockResolvedValue(snapshot)
    const state = setup()
    await flushPromises()

    expect(state.groups.value).toEqual(snapshot.groups)
    expect(state.loaded.value).toBe(true)
    expect(state.groupsFor('modrinth:one')).toEqual(['a'])
    expect(state.isInGroup('modrinth:one', 'a')).toBe(true)
    expect(state.isInGroup('modrinth:one', 'b')).toBe(false)
    expect(state.groupsFor('modrinth:missing')).toEqual([])
})

it('refreshes when the backend reports a group change', async () => {
    fixture.list.mockResolvedValue(snapshot)
    const state = setup()
    await flushPromises()

    fixture.list.mockResolvedValue({
        groups: [{ id: 'b', name: 'Beta' }],
        memberships: [{ favoriteKey: 'modrinth:one', groupIds: ['b'] }],
    })
    fixture.listener?.()
    await flushPromises()

    expect(state.groups.value).toEqual([{ id: 'b', name: 'Beta' }])
    expect(state.groupsFor('modrinth:one')).toEqual(['b'])
})

it('applies membership changes optimistically and sends add/remove ids', async () => {
    fixture.list.mockResolvedValue(snapshot)
    fixture.update.mockResolvedValue(undefined)
    const state = setup()
    await flushPromises()

    await state.updateFavoriteGroups('modrinth:one', ['b'], ['a'])

    expect(state.groupsFor('modrinth:one')).toEqual(['b'])
    expect(fixture.update).toHaveBeenCalledWith([
        { favoriteKey: 'modrinth:one', addGroupIds: ['b'], removeGroupIds: ['a'] },
    ])
})

it('rolls back an optimistic membership change when the write fails', async () => {
    fixture.list.mockResolvedValue(snapshot)
    fixture.update.mockRejectedValue(new Error('write failed'))
    const state = setup()
    await flushPromises()

    await expect(state.updateFavoriteGroups('modrinth:one', ['b'], [])).rejects.toThrow(
        'write failed',
    )

    expect(state.groupsFor('modrinth:one')).toEqual(['a'])
    expect(fixture.handleError).toHaveBeenCalledTimes(1)
})

it('creates, renames, and deletes groups locally after the backend accepts', async () => {
    fixture.list.mockResolvedValue(snapshot)
    fixture.create.mockResolvedValue({ id: 'c', name: 'Gamma' })
    fixture.rename.mockResolvedValue({ id: 'a', name: 'Alpha renamed' })
    fixture.remove.mockResolvedValue(undefined)
    const state = setup()
    await flushPromises()

    await state.createGroup('Gamma')
    expect(state.groups.value[0]).toEqual({ id: 'c', name: 'Gamma' })

    await state.renameGroup('a', 'Alpha renamed')
    expect(state.groups.value.find((group) => group.id === 'a')?.name).toBe('Alpha renamed')

    await state.deleteGroup('a')
    expect(state.groups.value.map((group) => group.id)).toEqual(['c', 'b'])
    expect(state.groupsFor('modrinth:one')).toEqual([])
})
