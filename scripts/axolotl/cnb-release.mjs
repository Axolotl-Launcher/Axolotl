import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { isExcludedRelease, selectLatestStableRelease } from './release-policy.mjs'
import {
    assetDownloadUrl,
    githubApiBase,
    githubDownloadBase,
    releaseVersion,
    verifyAssetUrl,
    verifyRelease,
} from './release-utils.mjs'
import { verifyUpdateManifest } from './verify-update-manifest.mjs'

const [command, tag, outputDirectory] = process.argv.slice(2)
if (isExcludedRelease(tag)) {
    console.log(`Skipping temporary test release ${tag}`)
    process.exit(0)
}
const apiEndpoint = (process.env.CNB_API_ENDPOINT || 'https://api.cnb.cool').replace(/\/$/, '')
releaseVersion(tag)
const repo = process.env.CNB_REPO_SLUG || 'axlmc/Axolotl'
const token = process.env.CNB_TOKEN
const tokenUser = process.env.CNB_TOKEN_USER_NAME || 'cnb'
const repoUrl = process.env.CNB_REPO_URL_HTTPS || `https://cnb.cool/${repo}.git`
const githubReleaseBaseUrl = (process.env.GITHUB_RELEASE_BASE_URL || githubDownloadBase()).replace(
    /\/$/,
    '',
)
const githubApiBaseUrl = (process.env.GITHUB_API_BASE_URL || githubApiBase()).replace(/\/$/, '')
const configuredAssetMirrorConcurrency = Number(process.env.ASSET_MIRROR_CONCURRENCY || 4)
const assetMirrorConcurrency =
	Number.isSafeInteger(configuredAssetMirrorConcurrency) && configuredAssetMirrorConcurrency > 0
		? configuredAssetMirrorConcurrency
		: 4

if (command !== 'finalize' || !tag || !outputDirectory || !token) {
	throw new Error('Usage: node cnb-release.mjs finalize <tag> <output-dir>; CNB_TOKEN is required')
}

const apiHeaders = {
	Accept: 'application/vnd.cnb.api+json',
	Authorization: `Bearer ${token}`,
}
const githubApiHeaders = {
	Accept: 'application/vnd.github+json',
	'User-Agent': 'Axolotl-CNB-Release',
	...(process.env.GITHUB_TOKEN ? { Authorization: `Bearer ${process.env.GITHUB_TOKEN}` } : {}),
}

async function apiRequest(url, options = {}, allowedStatuses = []) {
	const response = await fetch(url, {
		...options,
		headers: {
			...apiHeaders,
			...options.headers,
		},
	})
	if (!response.ok && !allowedStatuses.includes(response.status)) {
		throw new Error(
			`${options.method || 'GET'} ${url} failed (${response.status}): ${await response.text()}`,
		)
	}
	return response
}

async function getRelease() {
	const response = await apiRequest(
		`${apiEndpoint}/${repo}/-/releases/tags/${encodeURIComponent(tag)}`,
		{},
		[404],
	)
	return response.status === 404 ? null : await response.json()
}

async function ensureRelease(githubRelease) {
	const existing = await getRelease()
	if (existing) {
		return existing
	}

	const prerelease = tag.includes('-')
	const response = await apiRequest(
		`${apiEndpoint}/${repo}/-/releases`,
		{
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({
				tag_name: tag,
				target_commitish: process.env.CNB_COMMIT || tag,
				name:
					process.env.CNB_TAG_RELEASE_TITLE ||
					githubRelease.name ||
					`Axolotl Launcher ${tag}`,
				body: process.env.CNB_TAG_RELEASE_DESC || githubRelease.body || '',
				draft: true,
				prerelease,
				make_latest: 'false',
			}),
		},
		[409],
	)
	if (response.status !== 409) {
		return await response.json()
	}

	for (let attempt = 0; attempt < 10; attempt++) {
		await new Promise((resolve) => setTimeout(resolve, 1000))
		const release = await getRelease()
		if (release) {
			return release
		}
	}
	throw new Error(`Release ${tag} was created concurrently but could not be loaded`)
}

