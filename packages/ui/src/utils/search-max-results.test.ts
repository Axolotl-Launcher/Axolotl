import assert from 'node:assert/strict'
import test from 'node:test'

import {
    MAX_RESULTS_DEFAULT,
    MAX_RESULTS_STORAGE_KEY,
    persistMaxResults,
    readPersistedMaxResults,
} from './browse-max-results.ts'

const originalStorageDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')

function installStorage(store: Map<string, string>): void {
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: {
            getItem: (key: string) => store.get(key) ?? null,
            setItem: (key: string, value: string) => store.set(key, value),
            removeItem: (key: string) => store.delete(key),
        },
    })
}

function restoreStorage(): void {
    if (originalStorageDescriptor) {
        Object.defineProperty(globalThis, 'localStorage', originalStorageDescriptor)
    } else {
        delete (globalThis as { localStorage?: Storage }).localStorage
    }
}

test('returns the default when nothing is stored', () => {
    installStorage(new Map())
    try {
        assert.equal(readPersistedMaxResults(), MAX_RESULTS_DEFAULT)
    } finally {
        restoreStorage()
    }
})

test('round-trips a chosen page size', () => {
    const store = new Map<string, string>()
    installStorage(store)
    try {
        persistMaxResults(100)
        assert.equal(store.get(MAX_RESULTS_STORAGE_KEY), '100')
        assert.equal(readPersistedMaxResults(), 100)
    } finally {
        restoreStorage()
    }
})

test('falls back to the default for unusable stored values', () => {
    for (const raw of ['', 'abc', '0', '-5', 'NaN']) {
        installStorage(new Map([[MAX_RESULTS_STORAGE_KEY, raw]]))
        try {
            assert.equal(
                readPersistedMaxResults(),
                MAX_RESULTS_DEFAULT,
                `unexpected result for ${JSON.stringify(raw)}`,
            )
        } finally {
            restoreStorage()
        }
    }
})

test('keeps a leading number even when the entry is partly malformed', () => {
    installStorage(new Map([[MAX_RESULTS_STORAGE_KEY, '12.5x']]))
    try {
        assert.equal(readPersistedMaxResults(), 12)
    } finally {
        restoreStorage()
    }
})

test('survives storage that throws', () => {
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: {
            getItem: () => {
                throw new Error('blocked')
            },
            setItem: () => {
                throw new Error('quota exceeded')
            },
        },
    })
    try {
        assert.equal(readPersistedMaxResults(), MAX_RESULTS_DEFAULT)
        assert.doesNotThrow(() => persistMaxResults(50))
    } finally {
        restoreStorage()
    }
})

test('works without any storage at all', () => {
    restoreStorage()
    assert.equal(readPersistedMaxResults(), MAX_RESULTS_DEFAULT)
    assert.doesNotThrow(() => persistMaxResults(50))
})
