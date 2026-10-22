import assert from 'node:assert/strict'
import test from 'node:test'

import { resolveDownloadUrl, restoreCachedUrls } from './download-url.ts'

test('Tianpao historical download and image paths recover their official origins', () => {
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
        assert.equal(resolveDownloadUrl('https://mod.tianpao.top' + path), target)
        assert.equal(resolveDownloadUrl('HTTPS://MOD.TIANPAO.TOP:443' + path), target)
    }
    assert.equal(
        resolveDownloadUrl('https://cdn-alt.modrinth.com/data/file.jar'),
        'https://cdn.modrinth.com/data/file.jar',
    )
})

test('Tianpao unknown or unsafe links are not opened and unrelated hosts stay intact', () => {
    for (const input of [
        'https://mod.tianpao.top/unknown',
        'https://mod.tianpao.top/data',
        'http://mod.tianpao.top/data/file',
        'https://mod.tianpao.top:8443/files/file',
        'https://user@mod.tianpao.top/data/file',
        'https://mod.tianpao.top./data/file',
        'https://mod.tianpao.top/data/../unknown',
    ])
        assert.equal(resolveDownloadUrl(input), undefined)
    for (const input of [
        'https://mod.tianpao.top.evil.example/data/file',
        'https://mod.tianpao.top@evil.example/file',
        'https://cdn.modrinth.com/data/file',
    ])
        assert.equal(resolveDownloadUrl(input), input)
})

test('cached project images and file URLs recover on read without mutating history or prose', () => {
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
    assert.equal(restored.icon_url, 'https://media.forgecdn.net/attachments/icon.png')
    assert.equal(restored.gallery[0].url, 'https://media.forgecdn.net/screenshots/a.png')
    assert.equal(restored.gallery[0].raw_url, '')
    assert.equal(restored.search.gallery[0], 'https://media.forgecdn.net/screenshots/b.png')
    assert.equal(restored.files[0].url, 'https://cdn.modrinth.com/data/file.jar')
    assert.equal(restored.body, original.body)
    assert.equal(restored.source, 'tianpao')
    assert.equal(original.icon_url, 'https://mod.tianpao.top/media/attachments/icon.png')
})
