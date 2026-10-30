import { pathToFileURL } from 'node:url'

import { isExcludedRelease } from './release-policy.mjs'

export function isMain(url) {
    return process.argv[1] && pathToFileURL(process.argv[1]).href === url
}

export function releaseVersion(tag) {
    const match =
        typeof tag === 'string' &&
        /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/.exec(
            tag,
        )
    if (
        !match ||
        tag !== tag.trim() ||
        isExcludedRelease(tag) ||
        match[4]
            ?.split('.')
            .some((part) => /^\d+$/.test(part) && part.length > 1 && part.startsWith('0'))
    ) {
        throw new Error(`Expected an eligible version tag, received ${tag}`)
    }
    return tag.slice(1)
}

export function githubRepository() {
    const repository = process.env.GITHUB_REPOSITORY || 'Axolotl-Launcher/Axolotl'
    if (!/^[\w.-]+\/[\w.-]+$/.test(repository)) {
        throw new Error(`Invalid GitHub repository: ${repository}`)
    }
    return repository
}

export function githubDownloadBase() {
    return `https://github.com/${githubRepository()}/releases/download`
}

export function githubApiBase() {
    return `https://api.github.com/repos/${githubRepository()}`
}

export function assetDownloadUrl(tag, filename, source = 'github') {
    releaseVersion(tag)
    if (
        typeof filename !== 'string' ||
        !filename ||
        /[/\\\u0000-\u001f]/.test(filename) ||
        ['.', '..'].includes(filename)
    ) {
        throw new Error(`Invalid release asset filename: ${filename}`)
    }
    const base =
        source === 'github'
            ? githubDownloadBase()
            : `https://cnb.cool/${process.env.CNB_REPO_SLUG || 'axlmc/Axolotl'}/-/releases/download`
    if (!['github', 'cnb'].includes(source)) throw new Error(`Unknown release source: ${source}`)
    return `${base}/${encodeURIComponent(tag)}/${encodeURIComponent(filename)}`
}

export function verifyAssetUrl(value, tag, filename, source = 'github') {
    const url = new URL(value)
    const expected = new URL(assetDownloadUrl(tag, filename, source))
    const parts = url.pathname.split('/')
    const expectedParts = expected.pathname.split('/')
    const matches =
        parts.length === expectedParts.length &&
        parts.every((part, index) =>
            index === 1 || index === 2
                ? part.toLowerCase() === expectedParts[index].toLowerCase()
                : part === expectedParts[index],
        )
    if (
        url.protocol !== 'https:' ||
        url.hostname !== expected.hostname ||
        !matches ||
        url.username ||
        url.password ||
        url.search ||
        url.hash ||
        // URL normalizes an explicit default port, so check the original authority too.
        url.origin !== expected.origin ||
        /^https:\/\/[^/]*:/.test(value)
    ) {
        throw new Error(`Unexpected ${source} update URL: ${value}`)
    }
}

export function assetSha256(asset) {
    if (typeof asset?.digest !== 'string' || !/^sha256:[a-f0-9]{64}$/i.test(asset.digest)) {
        throw new Error(`Release asset ${asset?.name} is missing a SHA-256 digest`)
    }
    return asset.digest.slice('sha256:'.length).toLowerCase()
}

export function verifyRelease(release, tag, { published = false } = {}) {
    releaseVersion(tag)
    if (release.tag_name !== tag)
        throw new Error(`Release tag ${release.tag_name} does not match ${tag}`)
    if (!Array.isArray(release.assets))
        throw new Error('Release metadata does not contain an assets array')
    if (
        published &&
        (release.draft !== false ||
            !release.published_at ||
            !Number.isFinite(Date.parse(release.published_at)))
    ) {
        throw new Error(`GitHub release ${tag} must be published`)
    }
    const names = new Set()
    for (const asset of release.assets) {
        assetDownloadUrl(tag, asset.name)
        if (names.has(asset.name)) throw new Error(`Duplicate release asset: ${asset.name}`)
        names.add(asset.name)
        if (published) verifyAssetUrl(asset.browser_download_url, tag, asset.name)
    }
}
