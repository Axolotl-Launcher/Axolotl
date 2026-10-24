import assert from 'node:assert/strict'
import crypto from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import test from 'node:test'

import {
    compareVersions,
    loadConfiguration,
    mirrorRelease,
    pruneVersions,
    retentionPlan,
} from './aliyunoss-release.mjs'
import { assetDownloadUrl } from './release-utils.mjs'

function published(version, overrides = {}) {
    return {
        tag_name: `v${version}`,
        draft: false,
        published_at: '2026-10-24T00:00:00Z',
        assets: [],
        ...overrides,
    }
}

test('retain three stable and three prerelease versions by SemVer, not date or GitHub flags', () => {
    const releases = [
        ...['1.0.1', '1.0.2', '1.0.9', '1.0.10'].map((version) =>
            published(version, { prerelease: true }),
        ),
        ...['2.0.0-beta.1', '2.0.0-beta.2', '2.0.0-beta.9', '2.0.0-beta.10'].map((version) =>
            published(version, { prerelease: false }),
        ),
        published('99.0.0', { draft: true }),
        published('98.0.0', { published_at: null }),
        published('97.0.0', { published_at: 'invalid' }),
        published('1.9.9'),
        published('../outside'),
        published('3.0.0-beta.01'),
        published('1.0.10'),
    ]
    const plan = retentionPlan(releases.reverse())
    assert.deepEqual(plan.retained, [
        '1.0.10',
        '1.0.9',
        '1.0.2',
        '2.0.0-beta.10',
        '2.0.0-beta.9',
        '2.0.0-beta.2',
    ])
    assert.deepEqual(new Set(plan.expired), new Set(['1.0.1', '2.0.0-beta.1']))
})

test('SemVer compares numeric identifiers, prerelease length, hyphens and large integers', () => {
    const ordered = [
        '1.0.0-1',
        '1.0.0-alpha',
        '1.0.0-alpha.1',
        '1.0.0-alpha.2',
        '1.0.0-alpha.10',
        '1.0.0-alpha.beta',
        '1.0.0-alpha-z',
        '1.0.0-beta',
        '1.0.0-beta.2',
        '1.0.0-beta.11',
        '1.0.0-rc.1',
        '1.0.0',
        '1.0.1',
        '1.1.0',
        '2.0.0',
        '9007199254740992.0.0',
        '9007199254740993.0.0',
    ]
    for (let index = 1; index < ordered.length; index++) {
        assert.equal(compareVersions(ordered[index - 1], ordered[index]), -1)
        assert.equal(compareVersions(ordered[index], ordered[index - 1]), 1)
    }
    assert.equal(compareVersions('2.0.0-beta.1', '2.0.0-beta.1'), 0)
    assert.equal(compareVersions('2.0.0', '2.0.0'), 0)
})

test('retention counts match configurable Update Server windows, including zero', () => {
    const releases = ['1.0.0', '1.1.0', '2.0.0-beta.1'].map((version) => published(version))
    assert.deepEqual(retentionPlan(releases, { releaseCount: '1', betaCount: '0' }).retained, [
        '1.1.0',
    ])
    for (const value of [-1, 1.5, '', 'invalid', Infinity, 9007199254740992]) {
        assert.throws(
            () => retentionPlan(releases, { releaseCount: value }),
            /non-negative integer/,
        )
        assert.throws(() => retentionPlan(releases, { betaCount: value }), /non-negative integer/)
    }
})

test('configure HTTPS, V4 signing, optional STS and required credentials', () => {
    const env = {
        ALIYUN_OSS_REGION: 'cn-hangzhou',
        ALIYUN_OSS_BUCKET: 'release-bucket',
        ALIYUN_OSS_ACCESS_KEY_ID: 'test-id',
        ALIYUN_OSS_ACCESS_KEY_SECRET: 'test-secret',
    }
    const config = loadConfiguration(env)
    assert.equal(config.region, 'oss-cn-hangzhou')
    assert.equal(config.secure, true)
    assert.equal(config.authorizationV4, true)
    assert.equal(config.stsToken, undefined)
    assert.equal(
        loadConfiguration({
            ...env,
            ALIYUN_OSS_REGION: config.region,
            ALIYUN_OSS_STS_TOKEN: 'test-token',
        }).stsToken,
        'test-token',
    )
    for (const key of Object.keys(env))
        assert.throws(() => loadConfiguration({ ...env, [key]: '' }), new RegExp(key))
})