async function createAssetUpload(release, assetName, size) {
	const uploadResponse = await apiRequest(
		`${apiEndpoint}/${repo}/-/releases/${release.id}/asset-upload-url`,
		{
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ asset_name: assetName, size, overwrite: true, ttl: 0 }),
		},
	)
	return await uploadResponse.json()
}

async function verifyAssetUpload(upload) {
	const verifyUrl = new URL(upload.verify_url, apiEndpoint).toString()
	await apiRequest(`${verifyUrl}${verifyUrl.includes('?') ? '&' : '?'}ttl=0`, { method: 'POST' })
}

async function uploadAsset(release, filePath) {
	const assetName = path.basename(filePath)
	const size = fs.statSync(filePath).size
	const upload = await createAssetUpload(release, assetName, size)
	const fileResponse = await fetch(upload.upload_url, {
		method: 'PUT',
		body: fs.createReadStream(filePath),
		duplex: 'half',
		headers: {
			'Content-Length': String(size),
			'Content-Type': 'application/octet-stream',
		},
	})
	if (!fileResponse.ok) {
		throw new Error(
			`Uploading ${assetName} failed (${fileResponse.status}): ${await fileResponse.text()}`,
		)
	}
	await verifyAssetUpload(upload)
	console.log(`Uploaded ${assetName}`)
}

async function loadGithubManifest() {
	const manifestUrl = `${githubReleaseBaseUrl}/${encodeURIComponent(tag)}/latest.json`
	const response = await fetch(manifestUrl)
	if (!response.ok) {
		throw new Error(
			`Downloading GitHub manifest failed (${response.status}): ${await response.text()}`,
		)
	}
	const manifest = await response.json()
    verifyUpdateManifest(manifest, tag)
	if (manifest.version !== tag.replace(/^v/, '')) {
		throw new Error(`GitHub manifest version ${manifest.version} does not match ${tag}`)
	}
	return manifest
}

async function loadGithubRelease(url) {
	const response = await fetch(url, {
		headers: githubApiHeaders,
	})
	if (!response.ok) {
		throw new Error(`Loading GitHub release failed (${response.status}): ${await response.text()}`)
	}
	return await response.json()
}

async function getGithubRelease() {
	return await loadGithubRelease(
		`${githubApiBaseUrl}/releases/tags/${encodeURIComponent(tag)}`,
	)
}

async function getLatestGithubRelease() {
    const latest = await loadGithubRelease(`${githubApiBaseUrl}/releases/latest`)
    if (!isExcludedRelease(latest.tag_name)) return latest

    for (let page = 1; ; page++) {
        const releases = await loadGithubRelease(`${githubApiBaseUrl}/releases?per_page=100&page=${page}`)
        const release = selectLatestStableRelease(releases)
        if (release) return release
        if (releases.length < 100) {
            throw new Error('No eligible stable GitHub release was found')
        }
    }
}

async function mirrorGithubAsset(release, asset) {
	const upload = await createAssetUpload(release, asset.name, asset.size)
	const response = await fetch(asset.browser_download_url)
	if (!response.ok || !response.body) {
		throw new Error(
			`Downloading ${asset.name} failed (${response.status}): ${await response.text()}`,
		)
	}
	const uploadResponse = await fetch(upload.upload_url, {
		method: 'PUT',
		body: response.body,
		duplex: 'half',
		headers: {
			'Content-Length': String(asset.size),
			'Content-Type': 'application/octet-stream',
		},
	})
	if (!uploadResponse.ok) {
		throw new Error(
			`Uploading ${asset.name} failed (${uploadResponse.status}): ${await uploadResponse.text()}`,
		)
	}
	await verifyAssetUpload(upload)
	console.log(`Mirrored ${asset.name}`)
}

