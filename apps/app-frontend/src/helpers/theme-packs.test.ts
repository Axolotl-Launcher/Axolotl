import { describe, expect, it, vi } from 'vitest'

import {
    createThemePackPagesRefresher,
    themePackAssetUrl,
    type ThemePackNavPage,
    themePackOrigin,
    themePackPort,
} from './theme-packs'

describe('themePackPort', () => {
    it('matches the Rust protocol port algorithm', () => {
        expect(themePackPort('demo')).toBe(49814)
    })
})

describe('themePackAssetUrl', () => {
    it('escapes characters that could break out of a CSS url() string', () => {
        expect(themePackAssetUrl('demo', "assets/it's).css")).toBe(
            `${themePackOrigin('demo')}/demo/assets/it%27s%29.css`,
        )
    })

    it('keeps nested asset paths readable', () => {
        expect(themePackAssetUrl('demo', 'assets/fonts/ui font.woff2')).toBe(
            `${themePackOrigin('demo')}/demo/assets/fonts/ui%20font.woff2`,
        )
    })
})

function deferred<T>() {
    let resolve!: (value: T) => void
    const promise = new Promise<T>((done) => {
        resolve = done
    })
    return { promise, resolve }
}

const page: ThemePackNavPage = {
    packId: 'demo',
    pageId: 'home',
    title: 'Home',
    iconUrl: null,
}

describe('createThemePackPagesRefresher', () => {
    it('publishes only the newest load result', async () => {
        const firstSource = deferred<string | null>()
        let reads = 0
        const readSource = vi.fn(async () => (++reads === 1 ? firstSource.promise : 'pack:demo'))
        const listPages = vi.fn(async () => [page])
        const refresher = createThemePackPagesRefresher(readSource, () => 'en-US', listPages)
        const firstApply = vi.fn()
        const secondApply = vi.fn()

        const first = refresher.refresh(firstApply)
        await refresher.refresh(secondApply)

        firstSource.resolve('builtin')
        await first

        expect(firstApply).not.toHaveBeenCalled()
        expect(secondApply).toHaveBeenCalledWith([page])
    })

    it('ignores a load that finishes after invalidation', async () => {
        const source = deferred<string | null>()
        const refresher = createThemePackPagesRefresher(
            () => source.promise,
            () => 'en-US',
            vi.fn(async () => [page]),
        )
        const apply = vi.fn()

        const refresh = refresher.refresh(apply)
        refresher.invalidate()
        source.resolve('pack:demo')
        await refresh

        expect(apply).not.toHaveBeenCalled()
    })
})
