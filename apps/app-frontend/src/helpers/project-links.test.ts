import { expect, it } from 'vitest'

import {
    createProjectBrowseLocation,
    createProjectDetailsQuery,
    getExternalLinkUrl,
} from './project-links.ts'

it('project navigation keeps instance context and browse return location without version filter collisions', () => {
    const query = {
        g: 'categories:neoforge',
        l: '100',
        c: 'search',
        page: '8',
        i: 'instance',
        from: 'worlds',
    }
    const browsePath = '/browse/mod?g=categories:neoforge&page=8'
    expect(createProjectDetailsQuery(query, browsePath)).toEqual({
        i: 'instance',
        from: 'worlds',
        b: browsePath,
    })
    expect(query.g).toBe('categories:neoforge')
    expect(query.page).toBe('8')
})

it('project sidebar loader links target the app browse route and loader filter', () => {
    expect(createProjectBrowseLocation('mod', 'loader', 'forge')).toEqual({
        path: '/browse/mod',
        query: { g: 'categories:forge' },
    })
})

it('project sidebar category links target the app browse route and category filter', () => {
    expect(createProjectBrowseLocation('mod', 'category', 'library-api')).toEqual({
        path: '/browse/mod',
        query: { f: 'categories:library-api' },
    })
})

it('server categories use the server browse route and server category filter', () => {
    expect(createProjectBrowseLocation('minecraft_java_server', 'category', 'vanilla')).toEqual({
        path: '/browse/server',
        query: { sc: 'vanilla' },
    })
})

it('bare external domains are recovered from the Tauri app origin', () => {
    expect(
        getExternalLinkUrl('www.example.com/docs', 'https://tauri.localhost/www.example.com/docs'),
    ).toBe('https://www.example.com/docs')
})

it('CurseForge linkout links are decoded and opened externally', () => {
    const href = '/linkout?remoteUrl=https%253a%252f%252fwww.akliz.net%252fallthemods'
    expect(getExternalLinkUrl(href, `https://tauri.localhost${href}`)).toBe(
        'https://www.akliz.net/allthemods',
    )
})

it('absolute external links are passed through unchanged', () => {
    expect(getExternalLinkUrl('https://example.com/docs', 'https://example.com/docs')).toBe(
        'https://example.com/docs',
    )
})

it('launcher-relative and local links stay in the web view', () => {
    expect(getExternalLinkUrl('/browse/mod', 'https://tauri.localhost/browse/mod')).toBe(null)
    expect(
        getExternalLinkUrl('https://tauri.localhost/settings', 'https://tauri.localhost/settings'),
    ).toBe(null)
    expect(getExternalLinkUrl('http://localhost:8000', 'http://localhost:8000')).toBe(null)
})
