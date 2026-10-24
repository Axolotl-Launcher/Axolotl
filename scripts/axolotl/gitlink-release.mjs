import crypto from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { isExcludedRelease } from './release-policy.mjs'
import { githubApiBase, isMain, releaseVersion, verifyRelease } from './release-utils.mjs'

// GitLink reports business failures with HTTP 200, so every response is checked for status === 0.
const DEFAULT_BASE_URL = 'https://www.gitlink.org.cn'
const DEFAULT_REPO_SLUG = 'xinvxueyuan/Axolotl'
const DEFAULT_UPLOAD_CONCURRENCY = 4
const UPLOAD_ATTEMPTS = 3
const GITHUB_PAGE_SIZE = 100
const LATEST_MANIFEST = 'latest.json'
const DRAFT_STATUS = '草稿'
const PRERELEASE_TAG_PATTERN = /-(alpha|beta|rc|pre|nightly)/i
const VERSION_TAG_PATTERN = /^v?\d+(?:\.\d+)*(?:[-+][0-9A-Za-z.+-]+)?$/
const REPO_SLUG_PATTERN = /^[\w.-]+\/[\w.-]+$/
const USER_AGENT = 'Axolotl-GitLink-Release'
const USAGE = [
    'Usage: node gitlink-release.mjs finalize <tag> <output-dir>',
    '       node gitlink-release.mjs verify <tag> [expected-asset-count]',
].join('\n')

export function isPrereleaseTag(tag) {
    return typeof tag === 'string' && PRERELEASE_TAG_PATTERN.test(tag)
}

export function isVersionTag(tag) {
    return typeof tag === 'string' && VERSION_TAG_PATTERN.test(tag)
}

function createdAtKey(release) {
    const value = Date.parse(release?.created_at ?? '')
    return Number.isFinite(value) ? value : Number.NEGATIVE_INFINITY
}

function newestRelease(releases) {
    let newest = null
    for (const release of releases) {
        if (!newest || createdAtKey(release) > createdAtKey(newest)) newest = release
    }
    return newest
}

export function selectReleaseSlots(releases) {
    const published = (Array.isArray(releases) ? releases : []).filter(
        (release) =>
            release &&
            !release.draft &&
            typeof release.tag_name === 'string' &&
            !isExcludedRelease(release.tag_name),
    )
    return {
        stable: newestRelease(published.filter((release) => !release.prerelease)),
        beta: newestRelease(published.filter((release) => Boolean(release.prerelease))),
    }
}

export function computeAllowedTags(releases) {
    const { stable, beta } = selectReleaseSlots(releases)
    return [...new Set([stable?.tag_name, beta?.tag_name].filter((tag) => typeof tag === 'string'))]
}

export function selectPruneCandidates(releases, allowedTags) {
    const allowed = new Set(allowedTags ?? [])
    return (Array.isArray(releases) ? releases : []).filter(
        (release) =>
            release &&
            typeof release.tag_name === 'string' &&
            !allowed.has(release.tag_name) &&
            isVersionTag(release.tag_name),
    )
}

export function collectAttachmentTitles(release) {
    const titles = new Set()
    for (const attachment of Array.isArray(release?.attachments) ? release.attachments : []) {
        if (typeof attachment?.title === 'string' && attachment.title) titles.add(attachment.title)
    }
    return titles
}

export function selectMissingAssets(assets, existingTitles) {
    const known = existingTitles instanceof Set ? existingTitles : new Set(existingTitles ?? [])
    const seen = new Set()
    const missing = []
    for (const asset of Array.isArray(assets) ? assets : []) {
        if (typeof asset?.name !== 'string' || asset.name === '') continue
        if (known.has(asset.name) || seen.has(asset.name)) continue
        seen.add(asset.name)
        missing.push(asset)
    }
    return missing
}

export function findReleaseByTag(releases, tag) {
    const matches = (Array.isArray(releases) ? releases : []).filter(
        (release) => release && release.tag_name === tag,
    )
    if (matches.length === 0) return null
    // A duplicate tag is possible upstream, so prefer the published record over a draft.
    return matches.find((release) => release.draft !== DRAFT_STATUS) ?? matches[0]
}

