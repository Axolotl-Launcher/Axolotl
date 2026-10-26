import assert from 'node:assert/strict'
import test from 'node:test'

import { getContentTabMap } from './content-tab-memory.ts'

const originalWindowDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'window')
const originalMemoryDescriptor = Object.getOwnPropertyDescriptor(globalThis, '__ctMemory')

function installBrowserWindow() {
    // `writable` matters: without it the stubbed descriptor survives a thrown
    // assertion instead of being restorable by the cleanup that follows.
    Object.defineProperty(globalThis, 'window', { configurable: true, writable: true, value: {} })
}

function restoreGlobals() {
    if (originalWindowDescriptor) {
        Object.defineProperty(globalThis, 'window', originalWindowDescriptor)
    } else {
        delete (globalThis as { window?: unknown }).window
    }
    if (originalMemoryDescriptor) {
        Object.defineProperty(globalThis, '__ctMemory', originalMemoryDescriptor)
    } else {
        delete (globalThis as { __ctMemory?: unknown }).__ctMemory
    }
}

test('shares one map per namespace while a window exists', (t) => {
    t.after(restoreGlobals)
    installBrowserWindow()

    try {
        const searchMemory = getContentTabMap<string, string>('search')
        searchMemory.set('instance-a', 'toolbars')

        const again = getContentTabMap<string, string>('search')
        assert.equal(again, searchMemory)
        assert.equal(again.get('instance-a'), 'toolbars')

        // Different namespaces stay separate, but live in the same window store.
        assert.notEqual(getContentTabMap<string, string>('sort'), searchMemory)
        const store = (globalThis as { __ctMemory?: Record<string, Map<string, unknown>> })
            .__ctMemory
        assert.equal(store?.search, searchMemory)
    } finally {
        restoreGlobals()
    }
})

test('gives every server render its own store', (t) => {
    t.after(restoreGlobals)
    restoreGlobals()

    try {
        const first = getContentTabMap<string, string>('search')
        first.set('instance-a', 'toolbars')

        const second = getContentTabMap<string, string>('search')
        assert.notEqual(second, first)
        assert.equal(second.get('instance-a'), undefined)

        // Two distinct namespaces within one server render are independent too.
        assert.notEqual(getContentTabMap<string, string>('sort'), first)

        // Nothing is parked on globalThis, so the next request cannot see it.
        assert.equal((globalThis as { __ctMemory?: unknown }).__ctMemory, undefined)
    } finally {
        restoreGlobals()
    }
})

test('a kept server map still reads back what the same render wrote', (t) => {
    t.after(restoreGlobals)
    restoreGlobals()

    try {
        // Callers take the map once and keep it, which is why a server render can
        // write a filter and read it back a few lines later.
        const filterMemory = getContentTabMap<string, { type: string[] }>('filter')
        filterMemory.set('instance-a', { type: ['mod'] })
        assert.deepEqual(filterMemory.get('instance-a'), { type: ['mod'] })

        // Same contract as the isolation test: a server render leaves nothing behind.
        assert.equal((globalThis as { __ctMemory?: unknown }).__ctMemory, undefined)
    } finally {
        restoreGlobals()
    }
})
