import { afterEach, expect, it, vi } from 'vitest'

import {
    getFavoriteCategoryFilterOpen,
    setFavoriteCategoryFilterOpen,
} from './favorites-filter-state'

afterEach(() => vi.unstubAllGlobals())

it('opens the category filter by default and remembers the last toggle', () => {
    const store = new Map<string, string>()
    vi.stubGlobal('localStorage', {
        getItem: (key: string) => store.get(key) ?? null,
        setItem: (key: string, value: string) => {
            store.set(key, value)
        },
    })

    expect(getFavoriteCategoryFilterOpen()).toBe(true)

    setFavoriteCategoryFilterOpen(false)
    expect(getFavoriteCategoryFilterOpen()).toBe(false)

    setFavoriteCategoryFilterOpen(true)
    expect(getFavoriteCategoryFilterOpen()).toBe(true)
})
