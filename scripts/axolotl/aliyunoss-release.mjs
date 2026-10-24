import { execFileSync } from 'node:child_process'
import crypto from 'node:crypto'
import fs from 'node:fs'
import { createRequire } from 'node:module'
import os from 'node:os'
import path from 'node:path'
import { setTimeout } from 'node:timers/promises'

import {
    assetSha256,
    githubRepository,
    isMain,
    releaseVersion,
    verifyRelease,
} from './release-utils.mjs'

function gh(args) {
    return execFileSync('gh', args, { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 })
}

function retentionCount(value, name) {
    if (!/^\d+$/.test(String(value)) || !Number.isSafeInteger(Number(value))) {
        throw new Error(`${name} must be a non-negative integer`)
    }
    return Number(value)
}

export function loadConfiguration(env = process.env) {
    for (const name of [
        'ALIYUN_OSS_REGION',
        'ALIYUN_OSS_BUCKET',
        'ALIYUN_OSS_ACCESS_KEY_ID',
        'ALIYUN_OSS_ACCESS_KEY_SECRET',
    ]) {
        if (!env[name]?.trim()) throw new Error(`${name} is required`)
    }
    return {
        region: env.ALIYUN_OSS_REGION.startsWith('oss-')
            ? env.ALIYUN_OSS_REGION
            : `oss-${env.ALIYUN_OSS_REGION}`,
        bucket: env.ALIYUN_OSS_BUCKET,
        accessKeyId: env.ALIYUN_OSS_ACCESS_KEY_ID,
        accessKeySecret: env.ALIYUN_OSS_ACCESS_KEY_SECRET,
        ...(env.ALIYUN_OSS_STS_TOKEN ? { stsToken: env.ALIYUN_OSS_STS_TOKEN } : {}),
        authorizationV4: true,
        secure: true,
        timeout: 120000,
    }
}

// Compare SemVer identifiers numerically without precision loss for large version numbers.
export function compareVersions(left, right) {
    const parts = (version) => {
        const [core, prerelease] = version.split(/-(.*)/s)
        return { core: core.split('.').map(BigInt), prerelease: prerelease?.split('.') }
    }
    const a = parts(left)
    const b = parts(right)
    for (let index = 0; index < 3; index++) {
        if (a.core[index] !== b.core[index]) return a.core[index] < b.core[index] ? -1 : 1
    }
    if (!a.prerelease || !b.prerelease) return a.prerelease ? -1 : b.prerelease ? 1 : 0
    for (let index = 0; index < Math.max(a.prerelease.length, b.prerelease.length); index++) {
        const x = a.prerelease[index]
        const y = b.prerelease[index]
        if (x === y) continue
        if (x === undefined || y === undefined) return x === undefined ? -1 : 1
        const numericX = /^\d+$/.test(x)
        const numericY = /^\d+$/.test(y)
        if (numericX && numericY) return BigInt(x) < BigInt(y) ? -1 : 1
        if (numericX !== numericY) return numericX ? -1 : 1
        return x < y ? -1 : 1
    }
    return 0
}

export function retentionPlan(releases, { releaseCount = 3, betaCount = 3 } = {}) {
    releaseCount = retentionCount(releaseCount, 'RELEASE_RETENTION_COUNT')
    betaCount = retentionCount(betaCount, 'BETA_RETENTION_COUNT')
    const published = new Set()
    for (const release of releases) {
        if (
            release?.draft !== false ||
            !release.published_at ||
            !Number.isFinite(Date.parse(release.published_at))
        )
            continue
        try {
            published.add(releaseVersion(release.tag_name))
        } catch {
            // Only eligible published version tags may authorize deletion of an OSS prefix.
        }
    }
    const versions = [...published].sort((a, b) => compareVersions(b, a))
    // Match Update Server: stable and prerelease channels each retain their newest SemVer versions.
    const retained = new Set([
        ...versions.filter((version) => !version.includes('-')).slice(0, releaseCount),
        ...versions.filter((version) => version.includes('-')).slice(0, betaCount),
    ])
    return {
        retained: [...retained],
        expired: versions.filter((version) => !retained.has(version)),
    }
}

function loadPublishedReleases(run) {
    const releases = []
    for (let page = 1; ; page++) {
        const batch = JSON.parse(
            run(['api', `repos/${githubRepository()}/releases?per_page=100&page=${page}`]),
        )
        if (!Array.isArray(batch)) throw new Error('GitHub releases response is not an array')
        releases.push(...batch)
        if (batch.length < 100) return releases
    }
}

async function verifyFile(asset, filename) {
    const info = await fs.promises.lstat(filename)
    if (!info.isFile() || info.size !== asset.size)
        throw new Error(`Downloaded ${asset.name} size does not match GitHub`)
    const hash = crypto.createHash('sha256')
    for await (const chunk of fs.createReadStream(filename)) hash.update(chunk)
    if (hash.digest('hex') !== assetSha256(asset))
        throw new Error(`Downloaded ${asset.name} SHA-256 does not match GitHub`)
}

