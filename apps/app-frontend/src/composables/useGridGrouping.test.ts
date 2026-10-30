import { expect, it } from 'vitest'
import { effectScope, ref } from 'vue'

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