function fixture({ large = false, paginated = false } = {}) {
    const tag = 'v1.0.4'
    const files = new Map([
        ['Axolotl.zip', large ? Buffer.alloc(9 * 1024 * 1024, 7) : Buffer.from('installer')],
        ['Axolotl.zip.sig', Buffer.from('signature')],
        ['latest.json', Buffer.from('{"version":"1.0.4"}')],
    ])
    const release = published('1.0.4', {
        assets: [...files].map(([name, content]) => ({
            name,
            size: content.length,
            digest: `sha256:${crypto.createHash('sha256').update(content).digest('hex')}`,
            browser_download_url: assetDownloadUrl(tag, name),
        })),
    })
    const events = []
    const objects = new Map([
        ['dist/1.0.1/expired.zip', { content: Buffer.from('old') }],
        ['dist/1.0.10/keep.zip', { content: Buffer.from('new') }],
        ['dist/not-a-version/keep.zip', { content: Buffer.from('unknown') }],
        ['other/1.0.1/keep.zip', { content: Buffer.from('outside') }],
    ])
    let directory
    let corruptDownload = false
    const releases = [release, published('1.0.3'), published('1.0.2'), published('1.0.1')]
    const run = (args) => {
        if (args[0] === 'release') {
            directory = args[args.indexOf('--dir') + 1]
            for (const [name, content] of files)
                fs.writeFileSync(
                    path.join(directory, name),
                    corruptDownload ? Buffer.alloc(content.length) : content,
                )
            return ''
        }
        events.push(`github:${args[1]}`)
        if (paginated && args[1].endsWith('page=1'))
            return JSON.stringify(
                Array.from({ length: 100 }, () => published('99.0.0', { draft: true })),
            )
        return JSON.stringify(releases)
    }
    const upload = async (key, filename, options) => {
        events.push(`upload:${key}`)
        objects.set(key, { content: fs.readFileSync(filename), sha256: options.meta.sha256 })
    }
    const client = {
        options: { bucket: 'test-bucket' },
        put: upload,
        multipartUpload: async (...args) => {
            events.push('multipart')
            await upload(...args)
        },
        head: async (key) => {
            events.push(`head:${key}`)
            const object = objects.get(key)
            return {
                res: {
                    headers: {
                        'content-length': String(object.content.length),
                        'x-oss-meta-sha256': object.sha256,
                    },
                },
            }
        },
        listV2: async ({ prefix }) => {
            events.push(`list:${prefix}`)
            return {
                isTruncated: false,
                objects: [...objects.keys()]
                    .filter((key) => key.startsWith(prefix))
                    .map((name) => ({ name })),
            }
        },
        deleteMulti: async (keys, options) => {
            assert.equal(options.quiet, false)
            for (const key of keys) {
                events.push(`delete:${key}`)
                objects.delete(key)
            }
            return { deleted: keys.map((Key) => ({ Key })) }
        },
    }
    return {
        tag,
        release,
        releases,
        files,
        events,
        objects,
        run,
        client,
        get directory() {
            return directory
        },
        corrupt() {
            corruptDownload = true
        },
    }
}

test('mirror every attachment including signatures and manifests before pruning exact expired prefixes', async () => {
    const f = fixture({ large: true, paginated: true })
    const result = await mirrorRelease(f.release, f.tag, { client: f.client, run: f.run })
    assert.equal(result.uploaded, f.files.size)
    assert.equal(result.removed, 1)
    assert(f.events.includes('multipart'))
    assert(f.events.some((event) => event.endsWith('page=2')))
    for (const [name, content] of f.files)
        assert.deepEqual(f.objects.get(`dist/1.0.4/${name}`).content, content)
    assert(!f.objects.has('dist/1.0.1/expired.zip'))
    assert(f.objects.has('dist/1.0.10/keep.zip'))
    assert(f.objects.has('dist/not-a-version/keep.zip'))
    assert(f.objects.has('other/1.0.1/keep.zip'))
    assert(
        f.events.findIndex((event) => event.startsWith('list:')) >
            f.events.lastIndexOf('head:dist/1.0.4/latest.json'),
    )
    assert(!fs.existsSync(f.directory))
})

test('corrupt downloads fail before uploading and never trigger cleanup', async () => {
    const f = fixture()
    f.corrupt()
    await assert.rejects(
        mirrorRelease(f.release, f.tag, { client: f.client, run: f.run }),
        /SHA-256/,
    )
    assert.deepEqual(f.events, [])
    assert(!fs.existsSync(f.directory))
})

test('multipart upload retries reuse the checkpoint before allowing pruning', async () => {
    const f = fixture({ large: true })
    const upload = f.client.multipartUpload
    const checkpoint = { uploadId: 'test-upload', doneParts: [{ number: 1, etag: 'part-etag' }] }
    let attempts = 0
    f.client.multipartUpload = async (key, filename, options) => {
        attempts++
        if (attempts === 1) {
            assert.equal(options.checkpoint, undefined)
            options.progress(0.5, checkpoint)
            throw new Error('temporary upload failure')
        }
        assert.deepEqual(options.checkpoint, checkpoint)
        return upload(key, filename, options)
    }
    await mirrorRelease(f.release, f.tag, { client: f.client, run: f.run })
    assert.equal(attempts, 2)
    assert(f.events.some((event) => event.startsWith('delete:')))
})

