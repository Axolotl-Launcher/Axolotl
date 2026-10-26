import assert from 'node:assert/strict'
import { test } from 'node:test'

import { siteLocaleMessages } from '../locales/site.ts'
import {
    downloadSources,
    getDownloadCatalogUrl,
    getDownloadSource,
    isDownloadSource,
    normalizeDownloadCatalog,
} from './download-sources.ts'

function catalog(overrides: Record<string, unknown> = {}) {
    return {
        version: '1.10.0-beta.5',
        release_tag: 'v1.10.0-beta.5',
        downloads: [
            {
                filename: 'Axolotl x64-setup.exe',
                relative_path: 'dist/1.10.0-beta.5/Axolotl x64-setup.exe',
                sha256: 'a'.repeat(64),
            },
        ],
        ...overrides,
    }
}

test('registered source identities drive preference validation and the recommended default', () => {
    assert.equal(getDownloadSource('auto'), downloadSources[0])
    for (const source of downloadSources) {
        assert(isDownloadSource(source.id))
        assert.equal(getDownloadSource(source.id), source)
        assert(source.label.id && source.description.id)
    }
    for (const value of [null, undefined, '', 'cnb', {}, 'AUTO'])
        assert.equal(isDownloadSource(value), false)
    assert(isDownloadSource('auto'))
})

test('every source uses the same Update Server catalog for either channel', () => {
    assert.equal(
        getDownloadCatalogUrl('release'),
        'https://update.axlmc.org/api/downloads/latest?channel=release',
    )
    assert.equal(
        getDownloadCatalogUrl('beta'),
        'https://update.axlmc.org/api/downloads/latest?channel=beta',
    )
    assert.throws(() => getDownloadCatalogUrl('invalid' as 'release'), /Unknown release channel/)
})

test('normalize the authoritative attachment names, version tag, relative paths and checksums', () => {
    const release = normalizeDownloadCatalog(catalog())!
    assert.equal(release.version, '1.10.0-beta.5')
    assert.equal(release.tag_name, 'v1.10.0-beta.5')
    assert.deepEqual(release.assets, [
        {
            name: 'Axolotl x64-setup.exe',
            relative_path: 'dist/1.10.0-beta.5/Axolotl x64-setup.exe',
            sha256: 'a'.repeat(64),
        },
    ])
    for (const key of ['tag', 'tag_name']) {
        const payload = catalog({ release_tag: undefined, [key]: 'v1.10.0-beta.5' })
        assert.equal(normalizeDownloadCatalog(payload)!.tag_name, 'v1.10.0-beta.5')
    }
})

test('support the deployed catalog shape by reading its authoritative absolute download URL', () => {
    const release = normalizeDownloadCatalog(
        catalog({
            release_tag: undefined,
            downloads: [
                {
                    filename: 'Axolotl x64-setup.exe',
                    url: 'https://update.axlmc.org/dist/1.10.0-beta.5/Axolotl%20x64-setup.exe',
                },
            ],
        }),
    )!
    assert.equal(release.tag_name, 'v1.10.0-beta.5')
    assert.equal(release.assets[0].relative_path, 'dist/1.10.0-beta.5/Axolotl x64-setup.exe')
})

test('registered URL builders use catalog fields and encode each path segment exactly once', () => {
    const release = normalizeDownloadCatalog(catalog())!
    const urls = Object.fromEntries(
        downloadSources.map((source) => [source.id, source.assetUrl(release, release.assets[0])]),
    )
    assert.deepEqual(urls, {
        'update-server': 'https://dist.axlmc.org/dist/1.10.0-beta.5/Axolotl%20x64-setup.exe',
        github: 'https://github.com/Mystic-Stars/Axolotl/releases/download/v1.10.0-beta.5/Axolotl%20x64-setup.exe',
        gitlink:
            'https://www.gitlink.org.cn/xinvxueyuan/Axolotl/releases/download/v1.10.0-beta.5/Axolotl%20x64-setup.exe',
    })
    const nested = normalizeDownloadCatalog(
        catalog({
            downloads: [
                {
                    filename: 'file%name.exe',
                    relative_path: '/dist/1.10.0-beta.5/windows/file%name.exe',
                },
            ],
        }),
    )!
    assert.equal(
        getDownloadSource('update-server').assetUrl(nested, nested.assets[0]),
        'https://dist.axlmc.org/dist/1.10.0-beta.5/windows/file%25name.exe',
    )
})

