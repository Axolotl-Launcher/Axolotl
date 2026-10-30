import { expect, it } from 'vitest'

import { resolveDownloadUrl, restoreCachedUrls } from './download-url.ts'

it('Tianpao historical download and image paths recover their official origins', () => {
    for (const [path, target] of [
        [
            '/data/a%2B+%e9%87%91.jar?x=%2b#f',
            'https://cdn.modrinth.com/data/a%2B+%e9%87%91.jar?x=%2b#f',
        ],
        ['/files/1/2/a%20b.jar', 'https://edge.forgecdn.net/files/1/2/a%20b.jar'],
        [
            '/media/attachments/image.png?x=1',
            'https://media.forgecdn.net/attachments/image.png?x=1',
        ],
    ]) {
        expect(resolveDownloadUrl('https://mod.tianpao.top' + path)).toBe(target)
        expect(resolveDownloadUrl('HTTPS://MOD.TIANPAO.TOP:443' + path)).toBe(target)
    }
    expect(resolveDownloadUrl('https://cdn-alt.modrinth.com/data/file.jar')).toBe(
        'https://cdn.modrinth.com/data/file.jar',
    )
})

it('Tianpao unknown or unsafe links are not opened and unrelated hosts stay intact', () => {
    for (const input of [
        'https://mod.tianpao.top/unknown',
        'https://mod.tianpao.top/data',
        'http://mod.tianpao.top/data/file',
        'https://mod.tianpao.top:8443/files/file',
        'https://user@mod.tianpao.top/data/file',
        'https://mod.tianpao.top./data/file',
        'https://mod.tianpao.top/data/../unknown',
    ])
        expect(resolveDownloadUrl(input)).toBe(undefined)
    for (const input of [
        'https://mod.tianpao.top.evil.example/data/file',
        'https://mod.tianpao.top@evil.example/file',
        'https://cdn.modrinth.com/data/file',
    ])
        expect(resolveDownloadUrl(input)).toBe(input)
})

it('cached project images and file URLs recover on read without mutating history or prose', () => {
    const original = {
        icon_url: 'https://mod.tianpao.top/media/attachments/icon.png',
        gallery: [
            {
                url: 'https://mod.tianpao.top/media/screenshots/a.png',
                raw_url: 'https://mod.tianpao.top/unknown',
            },
        ],
        search: { gallery: ['https://mod.tianpao.top/media/screenshots/b.png'] },
        files: [{ url: 'https://mod.tianpao.top/data/file.jar' }],
        body: 'https://mod.tianpao.top/media/attachments/icon.png',
        source: 'tianpao',
    }
    const restored = restoreCachedUrls(original)
    expect(restored.icon_url).toBe('https://media.forgecdn.net/attachments/icon.png')
    expect(restored.gallery[0].url).toBe('https://media.forgecdn.net/screenshots/a.png')
    expect(restored.gallery[0].raw_url).toBe('')
    expect(restored.search.gallery[0]).toBe('https://media.forgecdn.net/screenshots/b.png')
    expect(restored.files[0].url).toBe('https://cdn.modrinth.com/data/file.jar')
    expect(restored.body).toBe(original.body)
    expect(restored.source).toBe('tianpao')
    expect(original.icon_url).toBe('https://mod.tianpao.top/media/attachments/icon.png')
})
