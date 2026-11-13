import fs from 'node:fs'

import {
    firstTagWithArtifacts,
    hasWindowsArm64Assets,
    portableArtifactName,
    portableTargets,
    requiresArtifacts,
    updaterTargets,
} from './create-update-manifest.mjs'
import { isMain, releaseVersion, verifyAssetUrl } from './release-utils.mjs'

export function verifyUpdateManifest(manifest, tag, source = 'github', release) {
    const expectedVersion = releaseVersion(tag)
    if (manifest.version !== expectedVersion) {
        throw new Error(`Manifest version ${manifest.version} does not match ${expectedVersion}`)
    }
    // The digest is self-reported by the manifest, so it only proves freshness against the
    // release itself when the caller passes the release metadata it came from.
    const digests = new Map(
        (release?.assets ?? [])
            .filter((asset) => typeof asset?.digest === 'string')
            .map((asset) => [asset.name, asset.digest.replace(/^sha256:/i, '').toLowerCase()]),
    )
    const arm64 =
        hasWindowsArm64Assets(release) ||
        Object.hasOwn(manifest.platforms ?? {}, 'windows-aarch64') ||
        Object.hasOwn(manifest.artifacts ?? {}, 'portable-arm64')
    for (const target of updaterTargets) {
        if (target.arm64 && !arm64) continue
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
    if (manifest.artifacts !== undefined) {
        if (typeof manifest.artifacts !== 'object' || Array.isArray(manifest.artifacts)) {
            throw new Error('Manifest artifacts must be an object')
        }
        for (const [key, artifact] of Object.entries(manifest.artifacts)) {
            const target = portableTargets.find((candidate) => candidate.key === key)
            if (!target) throw new Error(`Unknown manifest artifact: ${key}`)
            if (
                !artifact ||
                typeof artifact.name !== 'string' ||
                artifact.platform !== target.platform ||
                !/^[a-f0-9]{64}$/.test(artifact.sha256) ||
                !artifact.name.endsWith(target.assetSuffix)
            ) {
                throw new Error(`Invalid manifest artifact: ${key}`)
            }
            // A stale zip attached to a newer release must not be described as this version's.
            if (
                (target.arm64 || artifact.signature !== undefined || artifact.size !== undefined) &&
                (typeof artifact.signature !== 'string' ||
                    artifact.signature.trim().length < 32 ||
                    !Number.isSafeInteger(artifact.size) ||
                    artifact.size <= 0)
            ) {
                throw new Error(`Invalid signed portable metadata: ${key}`)
            }
            const releaseAsset = release?.assets?.find((asset) => asset.name === artifact.name)
            if (artifact.size !== undefined && releaseAsset && artifact.size !== releaseAsset.size)
                throw new Error(`Manifest artifact ${key} size does not match the release asset`)
            if (artifact.name !== portableArtifactName(tag, target)) {
                throw new Error(`Manifest artifact ${key} does not match ${expectedVersion}`)
            }
            const filename = decodeURIComponent(new URL(artifact.url).pathname.split('/').at(-1))
            if (filename !== artifact.name) {
                throw new Error(`Manifest artifact ${key} does not match its download URL`)
            }
            if (digests.has(artifact.name) && digests.get(artifact.name) !== artifact.sha256) {
                throw new Error(`Manifest artifact ${key} digest does not match the release asset`)
            }
            verifyAssetUrl(artifact.url, tag, filename, source)
        }
    }
    // Releases older than the one that introduced the field never had a manifest with it.
    for (const target of portableTargets) {
        if (target.arm64 ? !arm64 : !requiresArtifacts(tag)) continue
        if (manifest.artifacts?.[target.key]) continue
        throw new Error(
            `Manifest is missing the ${target.key} artifact required since ${firstTagWithArtifacts}`,
        )
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
