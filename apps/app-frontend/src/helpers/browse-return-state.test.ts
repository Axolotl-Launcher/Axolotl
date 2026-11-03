import { expect, it } from 'vitest'

import {
    clearBrowseReturnSnapshot,
    completeBrowseReturnNavigation,
    consumeBrowseReturnSnapshot,
    hasBrowseReturnSnapshot,
    isBrowseReturnNavigation,
    isBrowseReturnSourcePath,
    prepareBrowseReturnNavigation,
    saveBrowseReturnSnapshot,
} from './browse-return-state.ts'

it('consumes a matching browse snapshot only once', () => {
    const url = '/browse/mod?m=100&o=100'
    saveBrowseReturnSnapshot({ url, scrollTop: 480, state: { currentPage: 2, hits: ['a'] } })
    expect(prepareBrowseReturnNavigation(url, '/project/sodium')).toBe(true)

    expect(consumeBrowseReturnSnapshot(url)).toEqual({
        url,
        scrollTop: 480,
        state: { currentPage: 2, hits: ['a'] },
    })
    expect(consumeBrowseReturnSnapshot(url)).toBe(null)
    expect(isBrowseReturnNavigation(url)).toBe(true)
    completeBrowseReturnNavigation(url)
    expect(isBrowseReturnNavigation(url)).toBe(false)
})

it('does not consume a snapshot for a different browse URL', () => {
    saveBrowseReturnSnapshot({ url: '/browse/mod?page=2', scrollTop: 480, state: {} })

    expect(consumeBrowseReturnSnapshot('/browse/mod?page=3')).toBe(null)
    expect(hasBrowseReturnSnapshot('/browse/mod?page=2')).toBe(true)
    clearBrowseReturnSnapshot()
})

it('consumes a matching snapshot without a route guard marker', () => {
    const url = '/browse/mod?source=modrinth'
    saveBrowseReturnSnapshot({ url, scrollTop: 480, state: {} })

    expect(consumeBrowseReturnSnapshot(url)).toEqual({ url, scrollTop: 480, state: {} })
})

it('clears snapshots for ordinary Browse navigation', () => {
    const url = '/browse/mod?page=2'
    saveBrowseReturnSnapshot({ url, scrollTop: 480, state: {} })

    expect(prepareBrowseReturnNavigation(url, '/library')).toBe(false)
    expect(hasBrowseReturnSnapshot(url)).toBe(false)
})

it('recognizes only project, download, and instance return routes', () => {
    expect(isBrowseReturnSourcePath('/project/sodium')).toBe(true)
    expect(isBrowseReturnSourcePath('/project/sodium/versions')).toBe(true)
    expect(isBrowseReturnSourcePath('/downloads')).toBe(true)
    expect(isBrowseReturnSourcePath('/instance/example')).toBe(true)
    expect(isBrowseReturnSourcePath('/library')).toBe(false)
})