export function requireVersionId(release, tag) {
    const versionId = Number(release?.version_id)
    if (!Number.isSafeInteger(versionId) || versionId <= 0) {
        throw new Error(`GitLink release ${tag} has no usable version_id: ${release?.version_id}`)
    }
    return versionId
}

export function assertGitlinkStatus(payload, action) {
    if (!payload || typeof payload !== 'object') {
        throw new Error(`GitLink ${action} returned a non-JSON response: ${String(payload)}`)
    }
    if (payload.status !== 0) {
        const message =
            typeof payload.message === 'string' && payload.message.trim()
                ? payload.message
                : JSON.stringify(payload)
        throw new Error(`GitLink ${action} failed: ${message}`)
    }
    return payload
}

export function resolvePrerelease(release, tag) {
    const declared = Boolean(release?.prerelease)
    const byTag = isPrereleaseTag(tag)
    return { declared, byTag, prerelease: byTag, mismatch: declared !== byTag }
}

export function buildReleasePayload({ release, tag, attachmentIds = [], prerelease = false }) {
    const name =
        typeof release?.name === 'string' && release.name.trim()
            ? release.name
            : `Axolotl Launcher ${tag}`
    const commitish =
        typeof release?.target_commitish === 'string' && release.target_commitish
            ? release.target_commitish
            : 'main'
    return {
        name,
        tag_name: tag,
        target_commitish: commitish,
        body: typeof release?.body === 'string' ? release.body : '',
        draft: false,
        prerelease: Boolean(prerelease),
        attachment_ids: (Array.isArray(attachmentIds) ? attachmentIds : []).filter(
            (id) => typeof id === 'string' && id !== '',
        ),
    }
}

export function validateReleasePayload(payload) {
    // GitLink accepts PUT/POST only with a name, and a missing one fails with HTTP 200.
    if (typeof payload?.name !== 'string' || payload.name.trim() === '') {
        throw new Error(
            'GitLink release payload requires a non-empty name (PUT and POST reject it)',
        )
    }
    if (typeof payload.tag_name !== 'string' || payload.tag_name === '') {
        throw new Error('GitLink release payload requires a tag_name')
    }
    if (payload.draft !== false) {
        throw new Error('GitLink release payload must publish the record (draft: false)')
    }
    if (typeof payload.prerelease !== 'boolean') {
        throw new Error('GitLink release payload requires a boolean prerelease flag')
    }
    if (!Array.isArray(payload.attachment_ids)) {
        throw new Error('GitLink release payload requires an attachment_ids array')
    }
    return payload
}

export function verifyDownloadedAsset(asset, { bytes, sha256 }) {
    if (Number.isSafeInteger(asset?.size) && asset.size >= 0 && bytes !== asset.size) {
        throw new Error(`Downloaded ${asset.name} has ${bytes} bytes, expected ${asset.size}`)
    }
    if (typeof asset?.digest === 'string' && /^sha256:[a-f0-9]{64}$/i.test(asset.digest)) {
        const expected = asset.digest.slice('sha256:'.length).toLowerCase()
        if (`${sha256}`.toLowerCase() !== expected) {
            throw new Error(`Downloaded ${asset.name} digest ${sha256} does not match ${expected}`)
        }
    }
    return bytes
}

function preview(value) {
    return JSON.stringify(value).slice(0, 300)
}

