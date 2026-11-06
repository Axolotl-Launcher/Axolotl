import { expect, it } from 'vitest'

import {
    getLastBrowseContentDisplayMode,
    getLastBrowseContentProjectType,
    setLastBrowseContentDisplayMode,
    setLastBrowseContentProjectType,
} from './browse-display-mode.ts'
import {
    getLastInstanceLibraryDisplayMode,
    getLastLibraryDisplayMode,
    setLastInstanceLibraryDisplayMode,
    setLastLibraryDisplayMode,
} from './library-display-mode.ts'
import { getSidebarExpanded, setSidebarExpanded } from './sidebar-state.ts'

const storageKey = 'axolotl-browse-content-display-mode'
const projectTypeStorageKey = 'axolotl-browse-content-project-type'
const libraryDisplayModeStorageKey = 'axolotl-library-display-mode'
const instanceLibraryDisplayModeStorageKey = 'axolotl-instance-library-display-mode'
const sidebarStorageKey = 'axolotl-right-sidebar-expanded'
const originalStorageDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')

function installMemoryStorage() {
    const values = new Map<string, string>()
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: {
            getItem: (key: string) => values.get(key) ?? null,
            setItem: (key: string, value: string) => values.set(key, value),
        },
    })
    return values
}

function restoreStorage() {
    if (originalStorageDescriptor) {
        Object.defineProperty(globalThis, 'localStorage', originalStorageDescriptor)
    } else {
        delete (globalThis as { localStorage?: Storage }).localStorage
    }
}

it('browse display mode persists valid values and falls back to the list', () => {
    const values = installMemoryStorage()

    try {
        expect(getLastBrowseContentDisplayMode()).toBe('list')

        setLastBrowseContentDisplayMode('compact')
        expect(getLastBrowseContentDisplayMode()).toBe('compact')

        setLastBrowseContentDisplayMode('grid')
        expect(getLastBrowseContentDisplayMode()).toBe('grid')

        values.set(storageKey, 'invalid')
        expect(getLastBrowseContentDisplayMode()).toBe('list')
    } finally {
        restoreStorage()
    }
})

it('browse project type persists content types and rejects non-content routes', () => {
    const values = installMemoryStorage()

    try {
        expect(getLastBrowseContentProjectType()).toBe('modpack')

        setLastBrowseContentProjectType('mod')
        expect(getLastBrowseContentProjectType()).toBe('mod')

        setLastBrowseContentProjectType('world')
        expect(getLastBrowseContentProjectType()).toBe('world')

        values.set(projectTypeStorageKey, 'server')
        expect(getLastBrowseContentProjectType()).toBe('modpack')
    } finally {
        restoreStorage()
    }
})

it('library display mode persists cards and falls back to the standard grid', () => {
    const values = installMemoryStorage()

    try {
        expect(getLastLibraryDisplayMode()).toBe('standard')

        setLastLibraryDisplayMode('cards')
        expect(getLastLibraryDisplayMode()).toBe('cards')

        values.set(libraryDisplayModeStorageKey, 'invalid')
        expect(getLastLibraryDisplayMode()).toBe('standard')
    } finally {
        restoreStorage()
    }
})

it('instance library display mode persists the list and keeps the old preference', () => {
    const values = installMemoryStorage()

    try {
        expect(getLastInstanceLibraryDisplayMode()).toBe('standard')

        values.set(libraryDisplayModeStorageKey, 'cards')
        expect(getLastInstanceLibraryDisplayMode()).toBe('cards')

        setLastInstanceLibraryDisplayMode('list')
        expect(getLastInstanceLibraryDisplayMode()).toBe('list')
        expect(values.get(instanceLibraryDisplayModeStorageKey)).toBe('list')

        setLastInstanceLibraryDisplayMode('standard')
        expect(getLastInstanceLibraryDisplayMode()).toBe('standard')
        expect(values.get(libraryDisplayModeStorageKey)).toBe('cards')

        values.set(instanceLibraryDisplayModeStorageKey, 'invalid')
        expect(getLastInstanceLibraryDisplayMode()).toBe('cards')
    } finally {
        restoreStorage()
    }
})

it('right sidebar expansion persists and defaults to expanded', () => {
    const values = installMemoryStorage()

    try {
        expect(getSidebarExpanded()).toBe(true)

        setSidebarExpanded(false)
        expect(getSidebarExpanded()).toBe(false)

        setSidebarExpanded(true)
        expect(getSidebarExpanded()).toBe(true)

        values.set(sidebarStorageKey, 'invalid')
        expect(getSidebarExpanded()).toBe(true)
    } finally {
        restoreStorage()
    }
})