async function mirrorGithubAssets(release, githubRelease) {
	const assets = (githubRelease.assets || []).filter((asset) => asset.name !== 'latest.json')
	const mirroredNames = new Set(assets.map((asset) => asset.name))
	if (mirroredNames.size !== assets.length) {
		throw new Error('GitHub release contains duplicate asset names')
	}
	for (const asset of assets) {
        verifyAssetUrl(asset.browser_download_url, tag, asset.name)
		if (
			!asset.name ||
			!asset.browser_download_url ||
			!Number.isSafeInteger(asset.size) ||
			asset.size < 0
		) {
			throw new Error(`Invalid or duplicate GitHub release asset: ${asset.name}`)
		}
	}

	let nextAsset = 0
	const workers = Array.from(
		{ length: Math.min(assetMirrorConcurrency, assets.length) },
		async () => {
			while (nextAsset < assets.length) {
				const asset = assets[nextAsset++]
				await mirrorGithubAsset(release, asset)
			}
		},
	)
	await Promise.all(workers)
	return mirroredNames
}

function createCnbManifest(githubManifest, mirroredNames) {
	const requiredPlatforms = [
		'darwin-aarch64',
		'darwin-x86_64',
		'linux-aarch64',
		'linux-x86_64',
		'windows-x86_64',
	]
	const platforms = {}
	for (const platform of requiredPlatforms) {
		const update = githubManifest.platforms?.[platform]
		if (!update || typeof update.signature !== 'string' || update.signature.trim().length < 32) {
			throw new Error(`Missing signed GitHub update for ${platform}`)
		}
		const filename = decodeURIComponent(path.posix.basename(new URL(update.url).pathname))
		if (!mirroredNames.has(filename)) {
			throw new Error(`GitHub release is missing updater artifact ${filename} for ${platform}`)
		}
		platforms[platform] = {
			...update,
            url: assetDownloadUrl(tag, filename, 'cnb'),
		}
	}

	return {
		...githubManifest,
		version: tag.replace(/^v/, ''),
		platforms,
	}
}

function publishUpdateBranch(manifestPath) {
	if (tag.includes('-')) {
		return
	}

	const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-cnb-update-'))
	const auth = Buffer.from(`${tokenUser}:${token}`).toString('base64')
	const git = (...args) => execFileSync('git', args, { cwd: directory, stdio: 'inherit' })
	git('init')
	git('config', 'user.name', 'Axolotl CNB Release')
	git('config', 'user.email', 'build@cnb.cool')
	git('checkout', '--orphan', 'update')
	fs.copyFileSync(manifestPath, path.join(directory, 'latest.json'))
	git('add', 'latest.json')
    git('commit', '-m', `chore(release): publish ${tag}`)
	git('remote', 'add', 'origin', repoUrl)
	git(
		'-c',
		`http.extraHeader=Authorization: Basic ${auth}`,
		'push',
		'--force',
		'origin',
		'HEAD:update',
	)
}

async function finalizeRelease() {
	fs.mkdirSync(outputDirectory, { recursive: true })
	const [githubManifest, githubRelease, latestGithubRelease] = await Promise.all([
		loadGithubManifest(),
		getGithubRelease(),
		getLatestGithubRelease(),
	])
    verifyRelease(githubRelease, tag, { published: true })
	const isLatestRelease = githubRelease.tag_name === latestGithubRelease.tag_name
	const release = await ensureRelease(githubRelease)
	const mirroredNames = await mirrorGithubAssets(release, githubRelease)
	const manifest = createCnbManifest(githubManifest, mirroredNames)
    verifyUpdateManifest(manifest, tag, 'cnb')
	const manifestPath = path.join(outputDirectory, 'latest.json')
	fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
	await uploadAsset(release, manifestPath)

	const prerelease = tag.includes('-')
	await apiRequest(`${apiEndpoint}/${repo}/-/releases/${release.id}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({
			name: githubRelease.name || `Axolotl Launcher ${tag}`,
			body: githubRelease.body || '',
			draft: false,
			prerelease,
			make_latest: isLatestRelease ? 'true' : 'false',
		}),
	})
	if (isLatestRelease) {
		publishUpdateBranch(manifestPath)
	} else {
		console.log(`Skipped update branch for non-latest GitHub release ${tag}`)
	}
	console.log(`Published CNB release ${tag}`)
}

await finalizeRelease()