test('exhausted upload retries never prune previous versions', async () => {
    const f = fixture()
    let attempts = 0
    f.client.put = async () => {
        attempts++
        throw new Error('upload unavailable')
    }
    await assert.rejects(
        mirrorRelease(f.release, f.tag, { client: f.client, run: f.run }),
        /upload unavailable/,
    )
    assert.equal(attempts, 3)
    assert(!f.events.some((event) => event.startsWith('list:') || event.startsWith('delete:')))
    assert(f.objects.has('dist/1.0.1/expired.zip'))
    assert(!fs.existsSync(f.directory))
})

test('rerunning an older release still applies the same newest-version retention window', async () => {
    const f = fixture()
    f.releases.push(...['1.0.5', '1.0.6', '1.0.7'].map((version) => published(version)))
    const result = await mirrorRelease(f.release, f.tag, { client: f.client, run: f.run })
    assert.deepEqual(result.retained, ['1.0.7', '1.0.6', '1.0.5'])
    assert(f.events.includes('upload:dist/1.0.4/latest.json'))
    assert(![...f.objects.keys()].some((key) => key.startsWith('dist/1.0.4/')))
})

test('failed upload verification leaves all previous versions intact', async () => {
    const f = fixture()
    f.client.head = async () => ({ res: { headers: { 'content-length': '0' } } })
    await assert.rejects(
        mirrorRelease(f.release, f.tag, { client: f.client, run: f.run }),
        /metadata does not match/,
    )
    assert(!f.events.some((event) => event.startsWith('list:') || event.startsWith('delete:')))
    assert(f.objects.has('dist/1.0.1/expired.zip'))
    assert(!fs.existsSync(f.directory))
})

test('a missing published GitHub release aborts cleanup after upload', async () => {
    const f = fixture()
    f.releases.shift()
    await assert.rejects(
        mirrorRelease(f.release, f.tag, { client: f.client, run: f.run }),
        /disappeared/,
    )
    assert(!f.events.some((event) => event.startsWith('list:')))
})

test('reject drafts, empty assets, missing checksums and invalid retention before side effects', async () => {
    for (const change of [
        (f) => {
            f.release.draft = true
        },
        (f) => {
            f.release.assets = []
        },
        (f) => {
            delete f.release.assets[0].digest
        },
        (f) => {
            f.release.assets[0].size = -1
        },
        (f) => {
            f.release.assets[0].name = '../outside'
        },
    ]) {
        const f = fixture()
        change(f)
        await assert.rejects(mirrorRelease(f.release, f.tag, { client: f.client, run: f.run }))
        assert.equal(f.directory, undefined)
        assert.deepEqual(f.events, [])
    }
    const f = fixture()
    await assert.rejects(
        mirrorRelease(f.release, f.tag, { client: f.client, run: f.run, releaseCount: -1 }),
        /non-negative integer/,
    )
    assert.equal(f.directory, undefined)
})

test('pruning paginates first, then deletes in batches of at most 1000', async () => {
    const events = []
    const keys = Array.from({ length: 1001 }, (_, index) => `dist/1.0.0/file-${index}`)
    const client = {
        listV2: async (query) => {
            assert.equal(query.prefix, 'dist/1.0.0/')
            events.push('list')
            return query['continuation-token'] === 'next'
                ? { objects: [{ name: keys[1000] }], isTruncated: false }
                : {
                      objects: keys.slice(0, 1000).map((name) => ({ name })),
                      isTruncated: true,
                      nextContinuationToken: 'next',
                  }
        },
        deleteMulti: async (batch) => {
            events.push(batch.length)
            return { deleted: batch.map((Key) => ({ Key })) }
        },
    }
    assert.equal(await pruneVersions(client, ['1.0.0']), 1001)
    assert.deepEqual(events, ['list', 'list', 1000, 1])
})

test('pruning rejects unsafe prefixes, foreign keys, broken pagination and partial delete responses', async () => {
    await assert.rejects(pruneVersions({}, ['../outside']), /eligible version tag/)
    let deletes = 0
    const client = {
        listV2: async () => ({ objects: [{ name: 'dist/1.0.10/keep.zip' }], isTruncated: false }),
        deleteMulti: async () => {
            deletes++
            return { deleted: [] }
        },
    }
    await assert.rejects(pruneVersions(client, ['1.0.1']), /outside/)
    assert.equal(deletes, 0)
    client.listV2 = async () => ({ objects: [], isTruncated: true, nextContinuationToken: 'same' })
    await assert.rejects(pruneVersions(client, ['1.0.1']), /pagination/)
    assert.equal(deletes, 0)
    client.listV2 = async () => ({
        objects: [{ name: 'dist/1.0.1/expired.zip' }],
        isTruncated: false,
    })
    await assert.rejects(pruneVersions(client, ['1.0.1']), /confirm all deletions/)
})
