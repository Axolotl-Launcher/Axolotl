import fs from 'node:fs'

import { updaterTargets } from './create-update-manifest.mjs'
import { isMain, releaseVersion, verifyAssetUrl } from './release-utils.mjs'

export function verifyUpdateManifest(manifest, tag, source = 'github') {
    const expectedVersion = releaseVersion(tag)
    if (manifest.version !== expectedVersion) {
        throw new Error(`Manifest version ${manifest.version} does not match ${expectedVersion}`)
    }
    for (const target of updaterTargets) {
        for (const platform of target.platforms) {
            const update = manifest.platforms?.[platform]
            if (
                !update ||
                typeof update.signature !== 'string' ||
                update.signature.trim().length < 32
            ) {
                throw new Error(`Missing signed update for ${platform}`)
            }
            // Manifests published before SHA-256 support omit the field and must stay verifiable.
            if (update.sha256 !== undefined && !/^[a-f0-9]{64}$/.test(update.sha256)) {
                throw new Error(`Invalid SHA-256 digest for ${platform}: ${update.sha256}`)
            }
            const filename = decodeURIComponent(new URL(update.url).pathname.split('/').at(-1))
            verifyAssetUrl(update.url, tag, filename, source)
            if (!filename.endsWith(target.assetSuffix)) {
                throw new Error(`Unexpected updater artifact for ${platform}: ${filename}`)
            }
        }
    }
}

if (isMain(import.meta.url)) {
    const [manifestPath, tag, source = 'github'] = process.argv.slice(2)
    if (!manifestPath || !tag || !['github', 'cnb'].includes(source)) {
        throw new Error(
            'Usage: node verify-update-manifest.mjs <latest.json> <version-tag> [github|cnb]',
        )
    }
    verifyUpdateManifest(JSON.parse(fs.readFileSync(manifestPath, 'utf8')), tag, source)
    console.log(`Verified signed ${source} updater manifest for ${releaseVersion(tag)}`)
}
