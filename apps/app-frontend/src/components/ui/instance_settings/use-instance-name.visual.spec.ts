import { expect, it, vi } from 'vitest'
import { effectScope, nextTick } from 'vue'

import { useInstanceName } from './use-instance-name'

it('saves the full allowed name and serializes rapid edits so the newest name wins', async () => {
    const scope = effectScope()
    let finish!: () => void
    const save = vi
        .fn()
        .mockImplementationOnce(
            () =>
                new Promise<void>((resolve) => {
                    finish = resolve
                }),
        )
        .mockResolvedValue(undefined)
    const editor = scope.run(() => useInstanceName('serial-test', 'Original', save))!
    try {
        editor.title.value = '中'.repeat(60)
        await nextTick()
        await vi.waitFor(() => expect(save).toHaveBeenCalledExactlyOnceWith('中'.repeat(60)))
        editor.title.value = 'Next'
        await nextTick()
        editor.title.value = 'Latest'
        await nextTick()
        expect(save).toHaveBeenCalledTimes(1)
        finish()
        await editor.saveName()
        expect(save.mock.calls).toEqual([['中'.repeat(60)], ['Latest']])
        expect(editor.failed.value).toBe(false)
    } finally {
        scope.stop()
    }
})

it('preserves a rejected draft for retry and never persists an empty or overlong name', async () => {
    const scope = effectScope()
    const save = vi.fn().mockRejectedValueOnce(new Error('denied')).mockResolvedValue(undefined)
    const editor = scope.run(() => useInstanceName('retry-test', 'Original', save))!
    try {
        editor.title.value = '  My instance  '
        await nextTick()
        await vi.waitFor(() => expect(editor.failed.value).toBe(true))
        expect(save).toHaveBeenCalledTimes(1)
        expect(editor.title.value).toBe('  My instance  ')
        await editor.saveName()
        editor.normalize()
        await nextTick()
        expect(editor.title.value).toBe('My instance')
        expect(editor.failed.value).toBe(false)
        const savedCount = save.mock.calls.length
        editor.title.value = ' '
        await nextTick()
        editor.title.value = 'x'.repeat(81)
        await nextTick()
        expect(editor.invalid.value).toBe(true)
        expect(save).toHaveBeenCalledTimes(savedCount)
    } finally {
        scope.stop()
    }
})

it('keeps write order and the latest draft when the editor is remounted during a save', async () => {
    let finish!: () => void
    let persisted = 'Original'
    const save = vi
        .fn()
        .mockImplementationOnce(
            (name: string) =>
                new Promise<void>((resolve) => {
                    finish = () => {
                        persisted = name
                        resolve()
                    }
                }),
        )
        .mockImplementation(async (name: string) => {
            persisted = name
        })
    const firstScope = effectScope()
    const first = firstScope.run(() => useInstanceName('remount-test', 'Original', save))!
    first.title.value = 'Older draft'
    await nextTick()
    await vi.waitFor(() => expect(finish).toBeDefined())
    firstScope.stop()
    const secondScope = effectScope()
    const second = secondScope.run(() => useInstanceName('remount-test', 'Original', save))!
    try {
        expect(second.title.value).toBe('Older draft')
        second.title.value = 'Newest draft'
        await nextTick()
        finish()
        await second.saveName()
        expect(persisted).toBe('Newest draft')
        expect(second.title.value).toBe('Newest draft')
        expect(second.failed.value).toBe(false)
        expect(second.saving.value).toBe(false)
    } finally {
        secondScope.stop()
    }
})

it('waits for the existing write before a destructive action even when the current draft is invalid', async () => {
    let finish!: () => void
    const scope = effectScope()
    const save = vi.fn(
        () =>
            new Promise<void>((resolve) => {
                finish = resolve
            }),
    )
    const editor = scope.run(() => useInstanceName('flush-test', 'Original', save))!
    try {
        editor.title.value = 'Saving name'
        await nextTick()
        await vi.waitFor(() => expect(finish).toBeDefined())
        editor.title.value = ''
        await nextTick()
        let drained = false
        const flush = editor.saveName().then(() => {
            drained = true
        })
        await nextTick()
        expect(drained).toBe(false)
        finish()
        await flush
        expect(drained).toBe(true)
        expect(save).toHaveBeenCalledOnce()
    } finally {
        scope.stop()
    }
})