test('catalog download URLs may already use the official distribution origin', () => {
    const release = normalizeDownloadCatalog(
        catalog({
            downloads: [
                {
                    filename: 'Axolotl x64-setup.exe',
                    url: 'https://dist.axlmc.org/dist/1.10.0-beta.5/Axolotl%20x64-setup.exe',
                },
            ],
        }),
    )!
    assert.equal(release.assets[0].relative_path, 'dist/1.10.0-beta.5/Axolotl x64-setup.exe')
    assert.equal(
        getDownloadSource('update-server').assetUrl(release, release.assets[0]),
        'https://dist.axlmc.org/dist/1.10.0-beta.5/Axolotl%20x64-setup.exe',
    )
    assert.equal(
        getDownloadCatalogUrl('release'),
        'https://update.axlmc.org/api/downloads/latest?channel=release',
    )
})

test('no release and an empty catalog remain distinct from a malformed response', () => {
    for (const value of [undefined, null, '']) assert.equal(normalizeDownloadCatalog(value), null)
    assert.deepEqual(normalizeDownloadCatalog(catalog({ downloads: [] }))!.assets, [])
    for (const value of [
        {},
        [],
        'not json',
        { version: '../outside', downloads: [] },
        catalog({ downloads: null }),
        catalog({ release_tag: 'v1.9.0' }),
    ])
        assert.throws(() => normalizeDownloadCatalog(value))
})

test('reject unsafe or mismatched paths, external URL fallbacks and duplicate attachment names', () => {
    for (const relative_path of [
        'https://example.com/file.exe',
        '//example.com/file.exe',
        '../dist/1.10.0-beta.5/file.exe',
        'dist/1.10.0-beta.5/../file.exe',
        'dist/1.9.0/file.exe',
        'dist/1.10.0-beta.5/other.exe',
        'dist//1.10.0-beta.5/file.exe',
    ]) {
        assert.throws(
            () =>
                normalizeDownloadCatalog(
                    catalog({ downloads: [{ filename: 'file.exe', relative_path }] }),
                ),
            /relative_path/,
        )
    }
    for (const url of [
        'https://evil.example/dist/1.10.0-beta.5/file.exe',
        'javascript:alert(1)',
        'https://update.axlmc.org/dist/1.10.0-beta.5/file.exe?download=1',
        'https://user@update.axlmc.org/dist/1.10.0-beta.5/file.exe',
    ]) {
        assert.throws(() =>
            normalizeDownloadCatalog(catalog({ downloads: [{ filename: 'file.exe', url }] })),
        )
    }
    const download = { filename: 'file.exe', relative_path: 'dist/1.10.0-beta.5/file.exe' }
    assert.throws(
        () => normalizeDownloadCatalog(catalog({ downloads: [download, download] })),
        /duplicate/,
    )
    assert.throws(
        () =>
            normalizeDownloadCatalog(catalog({ downloads: [{ ...download, sha256: 'invalid' }] })),
        /SHA-256/,
    )
    assert.throws(
        () =>
            normalizeDownloadCatalog(
                catalog({
                    downloads: [
                        {
                            filename: '../file.exe',
                            relative_path: 'dist/1.10.0-beta.5/../file.exe',
                        },
                    ],
                }),
            ),
        /attachment name/,
    )
    assert.throws(
        () => normalizeDownloadCatalog(catalog({ downloads: [{ filename: 'file.exe' }] })),
        /relative_path/,
    )
})

test('all registered source labels and descriptions have Chinese translations', () => {
    for (const source of downloadSources) {
        assert(siteLocaleMessages['zh-CN'][source.label.id], source.label.id)
        assert(siteLocaleMessages['zh-CN'][source.description.id], source.description.id)
    }
})
