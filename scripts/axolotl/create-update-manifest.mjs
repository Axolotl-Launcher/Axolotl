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

// Artifacts that are not used by the updater but whose digests packaging channels
// (for example a Scoop manifest) need published alongside it. They live in their own
// top-level field so the updater's `platforms` contract stays untouched.
export const portableTargets = [
    { key: 'portable', platform: 'windows-x86_64', assetSuffix: '_x64_portable.zip' },
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
    const artifacts = {}
    for (const target of portableTargets) {
        const matches = release.assets.filter((asset) => asset.name.endsWith(target.assetSuffix))
        if (matches.length === 0) continue
        if (matches.length > 1) {
            throw new Error(
                `Expected one release asset ending in ${target.assetSuffix}, found ${matches.length}`,
            )
        }
        const asset = matches[0]
        artifacts[target.key] = {
            platform: target.platform,
            name: asset.name,
            sha256: assetSha256(asset),
            url: assetDownloadUrl(tag, asset.name),
        }
    }
    return {
        version: releaseVersion(tag),
        notes: release.body ?? '',
        pub_date: release.published_at || new Date().toISOString(),
        platforms,
        // Omitted for releases published before this field existed; the verifier treats it
        // as optional for the same reason it treats a platform `sha256` as optional.
        ...(Object.keys(artifacts).length > 0 ? { artifacts } : {}),
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