function formatSize(bytes) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`
}

function sleep(milliseconds) {
    return new Promise((resolve) => setTimeout(resolve, milliseconds))
}

function loadConfiguration({ requireToken }) {
    const baseUrl = (process.env.GITLINK_BASE_URL || DEFAULT_BASE_URL).replace(/\/+$/, '')
    const repoSlug = process.env.GITLINK_REPO_SLUG || DEFAULT_REPO_SLUG
    if (!REPO_SLUG_PATTERN.test(repoSlug)) {
        throw new Error(`Invalid GitLink repository slug: ${repoSlug}`)
    }
    const token = (process.env.GITLINK_TOKEN || '').trim()
    if (requireToken && token === '') {
        throw new Error('GITLINK_TOKEN is required to mirror releases to GitLink')
    }
    const configuredConcurrency = Number(
        process.env.GITLINK_UPLOAD_CONCURRENCY || DEFAULT_UPLOAD_CONCURRENCY,
    )
    return {
        baseUrl,
        repoSlug,
        token,
        uploadConcurrency:
            Number.isSafeInteger(configuredConcurrency) && configuredConcurrency > 0
                ? configuredConcurrency
                : DEFAULT_UPLOAD_CONCURRENCY,
        prune: !['0', 'false', 'no', 'off'].includes(
            (process.env.GITLINK_PRUNE ?? '1').trim().toLowerCase(),
        ),
    }
}

const releasesPath = (config) => `/api/${config.repoSlug}/releases.json`
const releasePath = (config, versionId) => `/api/${config.repoSlug}/releases/${versionId}.json`

async function readJsonResponse(response, label) {
    const text = await response.text()
    try {
        return text ? JSON.parse(text) : {}
    } catch {
        throw new Error(
            `${label} returned a non-JSON response (${response.status}): ${text.slice(0, 300)}`,
        )
    }
}

async function gitlinkRequest(config, pathname, { method = 'GET', body, token } = {}) {
    const authToken = token === undefined ? config.token : token
    const headers = { Accept: 'application/json', 'User-Agent': USER_AGENT }
    if (authToken) headers.Authorization = `Bearer ${authToken}`
    if (body !== undefined) headers['Content-Type'] = 'application/json'
    const url = `${config.baseUrl}${pathname}`
    const response = await fetch(url, {
        method,
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
    })
    const payload = await readJsonResponse(response, `${method} ${url}`)
    if (!response.ok) {
        throw new Error(`${method} ${url} failed (${response.status}): ${preview(payload)}`)
    }
    return payload
}

async function loadGitlinkReleases(config, { anonymous = false } = {}) {
    const payload = await gitlinkRequest(config, releasesPath(config), {
        token: anonymous ? '' : config.token,
    })
    assertGitlinkStatus(payload, `GET releases (${config.repoSlug})`)
    if (!Array.isArray(payload.releases)) {
        throw new Error('GitLink releases response does not contain a releases array')
    }
    return payload.releases
}

function githubHeaders() {
    const headers = { Accept: 'application/vnd.github+json', 'User-Agent': USER_AGENT }
    if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`
    return headers
}

function githubApiUrl() {
    return (process.env.GITHUB_API_BASE_URL || githubApiBase()).replace(/\/+$/, '')
}

async function loadGithubJson(pathname) {
    const url = `${githubApiUrl()}${pathname}`
    const response = await fetch(url, { headers: githubHeaders() })
    if (!response.ok) {
        throw new Error(
            `GitHub request ${url} failed (${response.status}): ${await response.text()}`,
        )
    }
    return await response.json()
}

async function loadGithubRelease(tag) {
    return await loadGithubJson(`/releases/tags/${encodeURIComponent(tag)}`)
}

// The retention policy is derived from the full list, so every page is read.
async function loadGithubReleases() {
    const releases = []
    for (let page = 1; ; page++) {
        const batch = await loadGithubJson(`/releases?per_page=${GITHUB_PAGE_SIZE}&page=${page}`)
        if (!Array.isArray(batch)) throw new Error('GitHub releases response is not an array')
        releases.push(...batch)
        if (batch.length < GITHUB_PAGE_SIZE) return releases
    }
}

