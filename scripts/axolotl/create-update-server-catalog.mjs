import fs from 'node:fs'

import { updaterTargets as manifestTargets } from './create-update-manifest.mjs'
import { releaseVersion, verifyAssetUrl, verifyRelease } from './release-utils.mjs'

const [releasePath, tag, outputPath] = process.argv.slice(2)

if (!releasePath || !tag || !outputPath) {
	throw new Error(
		'Usage: node create-update-server-catalog.mjs <release.json> <version-tag> <output.json>',
	)
}

const version = releaseVersion(tag)
const release = JSON.parse(fs.readFileSync(releasePath, 'utf8'))
verifyRelease(release, tag, { published: true })

function updaterTargets(filename) {
    return (
        manifestTargets.find((target) => filename.endsWith(target.assetSuffix))?.platforms ?? null
    )
}

function describe(filename) {
	const targets = updaterTargets(filename)
	if (targets) {
		return { kind: 'updater', platform: targets[0], targetPlatforms: targets, variant: 'tauri' }
	}
	if (filename.endsWith('.sig')) return { kind: 'signature', platform: null, targetPlatforms: [] }
	if (filename.endsWith('_x64_modern-setup.exe')) {
		return { kind: 'installer', platform: 'windows', targetPlatforms: [], variant: 'modern' }
	}
	if (filename.endsWith('_x64_nsis-setup.exe')) {
		return { kind: 'installer', platform: 'windows', targetPlatforms: [], variant: 'native' }
	}
	if (filename.endsWith('_x64-setup.exe')) {
		return { kind: 'installer', platform: 'windows', targetPlatforms: [], variant: 'legacy' }
	}
	if (filename.endsWith('_x64_portable.zip')) {
		return { kind: 'portable', platform: 'windows', targetPlatforms: [], variant: 'portable' }
	}
	if (filename.endsWith('.dmg')) {
		return { kind: 'installer', platform: 'macos', targetPlatforms: [], variant: 'dmg' }
	}
	if (filename.endsWith('.AppImage')) {
		return { kind: 'installer', platform: 'linux', targetPlatforms: [], variant: 'appimage' }
	}
	if (filename.endsWith('.deb')) {
		return { kind: 'installer', platform: 'linux', targetPlatforms: [], variant: 'deb' }
	}
	if (filename.endsWith('.rpm')) {
		return { kind: 'installer', platform: 'linux', targetPlatforms: [], variant: 'rpm' }
	}
	throw new Error(`Unrecognized release artifact ${filename}`)
}

function architecture(filename) {
	if (/universal/i.test(filename)) return 'universal'
	if (/(aarch64|arm64)/i.test(filename)) return 'aarch64'
	if (/(amd64|x86_64|x64)/i.test(filename)) return 'x86_64'
	return null
}

function digest(asset) {
    if (typeof asset.digest !== 'string' || !/^sha256:[a-f0-9]{64}$/i.test(asset.digest)) {
		throw new Error(`Release asset ${asset.name} has no SHA-256 digest`)
	}
    return asset.digest.slice('sha256:'.length).toLowerCase()
}

const assets = release.assets
	.filter((asset) => asset.name !== 'latest.json')
    .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))
    .map((asset) => {
        verifyAssetUrl(asset.browser_download_url, tag, asset.name)
        if (!Number.isSafeInteger(asset.size) || asset.size < 0) {
            throw new Error(`Invalid release asset size: ${asset.name}`)
        }
        return {
		filename: asset.name,
		size: asset.size,
		sha256: digest(asset),
            downloadUrl: asset.browser_download_url,
		architecture: architecture(asset.name),
		...describe(asset.name),
        }
    })

const primaryArtifacts = assets.filter((artifact) => artifact.kind !== 'signature')
const artifactKeys = new Set()
for (const artifact of assets.filter((candidate) => candidate.kind === 'signature')) {
	const primary = primaryArtifacts.find(
		(candidate) => candidate.filename === artifact.filename.slice(0, -'.sig'.length),
	)
	if (!primary) throw new Error(`Signature has no matching release artifact ${artifact.filename}`)
	Object.assign(artifact, {
		platform: primary.platform,
		architecture: primary.architecture,
		targetPlatforms: primary.targetPlatforms,
		variant: primary.variant,
	})
}

for (const artifact of primaryArtifacts) {
	const signature = assets.find(
        (candidate) =>
            candidate.kind === 'signature' && candidate.filename === `${artifact.filename}.sig`,
	)
	artifact.signatureFilename = signature?.filename ?? null
	if (artifact.kind === 'updater' && !signature) {
		throw new Error(`Missing Tauri updater signature for ${artifact.filename}`)
	}
    const key = [artifact.kind, artifact.platform, artifact.architecture, artifact.variant].join(
        ':',
    )
	if (artifactKeys.has(key)) throw new Error(`Duplicate release artifact classification ${key}`)
	artifactKeys.add(key)
}

fs.writeFileSync(
	outputPath,
	`${JSON.stringify({ version, artifacts: primaryArtifacts, files: assets }, null, 2)}\n`,
)
