import crypto from 'node:crypto'
import fs from 'node:fs'

import { isMain, releaseVersion, verifyAssetUrl, verifyRelease } from './release-utils.mjs'

export function createUpdateServerPayload(catalog, release, tag, forceUpdate = false) {
    const version = releaseVersion(tag)
    verifyRelease(release, tag, { published: true })
    if (
        catalog.version !== version ||
        !Array.isArray(catalog.files) ||
        !Array.isArray(catalog.artifacts)
    ) {
        throw new Error('Release catalog metadata does not match the release tag')
    }
    if (!release.id) throw new Error('GitHub release id is required')
    const sortFiles = (files) =>
        [...files].sort((a, b) => (a.filename < b.filename ? -1 : a.filename > b.filename ? 1 : 0))
    const files = sortFiles(catalog.files)
    const names = new Set()
    const assets = files.map((file) => {
        const asset = release.assets.find((candidate) => candidate.name === file.filename)
        if (
            names.has(file.filename) ||
            !asset ||
            file.downloadUrl !== asset.browser_download_url ||
            file.size !== asset.size ||
            !Number.isSafeInteger(file.size) ||
            file.size < 0 ||
            !/^[a-f0-9]{64}$/.test(file.sha256) ||
            `sha256:${file.sha256}` !== asset.digest?.toLowerCase()
        ) {
            throw new Error(`Catalog metadata does not match GitHub asset ${file.filename}`)
        }
        names.add(file.filename)
        verifyAssetUrl(file.downloadUrl, tag, file.filename)
        // Omit volatile API fields such as download_count and uploader metadata.
        return {
            name: asset.name,
            size: asset.size,
            digest: asset.digest.toLowerCase(),
            browser_download_url: asset.browser_download_url,
        }
    })
    return {
        // Separate the stable payload format from notifications sent by the previous script.
        event_id: `github-${tag}-${release.id}-v2`,
        tag,
        version,
        channel: version.includes('-') ? 'beta' : 'release',
        release: {
            id: release.id,
            tag_name: release.tag_name,
            draft: release.draft,
            body: release.body ?? '',
            published_at: release.published_at,
            assets,
        },
        catalog: { version, artifacts: sortFiles(catalog.artifacts), files },
        force_update: forceUpdate,
    }
}

export async function publishUpdateServer(catalog, release, tag) {
    const serverUrl = process.env.UPDATE_SERVER_URL?.replace(/\/$/, '')
    const webhookSecret = process.env.UPDATE_SERVER_WEBHOOK_SECRET
    if (!serverUrl || !webhookSecret)
        throw new Error('UPDATE_SERVER_URL and UPDATE_SERVER_WEBHOOK_SECRET are required')
    const publication = createUpdateServerPayload(
        catalog,
        release,
        tag,
        process.env.UPDATE_SERVER_FORCE_UPDATE === 'true',
    )
    const payload = JSON.stringify(publication)
    const timestamp = String(Math.floor(Date.now() / 1000))
    const signature = crypto
        .createHmac('sha256', webhookSecret)
        .update(`${timestamp}.${payload}`)
        .digest('hex')
    const response = await fetch(`${serverUrl}/api/webhook/release`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'X-Webhook-Timestamp': timestamp,
            'X-Webhook-Signature': `sha256=${signature}`,
        },
        body: payload,
    })
    const result = await response.text()
    if (!response.ok) {
        let errorCode
        try {
            errorCode = JSON.parse(result).error?.code
        } catch {
            // Preserve non-JSON proxy/server errors in the diagnostic below.
        }
        // Older notifications used a different payload format and event id. A matching
        // immutable version can already be public when migrating to this script.
        if (response.status === 409 && errorCode === 'version_immutable') {
            const existingResponse = await fetch(`${serverUrl}/api/versions/${publication.version}`)
            if (existingResponse.ok) {
                const existing = await existingResponse.json()
                const rows = existing.artifacts || []
                const matches =
                    existing.status === 'published' &&
                    String(existing.release_id) === String(release.id) &&
                    existing.release_tag === tag &&
                    existing.channel === publication.channel &&
                    existing.notes === publication.release.body &&
                    existing.force_update === publication.force_update &&
                    Date.parse(existing.published_at) === Date.parse(release.published_at) &&
                    new Set(rows.map((row) => row.filename)).size === catalog.files.length &&
                    catalog.files.every((file) =>
                        rows.some(
                            (row) =>
                                row.filename === file.filename &&
                                row.size === file.size &&
                                row.sha256 === file.sha256,
                        ),
                    )
                if (matches) {
                    console.log(`Update Server already published matching assets for ${tag}`)
                    return
                }
            }
        }
        throw new Error(`Update Server publish failed: ${response.status} ${result}`)
    }
    console.log(`Update Server accepted ${tag}: ${result}`)
}

if (isMain(import.meta.url)) {
    const [catalogPath, releasePath, tag] = process.argv.slice(2)
    if (!catalogPath || !releasePath || !tag) {
        throw new Error(
            'Usage: node publish-update-server.mjs <catalog.json> <release.json> <version-tag>',
        )
    }
    publishUpdateServer(
        JSON.parse(fs.readFileSync(catalogPath, 'utf8')),
        JSON.parse(fs.readFileSync(releasePath, 'utf8')),
        tag,
    ).catch((error) => {
        console.error(error.message)
        process.exitCode = 1
    })
}