async function downloadAsset(asset, directory) {
    const response = await fetch(asset.browser_download_url)
    if (!response.ok || !response.body) {
        throw new Error(`Downloading ${asset.name} failed (${response.status})`)
    }
    const filePath = path.join(directory, asset.name)
    const hash = crypto.createHash('sha256')
    const handle = await fs.promises.open(filePath, 'w')
    let bytes = 0
    try {
        // Stream to disk so a large asset never has to fit in memory.
        for await (const chunk of response.body) {
            const buffer = Buffer.from(chunk)
            bytes += buffer.length
            hash.update(buffer)
            await handle.write(buffer)
        }
    } finally {
        await handle.close()
    }
    verifyDownloadedAsset(asset, { bytes, sha256: hash.digest('hex') })
    return { filePath, bytes }
}

async function uploadAttachment(config, filePath, title) {
    // openAsBlob keeps the payload on disk and lets fetch stream it as multipart.
    const blob = await fs.openAsBlob(filePath)
    const form = new FormData()
    form.append('file', blob, title)
    const url = `${config.baseUrl}/api/attachments.json`
    const response = await fetch(url, {
        method: 'POST',
        headers: {
            Accept: 'application/json',
            Authorization: `Bearer ${config.token}`,
            'User-Agent': USER_AGENT,
        },
        body: form,
    })
    const payload = await readJsonResponse(response, `POST ${url}`)
    if (!response.ok) {
        throw new Error(`Uploading ${title} failed (${response.status}): ${preview(payload)}`)
    }
    if (payload.status !== undefined && payload.status !== 0) {
        throw new Error(`Uploading ${title} failed: ${payload.message || preview(payload)}`)
    }
    if (typeof payload.id !== 'string' || payload.id === '') {
        throw new Error(`Uploading ${title} returned no attachment id: ${preview(payload)}`)
    }
    return payload.id
}

async function uploadAttachmentWithRetry(config, filePath, title) {
    let lastError
    for (let attempt = 1; attempt <= UPLOAD_ATTEMPTS; attempt++) {
        try {
            return await uploadAttachment(config, filePath, title)
        } catch (error) {
            lastError = error
            if (attempt === UPLOAD_ATTEMPTS) break
            const delay = 1000 * 2 ** (attempt - 1)
            console.warn(
                `Upload attempt ${attempt}/${UPLOAD_ATTEMPTS} for ${title} failed: ` +
                    error.message,
            )
            await sleep(delay)
        }
    }
    // A release with only some of its assets is worse than a failed run, so this throws.
    throw new Error(
        `Uploading ${title} failed after ${UPLOAD_ATTEMPTS} attempts: ${lastError?.message}`,
    )
}

async function mirrorAsset(config, asset, directory) {
    const startedAt = Date.now()
    const { filePath, bytes } = await downloadAsset(asset, directory)
    try {
        const id = await uploadAttachmentWithRetry(config, filePath, asset.name)
        console.log(`Uploaded ${asset.name} (${formatSize(bytes)}) in ${Date.now() - startedAt}ms`)
        return { title: asset.name, id }
    } finally {
        fs.rmSync(filePath, { force: true })
    }
}

async function mirrorAssets(config, assets, directory) {
    const uploaded = []
    let next = 0
    const workers = Array.from(
        { length: Math.min(config.uploadConcurrency, assets.length) },
        async () => {
            while (next < assets.length) {
                const asset = assets[next++]
                uploaded.push(await mirrorAsset(config, asset, directory))
            }
        },
    )
    await Promise.all(workers)
    return uploaded
}

async function writeGitlinkRelease(config, { release, tag, attachmentIds, prerelease, existing }) {
    const payload = validateReleasePayload(
        buildReleasePayload({ release, tag, attachmentIds, prerelease }),
    )
    if (!existing) {
        assertGitlinkStatus(
            await gitlinkRequest(config, releasesPath(config), { method: 'POST', body: payload }),
            `POST release ${tag}`,
        )
        console.log(
            `Created GitLink release ${tag} with ${payload.attachment_ids.length} new attachments`,
        )
        return
    }
    const nameChanged = (existing.name ?? '') !== payload.name
    const bodyChanged = (existing.body ?? '') !== payload.body
    if (payload.attachment_ids.length === 0 && !nameChanged && !bodyChanged) {
        console.log(`GitLink release ${tag} is already up to date`)
        return
    }
    const versionId = requireVersionId(existing, tag)
    assertGitlinkStatus(
        await gitlinkRequest(config, releasePath(config, versionId), {
            method: 'PUT',
            body: payload,
        }),
        `PUT release ${tag}`,
    )
    console.log(
        `Updated GitLink release ${tag} (version_id ${versionId}, ` +
            `${payload.attachment_ids.length} new attachments appended)`,
    )
}

