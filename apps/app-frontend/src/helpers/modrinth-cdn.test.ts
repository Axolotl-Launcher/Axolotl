import assert from 'node:assert/strict'
import test from 'node:test'

import { normalizeModrinthCdnUrl } from './modrinth-cdn.ts'

test('saved CDN links retain exact encoding and migrate to the current host', () => {
    const suffix = '/data/%e9%87%91/file+name%2B.jar?token=a%2bb&x=1&x=2#fragment'
    for (const prefix of ['https://cdn-alt.modrinth.com', 'HTTPS://CDN-ALT.MODRINTH.COM:443']) {
        assert.equal(
            normalizeModrinthCdnUrl(prefix + suffix),
            prefix.split('://')[0] + '://cdn.modrinth.com' + suffix,
        )
    }
    assert.equal(
        normalizeModrinthCdnUrl('https://cdn-alt.modrinth.com'),
        'https://cdn.modrinth.com',
    )
})

test('current links and other authorities are left unchanged', () => {
    for (const input of [
        'https://cdn.modrinth.com/file.jar?x=%2b',
        'https://cdn-raw.modrinth.com/file.png',
        'http://cdn-alt.modrinth.com/file.jar',
        'https://cdn-alt.modrinth.com:8443/file.jar',
        'https://user:secret@cdn-alt.modrinth.com/file.jar',
        'https://cdn-alt.modrinth.com@evil.example/file.jar',
        'https://cdn-alt.modrinth.com.evil.example/file.jar',
        'https://evil.example/cdn-alt.modrinth.com',
        'invalid URL',
    ])
        assert.equal(normalizeModrinthCdnUrl(input), input)
})
