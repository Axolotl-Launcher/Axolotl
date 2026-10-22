import assert from 'node:assert/strict'
import test from 'node:test'

import { ref } from 'vue'

import { deletePendingItems, disablePendingItems } from './content-deletion.ts'

test('partial deletion retries only failed and untouched entries', async () => {
    const pending = ref(['a', 'b', 'c'])
    const calls: string[] = [],
        completed: string[] = []
    let fail = true
    const remove = async (id: string) => {
        calls.push(id)
        if (id === 'b' && fail) throw new Error('delete failed')
    }
    await assert.rejects(
        deletePendingItems(
            pending,
            remove,
            (id) => id,
            (id) => completed.push(id),
        ),
    )
    assert.deepEqual(pending.value, ['b', 'c'])
    fail = false
    await deletePendingItems(
        pending,
        remove,
        (id) => id,
        (id) => completed.push(id),
    )
    assert.deepEqual(calls, ['a', 'b', 'b', 'c'])
    assert.deepEqual(completed, ['a', 'b', 'c'])
})

test('unsuccessful dependent disabling retains the targets for confirmation retry', async () => {
    const pending = ref(['dependent'])
    await assert.rejects(
        disablePendingItems(
            pending,
            async () => false,
            () => new Error('disable failed'),
        ),
    )
    assert.deepEqual(pending.value, ['dependent'])
    await assert.rejects(
        disablePendingItems(
            pending,
            async () => {
                throw new Error('IPC failed')
            },
            () => new Error('disable failed'),
        ),
    )
    assert.deepEqual(pending.value, ['dependent'])
    await disablePendingItems(
        pending,
        async () => true,
        () => new Error('unused'),
    )
    assert.deepEqual(pending.value, [])
})