async function uploadAsset(client, key, asset, filename) {
    const options = { meta: { sha256: assetSha256(asset) } }
    let checkpoint
    for (let attempt = 1; ; attempt++) {
        try {
            if (asset.size <= 8 * 1024 * 1024) {
                await client.put(key, filename, options)
            } else {
                await client.multipartUpload(key, filename, {
                    ...options,
                    partSize: 8 * 1024 * 1024,
                    parallel: 4,
                    checkpoint,
                    progress: (_percentage, nextCheckpoint) => {
                        checkpoint = nextCheckpoint
                    },
                })
            }
            break
        } catch (error) {
            if (attempt === 3) throw error
            console.warn(`Retrying upload of ${asset.name} (attempt ${attempt + 1}/3)`)
            await setTimeout(1000 * 2 ** (attempt - 1))
        }
    }
    const result = await client.head(key)
    if (
        Number(result.res.headers['content-length']) !== asset.size ||
        result.res.headers['x-oss-meta-sha256'] !== assetSha256(asset)
    ) {
        throw new Error(`Uploaded ${key} metadata does not match GitHub`)
    }
    console.log(`Verified oss://${client.options.bucket}/${key}`)
}

export async function pruneVersions(client, versions) {
    let removed = 0
    for (const version of versions) {
        releaseVersion(`v${version}`)
        const prefix = `dist/${version}/`
        const keys = []
        const tokens = new Set()
        let token
        // Finish pagination before deleting so mutations cannot invalidate the listing cursor.
        for (;;) {
            const page = await client.listV2({
                prefix,
                'max-keys': 1000,
                ...(token ? { 'continuation-token': token } : {}),
            })
            for (const object of page.objects ?? []) {
                if (typeof object.name !== 'string' || !object.name.startsWith(prefix))
                    throw new Error(`OSS returned an object outside ${prefix}`)
                keys.push(object.name)
            }
            if (page.isTruncated === false) break
            if (
                page.isTruncated !== true ||
                !page.nextContinuationToken ||
                tokens.has(page.nextContinuationToken)
            )
                throw new Error(`Invalid OSS pagination for ${prefix}`)
            token = page.nextContinuationToken
            tokens.add(token)
        }
        for (let offset = 0; offset < keys.length; offset += 1000) {
            const batch = keys.slice(offset, offset + 1000)
            const result = await client.deleteMulti(batch, { quiet: false })
            const deleted = new Set((result.deleted ?? []).map((entry) => entry.Key))
            if (batch.some((key) => !deleted.has(key)))
                throw new Error(`OSS did not confirm all deletions under ${prefix}`)
            removed += batch.length
        }
        if (keys.length) console.log(`Pruned ${prefix} (${keys.length} objects)`)
    }
    return removed
}

export async function mirrorRelease(
    release,
    tag,
    { client, run = gh, releaseCount = 3, betaCount = 3 } = {},
) {
    const version = releaseVersion(tag)
    verifyRelease(release, tag, { published: true })
    if (!release.assets.length) throw new Error('Published release has no assets')
    for (const asset of release.assets) {
        assetSha256(asset)
        if (!Number.isSafeInteger(asset.size) || asset.size < 0)
            throw new Error(`Invalid size for ${asset.name}`)
    }
    // Validate retention settings before any upload or deletion.
    retentionPlan([], { releaseCount, betaCount })
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-aliyunoss-'))
    try {
        run(['release', 'download', tag, '--repo', githubRepository(), '--dir', directory])
        for (const asset of release.assets)
            await verifyFile(asset, path.join(directory, asset.name))
        for (const asset of release.assets) {
            await uploadAsset(
                client,
                `dist/${version}/${asset.name}`,
                asset,
                path.join(directory, asset.name),
            )
            fs.rmSync(path.join(directory, asset.name))
        }
        // A fresh, fully paginated published-release list is required before pruning.
        const releases = loadPublishedReleases(run)
        const current = releases.find((candidate) => candidate.tag_name === tag)
        if (!current) throw new Error(`GitHub release ${tag} disappeared; refusing to prune`)
        verifyRelease(current, tag, { published: true })
        const plan = retentionPlan(releases, { releaseCount, betaCount })
        console.log(`Retaining OSS versions: ${plan.retained.join(', ') || '(none)'}`)
        const removed = await pruneVersions(client, plan.expired)
        return { uploaded: release.assets.length, removed, ...plan }
    } finally {
        fs.rmSync(directory, { recursive: true, force: true })
    }
}

if (isMain(import.meta.url)) {
    try {
        const [releasePath, tag, ...extra] = process.argv.slice(2)
        if (!releasePath || !tag || extra.length)
            throw new Error('Usage: node aliyunoss-release.mjs <release.json> <version-tag>')
        const config = loadConfiguration()
        const require = process.env.ALIYUN_OSS_SDK_DIR
            ? createRequire(path.resolve(process.env.ALIYUN_OSS_SDK_DIR, 'package.json'))
            : createRequire(import.meta.url)
        const OSS = require('ali-oss')
        const result = await mirrorRelease(JSON.parse(fs.readFileSync(releasePath, 'utf8')), tag, {
            client: new OSS(config),
            releaseCount: process.env.RELEASE_RETENTION_COUNT ?? 3,
            betaCount: process.env.BETA_RETENTION_COUNT ?? 3,
        })
        console.log(
            `Mirrored ${tag}: ${result.uploaded} assets uploaded, ${result.removed} expired objects removed`,
        )
    } catch (error) {
        console.error(`aliyunoss-release: ${error.message}`)
        process.exitCode = 1
    }
}
