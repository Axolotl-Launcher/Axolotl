import fs from 'node:fs'
import path from 'node:path'

import {
    assetDownloadUrl,
    assetSha256,
    isMain,
    releaseVersion,
    verifyRelease,
} from './release-utils.mjs'

export const updaterTargets = [
    { platforms: ['darwin-aarch64', 'darwin-x86_64'], assetSuffix: '_universal.app.tar.gz' },
    { platforms: ['linux-aarch64'], assetSuffix: '_aarch64.AppImage.tar.gz' },
    { platforms: ['linux-x86_64'], assetSuffix: '_amd64.AppImage.tar.gz' },
    { platforms: ['windows-x86_64'], assetSuffix: '_x64-setup.nsis.zip' },
]

export function createUpdateManifest(release, signaturesPath, tag) {
    verifyRelease(release, tag)
    const platforms = {}
    for (const target of updaterTargets) {
        const matches = release.assets.filter((asset) => asset.name.endsWith(target.assetSuffix))
        if (matches.length !== 1) {
            throw new Error(
                `Expected one release asset ending in ${target.assetSuffix}, found ${matches.length}`,
            )
        }
        const asset = matches[0]
        if (!release.assets.some((candidate) => candidate.name === `${asset.name}.sig`)) {
            throw new Error(`Missing release signature attachment ${asset.name}.sig`)
        }
        const signaturePath = path.join(signaturesPath, `${asset.name}.sig`)
        if (!fs.existsSync(signaturePath)) {
            throw new Error(`Missing updater signature ${path.basename(signaturePath)}`)
        }
        const signature = fs.readFileSync(signaturePath, 'utf8')
        if (signature.trim().length < 32)
            throw new Error(`Invalid updater signature ${asset.name}.sig`)
        const sha256 = assetSha256(asset)
        // Draft release metadata can contain temporary untagged download URLs.
        const url = assetDownloadUrl(tag, asset.name)
        for (const platform of target.platforms) platforms[platform] = { signature, url, sha256 }
    }
    return {
        version: releaseVersion(tag),
        notes: release.body ?? '',
        pub_date: release.published_at || new Date().toISOString(),
        platforms,
    }
}

if (isMain(import.meta.url)) {
    const [releasePath, signaturesPath, tag, outputPath] = process.argv.slice(2)
    if (!releasePath || !signaturesPath || !tag || !outputPath) {
        throw new Error(
            'Usage: node create-update-manifest.mjs <release.json> <signatures-dir> <version-tag> <output.json>',
        )
    }
    const manifest = createUpdateManifest(
        JSON.parse(fs.readFileSync(releasePath, 'utf8')),
        signaturesPath,
        tag,
    )
    fs.writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`)
}
