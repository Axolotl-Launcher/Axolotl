import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
    getDownloadAvailability,
    getReleasePage,
    resolveLauncherDownloads,
} from './launcher-downloads.ts'

test('a successful response without a platform package is unavailable, not still loading', () => {
    const links = resolveLauncherDownloads(
        { tag_name: 'v1.0-beta', assets: ['Axolotl_universal.dmg'] },
        'update-server',
    )
    assert.equal(getDownloadAvailability('success', links.windows), 'unavailable')
    assert.equal(getDownloadAvailability('success', links.mac), 'ready')
    assert.equal(getDownloadAvailability('success', links.appImage), 'unavailable')
    assert.equal(getDownloadAvailability('pending', links.mac), 'loading')
    assert.equal(getDownloadAvailability('error', links.mac), 'error')
    assert.equal(
        getDownloadAvailability(
            'success',
            resolveLauncherDownloads({ tag_name: 'v1', assets: [] }, 'github').windows,
        ),
        'unavailable',
    )
})

test('download links use only the supplied release and source, and fallback respects the channel', () => {
    const release = {
        tag_name: 'v2.0-beta.1',
        assets: ['Axolotl_x64-setup.exe', 'Axolotl_amd64.AppImage', 'Axolotl_amd64.deb'],
    }
    assert.equal(
        resolveLauncherDownloads(release, 'github').windows,
        'https://github.com/Mystic-Stars/Axolotl/releases/download/v2.0-beta.1/Axolotl_x64-setup.exe',
    )
    assert.equal(resolveLauncherDownloads(null, 'github').windows, null)
    assert.equal(
        resolveLauncherDownloads(release, 'update-server').deb,
        'https://update.axlmc.org/dist/2.0-beta.1/Axolotl_amd64.deb',
    )
    assert.equal(
        getReleasePage('beta', release),
        'https://github.com/Mystic-Stars/Axolotl/releases/tag/v2.0-beta.1',
    )
    assert.equal(getReleasePage('beta'), 'https://github.com/Mystic-Stars/Axolotl/releases')
    assert.equal(
        getReleasePage('release'),
        'https://github.com/Mystic-Stars/Axolotl/releases/latest',
    )
})
