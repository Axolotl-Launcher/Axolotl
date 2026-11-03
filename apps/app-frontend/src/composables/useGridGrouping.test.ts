import { expect, it } from 'vitest'
import { effectScope, nextTick, ref } from 'vue'

import { resolveGameDirectory } from '@/helpers/game-directory'
import type { GameInstance } from '@/helpers/types'

import { UNGROUPED_GROUP_KEY, useGridGrouping } from './useGridGrouping.ts'

it('renders an instance in every assigned group including favorites', () => {
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: { getItem: () => null, setItem: () => {}, removeItem: () => {} },
    })
    const scope = effectScope()
    try {
        scope.run(() => {
            const shared = {
                id: 'shared',
                name: 'Shared',
                groups: ['a', 'group:favorites', 'b', 'a'],
            }
            const ungrouped = { id: 'ungrouped', name: 'Ungrouped', groups: [] }
            const instances = ref([shared, ungrouped])
            const { filteredResults } = useGridGrouping('membership-test', instances)
            for (const id of ['a', 'b', 'group:favorites']) {
                expect(filteredResults.value.get(id)?.map((instance) => instance.id)).toEqual([
                    'shared',
                ])
            }
            expect(
                filteredResults.value.get(UNGROUPED_GROUP_KEY)?.map((instance) => instance.id),
            ).toEqual(['ungrouped'])
            instances.value[0].groups = ['b', 'group:favorites']
            expect(filteredResults.value.has('a')).toBe(false)
            expect(filteredResults.value.get('b')?.length).toBe(1)
            expect(filteredResults.value.get('group:favorites')?.length).toBe(1)
        })
    } finally {
        scope.stop()
        if (descriptor) Object.defineProperty(globalThis, 'localStorage', descriptor)
        else delete (globalThis as { localStorage?: Storage }).localStorage
    }
})

it('groups by game directory and persists collapsed sections across instances', async () => {
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')
    const values = new Map<string, string>()
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: {
            getItem: (key: string) => values.get(key) ?? null,
            setItem: (key: string, value: string) => values.set(key, value),
            removeItem: (key: string) => values.delete(key),
        },
    })
    const scope = effectScope()

    try {
        scope.run(() => {
            const instances = ref<GameInstance[]>([
                {
                    id: 'external-b',
                    name: 'External B',
                    groups: [],
                    game_dir_override: 'D:/Minecraft/.minecraft',
                },
                {
                    id: 'managed',
                    name: 'Managed',
                    groups: [],
                },
                {
                    id: 'external-a',
                    name: 'External A',
                    groups: [],
                    game_dir_override: 'C:/Games/.minecraft/versions/1.21',
                },
            ] as GameInstance[])
            const { state, filteredResults, isSectionCollapsed, setSectionCollapsed } =
                useGridGrouping('game-directory-test', instances, {
                    getGameDirectory: resolveGameDirectory,
                })

            state.value.group = 'Game directory'

            expect([...filteredResults.value.keys()]).toEqual([
                'managed',
                'external:c:/games/.minecraft',
                'external:d:/minecraft/.minecraft',
            ])
            expect(filteredResults.value.get('managed')?.map((instance) => instance.id)).toEqual([
                'managed',
            ])
            expect(
                filteredResults.value
                    .get('external:c:/games/.minecraft')
                    ?.map((instance) => instance.id),
            ).toEqual(['external-a'])

            setSectionCollapsed('managed', true)
            expect(isSectionCollapsed('managed')).toBe(true)

            instances.value = instances.value.filter((instance) => instance.id === 'external-a')
            expect([...filteredResults.value.keys()]).toEqual(['external:c:/games/.minecraft'])
        })
        await nextTick()
        await nextTick()
        scope.stop()

        const restoredScope = effectScope()
        try {
            restoredScope.run(() => {
                const instances = ref<GameInstance[]>([])
                const { state, isSectionCollapsed } = useGridGrouping(
                    'game-directory-test',
                    instances,
                    { getGameDirectory: resolveGameDirectory },
                )
                expect(state.value.group).toBe('Game directory')
                expect(isSectionCollapsed('managed')).toBe(true)
            })
        } finally {
            restoredScope.stop()
        }
    } finally {
        scope.stop()
        if (descriptor) Object.defineProperty(globalThis, 'localStorage', descriptor)
        else delete (globalThis as { localStorage?: Storage }).localStorage
    }
})
