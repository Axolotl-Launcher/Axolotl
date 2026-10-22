import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import type { ContentItem } from '../types'
import { useBulkOperation } from './bulk-operations'
import { useContentSelection } from './content-selection'

it('filtered bulk selections address only matching actual entries and preserve choices on return', () => {
    const a = { id: 'a' } as ContentItem,
        b = { id: 'b' } as ContentItem
    const items = ref([a, b]),
        visible = ref([a, b])
    const selection = useContentSelection(items, (item) => item.id, visible)
    selection.selectedIds.value = ['a', 'b']
    visible.value = [a]
    expect(selection.selectedItems.value.map((item) => item.id)).toEqual(['a'])
    visible.value = [a, b]
    expect(selection.selectedItems.value.map((item) => item.id)).toEqual(['a', 'b'])
})

it('cancelled and failed bulk previews release busy state without running success cleanup', async () => {
    let bulk!: ReturnType<typeof useBulkOperation>
    const wrapper = mount(
        defineComponent({
            setup: () => {
                bulk = useBulkOperation()
                return () => h('div')
            },
        }),
    )
    try {
        let completed = 0
        const complete = () => {
            completed++
        }
        await bulk.runBulkWithWaiting('update', 2, async () => false, complete)
        expect(completed).toBe(0)
        expect(bulk.isBulkOperating.value).toBe(false)
        await expect(
            bulk.runBulkWithWaiting(
                'update',
                2,
                async () => {
                    throw new Error('queue rejected')
                },
                complete,
            ),
        ).rejects.toThrow('queue rejected')
        expect(completed).toBe(0)
        expect(bulk.isBulkOperating.value).toBe(false)
        await bulk.runBulkWithWaiting('update', 2, async () => true, complete)
        expect(completed).toBe(1)
    } finally {
        wrapper.unmount()
    }
})
