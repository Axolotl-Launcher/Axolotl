import assert from 'node:assert/strict'
import { test } from 'node:test'

import { downloadSources, normalizeDownloadCatalog } from './download-sources.ts'
import {
    getDownloadAvailability,
    getReleasePage,
    hasLauncherDownloads,
    resolveLauncherDownloads,
    shouldDegradeDownload,
} from './launcher-downloads.ts'

function release(tag: string, names: string[]) {
    const version = tag.replace(/^v/, '')
    return normalizeDownloadCatalog({
        version,
        release_tag: tag,
        downloads: names.map((filename) => ({
            filename,
            relative_path: 'dist/' + version + '/' + filename,
        })),
    })!
}

test('successful missing platform packages are unavailable rather than still loading', () => {
    const links = resolveLauncherDownloads(
        release('v1.0.0-beta.1', ['Axolotl_universal.dmg']),
        'update-server',
    )
    assert.equal(getDownloadAvailability('success', links.windows), 'unavailable')
    assert.equal(getDownloadAvailability('success', links.mac), 'ready')
    assert.equal(getDownloadAvailability('success', links.appImage), 'unavailable')
    assert.equal(getDownloadAvailability('pending', links.mac), 'loading')
    assert.equal(getDownloadAvailability('idle', links.mac), 'loading')
    assert.equal(getDownloadAvailability('error', links.mac), 'error')
})

test('each source builds links from the exact same release and attachment names', () => {
    const catalog = release('v2.0.0-beta.1', [
        'Axolotl_x64-setup.exe',
        'Axolotl_amd64.AppImage',
        'Axolotl_amd64.deb',
    ])
    assert.equal(
        resolveLauncherDownloads(catalog, 'github').windows,
        'https://github.com/Mystic-Stars/Axolotl/releases/download/v2.0.0-beta.1/Axolotl_x64-setup.exe',
    )
    assert.equal(
        resolveLauncherDownloads(catalog, 'update-server').deb,
        'https://dist.axlmc.org/dist/2.0.0-beta.1/Axolotl_amd64.deb',
    )
    assert.equal(
        resolveLauncherDownloads(catalog, 'gitlink').deb,
        'https://www.gitlink.org.cn/xinvxueyuan/Axolotl/releases/download/v2.0.0-beta.1/Axolotl_amd64.deb',
    )
    for (const source of downloadSources)
        assert.equal(resolveLauncherDownloads(null, source.id).windows, null)
    assert.equal(
        getReleasePage('beta', catalog),
        'https://github.com/Mystic-Stars/Axolotl/releases/tag/v2.0.0-beta.1',
    )
    assert.equal(getReleasePage('beta'), 'https://github.com/Mystic-Stars/Axolotl/releases')
    assert.equal(
        getReleasePage('release'),
        'https://github.com/Mystic-Stars/Axolotl/releases/latest',
    )
})

test('GitLink URLs require no mirror-specific metadata or attachment request', () => {
    const catalog = release('v2.0.0', [
        'Axolotl_2.0.0_x64_nsis-setup.exe',
        'Axolotl_2.0.0_amd64.deb',
    ])
    const links = resolveLauncherDownloads(catalog, 'gitlink')
    assert.equal(
        links.windows,
        'https://www.gitlink.org.cn/xinvxueyuan/Axolotl/releases/download/v2.0.0/Axolotl_2.0.0_x64_nsis-setup.exe',
    )
    assert.equal(
        links.deb,
        'https://www.gitlink.org.cn/xinvxueyuan/Axolotl/releases/download/v2.0.0/Axolotl_2.0.0_amd64.deb',
    )
    assert(hasLauncherDownloads(catalog))
})

test('availability and generated platform links agree, including installer naming variants', () => {
    for (const filename of [
        'Axolotl_x64-setup.exe',
        'Axolotl_x64_nsis-setup.exe',
        'Axolotl_x64_modern-setup.exe',
        'Axolotl_universal.dmg',
        'Axolotl_amd64.AppImage',
        'Axolotl_amd64.deb',
        'Axolotl.x86_64.rpm',
    ]) {
        const catalog = release('v1.10.0', [filename])
        assert(hasLauncherDownloads(catalog), filename)
        for (const source of downloadSources)
            assert(Object.values(resolveLauncherDownloads(catalog, source.id)).some(Boolean))
    }
    for (const names of [[], ['Axolotl_x64-setup.exe.sig'], ['latest.json']])
        assert.equal(hasLauncherDownloads(release('v1.10.0', names)), false)
})

test('a hanging version request degrades the hero call to action', () => {
    // Still waiting: the button keeps saying it is fetching.
    assert.equal(shouldDegradeDownload('loading', 0), false)
    assert.equal(shouldDegradeDownload('loading', 2499), false)

    // The limit itself already counts as waited out, so the button never sits one
    // tick short of usable.
    assert.equal(shouldDegradeDownload('loading', 2500), true)
    assert.equal(shouldDegradeDownload('loading', 60_000), true)
})

test('a settled request never degrades, however long it took', () => {
    // A slow success is still a success: the button should offer the download,
    // not fall back to the options list.
    assert.equal(shouldDegradeDownload('ready', 0), false)
    assert.equal(shouldDegradeDownload('ready', 60_000), false)
    assert.equal(shouldDegradeDownload('error', 60_000), false)
})

test('the degrade limit is configurable for the caller', () => {
    assert.equal(shouldDegradeDownload('loading', 999, 1000), false)
    assert.equal(shouldDegradeDownload('loading', 1000, 1000), true)
})
