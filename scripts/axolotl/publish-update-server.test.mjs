import assert from 'node:assert/strict'
import crypto from 'node:crypto'
import test from 'node:test'

import { createUpdateServerPayload, publishUpdateServer } from './publish-update-server.mjs'
import { assetDownloadUrl } from './release-utils.mjs'

const tag = 'v1.10.0-beta.5'
function fixture() {
    const files = ['Axolotl_x64-setup.nsis.zip', 'Axolotl_x64-setup.nsis.zip.sig'].map(
        (filename) => ({
            filename,
            downloadUrl: assetDownloadUrl(tag, filename),
            size: 123,
            sha256: 'a'.repeat(64),
        }),
    )
    const catalog = { version: tag.slice(1), files, artifacts: [{ ...files[0], kind: 'updater' }] }
    const release = {
        id: 123,
        tag_name: tag,
        draft: false,
        published_at: '2026-10-22T12:00:00Z',
        assets: files.map((file) => ({
            name: file.filename,
            browser_download_url: file.downloadUrl,
            size: file.size,
            digest: `sha256:${file.sha256}`,
            download_count: 1,
        })),
    }
    return { catalog, release }
}

test('webhook retries stay byte-identical despite volatile metadata and asset ordering', () => {
    const { catalog, release } = fixture()
    const original = JSON.stringify(createUpdateServerPayload(catalog, release, tag))
    release.assets[0].download_count = 20
    release.assets[0].uploader = { login: 'changed' }
    release.assets.reverse()
    catalog.files.reverse()
    release.assets.push({
        name: 'latest.json',
        browser_download_url: assetDownloadUrl(tag, 'latest.json'),
    })
    assert.equal(JSON.stringify(createUpdateServerPayload(catalog, release, tag)), original)
    assert.equal(JSON.parse(original).event_id, `github-${tag}-123-v2`)
})

test('reject draft metadata, temporary URLs and mismatched checksums before notifying', () => {
    const { catalog, release } = fixture()
    assert.throws(
        () => createUpdateServerPayload(catalog, { ...release, draft: true }, tag),
        /must be published/,
    )
    release.assets[0].browser_download_url = release.assets[0].browser_download_url.replace(
        tag,
        'untagged-example',
    )
    assert.throws(
        () => createUpdateServerPayload(catalog, release, tag),
        /Unexpected github update URL/,
    )
    release.assets[0].browser_download_url = catalog.files[0].downloadUrl
    catalog.files[0].sha256 = 'b'.repeat(64)
    assert.throws(
        () => createUpdateServerPayload(catalog, release, tag),
        /does not match GitHub asset/,
    )
})

test('sign the exact request body and retain actionable server diagnostics', async (t) => {
    const { catalog, release } = fixture()
    const previousUrl = process.env.UPDATE_SERVER_URL
    const previousSecret = process.env.UPDATE_SERVER_WEBHOOK_SECRET
    process.env.UPDATE_SERVER_URL = 'https://update.example'
    process.env.UPDATE_SERVER_WEBHOOK_SECRET = 'test-secret'
    t.after(() => {
        if (previousUrl === undefined) delete process.env.UPDATE_SERVER_URL
        else process.env.UPDATE_SERVER_URL = previousUrl
        if (previousSecret === undefined) delete process.env.UPDATE_SERVER_WEBHOOK_SECRET
        else process.env.UPDATE_SERVER_WEBHOOK_SECRET = previousSecret
    })
    t.mock.method(globalThis, 'fetch', async (url, options) => {
        assert.equal(url, 'https://update.example/api/webhook/release')
        const timestamp = options.headers['X-Webhook-Timestamp']
        const signature = crypto
            .createHmac('sha256', 'test-secret')
            .update(`${timestamp}.${options.body}`)
            .digest('hex')
        assert.equal(options.headers['X-Webhook-Signature'], `sha256=${signature}`)
        return new Response(
            '{"error":{"code":"invalid_github_asset_url","request_id":"example-request"}}',
            { status: 400 },
        )
    })
    await assert.rejects(
        publishUpdateServer(catalog, release, tag),
        /400.*invalid_github_asset_url.*example-request/,
    )
})

test('recognize a matching immutable version published by the previous notification format', async (t) => {
    const { catalog, release } = fixture()
    const previous = {
        url: process.env.UPDATE_SERVER_URL,
        secret: process.env.UPDATE_SERVER_WEBHOOK_SECRET,
    }
    process.env.UPDATE_SERVER_URL = 'https://update.example'
    process.env.UPDATE_SERVER_WEBHOOK_SECRET = 'test-secret'
    t.after(() => {
        for (const [name, value] of [
            ['UPDATE_SERVER_URL', previous.url],
            ['UPDATE_SERVER_WEBHOOK_SECRET', previous.secret],
        ]) {
            if (value === undefined) delete process.env[name]
            else process.env[name] = value
        }
    })
    const existing = {
        status: 'published',
        release_id: String(release.id),
        release_tag: tag,
        channel: 'beta',
        notes: '',
        force_update: false,
        published_at: release.published_at,
        artifacts: catalog.files.map((file) => ({
            filename: file.filename,
            size: file.size,
            sha256: file.sha256,
        })),
    }
    t.mock.method(globalThis, 'fetch', async (url, options) => {
        if (options?.method === 'POST')
            return new Response('{"error":{"code":"version_immutable"}}', { status: 409 })
        assert.equal(url, 'https://update.example/api/versions/1.10.0-beta.5')
        return Response.json(existing)
    })
    await publishUpdateServer(catalog, release, tag)
    existing.artifacts[0].sha256 = 'b'.repeat(64)
    await assert.rejects(publishUpdateServer(catalog, release, tag), /409.*version_immutable/)
})
