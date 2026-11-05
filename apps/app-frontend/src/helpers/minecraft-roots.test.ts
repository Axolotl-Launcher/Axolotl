import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { DirectLinkSyncReport, ExternalMinecraftRoot } from './instance'

const sync = vi.hoisted(() => ({
    calls: [] as ExternalMinecraftRoot[][],
    release: [] as (() => void)[],
}))

vi.mock('./direct-link-sync', () => ({
    syncConfiguredDirectLinks: (roots: ExternalMinecraftRoot[]) => {
        sync.calls.push(roots.map((root) => ({ ...root })))
        return new Promise<DirectLinkSyncReport>((resolve) => {
            sync.release.push(() =>
                resolve({ imported: 1, updated: 0, removed: 0, missing: 0, errors: [] }),
            )
        })
    },
}))

const { addMinecraftRoot, MINECRAFT_ROOTS_STORAGE_KEY, readMinecraftRoots } =
    await import('./minecraft-roots')

const storage = new Map<string, string>()
vi.stubGlobal('localStorage', {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
    removeItem: (key: string) => storage.delete(key),
})

async function settle() {
    for (let turn = 0; turn < 10; turn++) await Promise.resolve()
}

describe('addMinecraftRoot', () => {
    beforeEach(() => {
        storage.clear()
        sync.calls.length = 0
        sync.release.length = 0
    })

    it('keeps both folders when two are added before either finishes', async () => {
        const first = addMinecraftRoot('C:/A/.minecraft')
        const second = addMinecraftRoot('D:/B/.minecraft')
        await settle()
        // The second addition waits for the first instead of reconciling its own snapshot.
        expect(sync.calls).toHaveLength(1)
        sync.release.shift()!()
        await first
        await settle()
        expect(sync.calls[1]!.map((root) => root.path)).toEqual([
            'C:/A/.minecraft',
            'D:/B/.minecraft',
        ])
        sync.release.shift()!()
        await second
        expect(readMinecraftRoots().map((root) => root.path)).toEqual([
            'C:/A/.minecraft',
            'D:/B/.minecraft',
        ])
    })

    it('stores nothing when the report is rejected, and the next addition still runs', async () => {
        const rejected = addMinecraftRoot('C:/A/.minecraft', () => false)
        await settle()
        sync.release.shift()!()
        expect(await rejected).toMatchObject({ added: true, kept: false })
        expect(storage.has(MINECRAFT_ROOTS_STORAGE_KEY)).toBe(false)

        const next = addMinecraftRoot('D:/B/.minecraft')
        await settle()
        sync.release.shift()!()
        expect(await next).toMatchObject({ added: true, kept: true })
        expect(readMinecraftRoots().map((root) => root.path)).toEqual(['D:/B/.minecraft'])
    })

    it('does not add a folder that is already configured under another spelling', async () => {
        storage.set(
            MINECRAFT_ROOTS_STORAGE_KEY,
            JSON.stringify([{ path: 'C:\\A\\.minecraft', mode: 'isolated' }]),
        )
        const again = addMinecraftRoot('c:/a/.minecraft/')
        await settle()
        sync.release.shift()!()
        expect(await again).toMatchObject({ added: false, kept: true })
        expect(readMinecraftRoots()).toEqual([{ path: 'C:\\A\\.minecraft', mode: 'isolated' }])
    })
})