async function loadLatestManifest(config, tag) {
    const url =
        `${config.baseUrl}/${config.repoSlug}/releases/download/` +
        `${encodeURIComponent(tag)}/${LATEST_MANIFEST}`
    const response = await fetch(url, {
        headers: {
            Accept: 'application/json',
            'User-Agent': USER_AGENT,
            'Cache-Control': 'no-cache',
        },
    })
    const text = await response.text()
    let manifest = null
    try {
        manifest = JSON.parse(text)
    } catch {
        manifest = null
    }
    // GitLink answers HTTP 200 with HTML or an error object, so the payload decides.
    if (
        !response.ok ||
        !manifest ||
        typeof manifest !== 'object' ||
        typeof manifest.version !== 'string' ||
        manifest.version === ''
    ) {
        throw new Error(`GitLink ${LATEST_MANIFEST} for ${tag} is unavailable (${response.status})`)
    }
    return manifest
}

async function readBackRelease(config, tag, expectedAssets) {
    const releases = await loadGitlinkReleases(config, { anonymous: true })
    const release = findReleaseByTag(releases, tag)
    if (!release) throw new Error(`GitLink release ${tag} was not found after mirroring`)
    if (release.draft === DRAFT_STATUS) throw new Error(`GitLink release ${tag} is still a draft`)
    const titles = collectAttachmentTitles(release)
    const missing = expectedAssets.filter((name) => !titles.has(name))
    if (missing.length > 0) {
        throw new Error(`GitLink release ${tag} is missing attachments: ${missing.join(', ')}`)
    }
    if (!titles.has(LATEST_MANIFEST)) {
        throw new Error(`GitLink release ${tag} is missing ${LATEST_MANIFEST}`)
    }
    const manifest = await loadLatestManifest(config, tag)
    return { release, titles, manifest }
}

async function pruneReleases(config, allowedTags) {
    const releases = await loadGitlinkReleases(config)
    const candidates = selectPruneCandidates(releases, allowedTags)
    if (candidates.length === 0) {
        console.log('Nothing to prune: GitLink already holds only the allowed releases')
        return []
    }
    const pruned = []
    for (const release of candidates) {
        const versionId = requireVersionId(release, release.tag_name)
        assertGitlinkStatus(
            await gitlinkRequest(config, releasePath(config, versionId), { method: 'DELETE' }),
            `DELETE release ${release.tag_name}`,
        )
        pruned.push(release.tag_name)
        console.log(`Pruned ${release.tag_name} (version_id ${versionId})`)
    }
    return pruned
}

async function finalizeRelease(tag, outputDirectory) {
    // releaseVersion() itself rejects excluded tags, so the skip check has to run first.
    if (isExcludedRelease(tag)) {
        console.log(`Skipping temporary test release ${tag}`)
        return
    }
    releaseVersion(tag)
    const config = loadConfiguration({ requireToken: true })
    fs.mkdirSync(outputDirectory, { recursive: true })

    const [githubRelease, githubReleases] = await Promise.all([
        loadGithubRelease(tag),
        loadGithubReleases(),
    ])
    verifyRelease(githubRelease, tag, { published: true })

    const allowed = computeAllowedTags(githubReleases)
    console.log(`Allowed GitLink releases: ${allowed.join(', ') || '(none)'}`)
    if (!allowed.includes(tag)) {
        console.warn(
            `WARN ${tag} is not an allowed GitLink slot; the policy keeps ${allowed.join(', ')}`,
        )
    }

    const existing = findReleaseByTag(await loadGitlinkReleases(config), tag)
    const existingTitles = collectAttachmentTitles(existing)
    const expectedAssets = githubRelease.assets.map((asset) => asset.name)
    const missingAssets = selectMissingAssets(githubRelease.assets, existingTitles)
    console.log(
        `GitLink ${tag}: ${expectedAssets.length} assets, ${existingTitles.size} present, ` +
            `${missingAssets.length} to upload`,
    )

    const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-gitlink-'))
    let uploaded = []
    try {
        uploaded = await mirrorAssets(config, missingAssets, temporaryDirectory)
    } finally {
        fs.rmSync(temporaryDirectory, { recursive: true, force: true })
    }

    const { declared, prerelease, mismatch } = resolvePrerelease(githubRelease, tag)
    if (mismatch) {
        console.warn(
            `WARN GitHub prerelease=${declared} disagrees with tag ${tag}; ` +
                `using the tag-derived prerelease=${prerelease}`,
        )
    }

    await writeGitlinkRelease(config, {
        release: githubRelease,
        tag,
        attachmentIds: uploaded.map((entry) => entry.id),
        prerelease,
        existing,
    })

    const { release, titles, manifest } = await readBackRelease(config, tag, expectedAssets)
    console.log(
        `Verified GitLink ${tag}: draft=${release.draft ?? 'unknown'}, ` +
            `${titles.size} attachments, ${LATEST_MANIFEST} version ${manifest.version}`,
    )

    const pruned = config.prune ? await pruneReleases(config, allowed) : []

    const summary = {
        tag,
        version_id: release.version_id,
        assetCount: titles.size,
        uploaded: uploaded.map((entry) => entry.title),
        pruned,
        allowed,
        verifiedAt: new Date().toISOString(),
    }
    const summaryPath = path.join(outputDirectory, 'gitlink-release.json')
    fs.writeFileSync(summaryPath, `${JSON.stringify(summary, null, 2)}\n`)
    console.log(`Wrote ${summaryPath}`)
}

async function verifyMirroredRelease(tag, expectedAssetCountText) {
    releaseVersion(tag)
    const config = loadConfiguration({ requireToken: false })
    let expectedAssetCount = null
    if (expectedAssetCountText !== undefined) {
        expectedAssetCount = Number(expectedAssetCountText)
        if (!Number.isSafeInteger(expectedAssetCount) || expectedAssetCount < 0) {
            throw new Error(`Invalid expected asset count: ${expectedAssetCountText}`)
        }
    }

    const releases = await loadGitlinkReleases(config, { anonymous: true })
    const release = findReleaseByTag(releases, tag)
    if (!release) throw new Error(`GitLink release ${tag} is missing from ${config.repoSlug}`)
    if (release.draft === DRAFT_STATUS) throw new Error(`GitLink release ${tag} is still a draft`)

    const titles = collectAttachmentTitles(release)
    if (expectedAssetCount !== null && titles.size < expectedAssetCount) {
        throw new Error(
            `GitLink release ${tag} has ${titles.size} attachments, ` +
                `expected at least ${expectedAssetCount}`,
        )
    }
    const manifest = await loadLatestManifest(config, tag)
    console.log(
        `GitLink release ${tag} is public: draft=${release.draft ?? 'unknown'}, ` +
            `attachments=${titles.size}, ${LATEST_MANIFEST} version=${manifest.version}`,
    )
}

async function main(argv) {
    const [command, tag, ...rest] = argv
    try {
        if (command === 'finalize') {
            const [outputDirectory] = rest
            if (!tag || !outputDirectory) throw new Error(USAGE)
            await finalizeRelease(tag, outputDirectory)
        } else if (command === 'verify') {
            if (!tag) throw new Error(USAGE)
            await verifyMirroredRelease(tag, rest[0])
        } else {
            throw new Error(USAGE)
        }
    } catch (error) {
        console.error(`gitlink-release: ${error instanceof Error ? error.message : String(error)}`)
        process.exitCode = 1
    }
}

if (isMain(import.meta.url)) {
    await main(process.argv.slice(2))
}
