import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import crypto from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'

import {
    assertGitlinkStatus,
    buildReleasePayload,
    collectAttachmentTitles,
    computeAllowedTags,
    findReleaseByTag,
    isPrereleaseTag,
    resolvePrerelease,
    selectMissingAssets,
    selectPruneCandidates,
    selectReleaseSlots,
    validateReleasePayload,
    verifyDownloadedAsset,
} from './gitlink-release.mjs'

const SCRIPT = path.join(import.meta.dirname, 'gitlink-release.mjs')
const GITLINK_BASE = 'https://www.gitlink.org.cn'
const REPO_SLUG = 'xinvxueyuan/Axolotl'
const RELEASES_URL = GITLINK_BASE + '/api/' + REPO_SLUG + '/releases.json'
const INSTALLER = 'Axolotl_1.10.0_x64-setup.nsis.zip'
const MANIFEST = 'latest.json'
const GITHUB_RELEASES = [
    { tag_name: 'v1.10.0', draft: false, prerelease: false, created_at: '2026-10-23T10:00:00Z' },
    {
        tag_name: 'v1.10.0-beta.5',
        draft: false,
        prerelease: true,
        created_at: '2026-10-22T10:00:00Z',
    },
    { tag_name: 'v1.9.0', draft: false, prerelease: false, created_at: '2026-10-01T10:00:00Z' },
    // Newest stable upstream, but excluded from every release baseline.
    { tag_name: 'v1.9.9', draft: false, prerelease: false, created_at: '2026-10-24T10:00:00Z' },
]

function createFixture(tag) {
    const version = tag.replace(/^v/, '')
    const buffers = {
        [INSTALLER]: Buffer.from('installer-payload-'.repeat(8)),
        [MANIFEST]: Buffer.from(JSON.stringify({ version, platforms: {} })),
    }
    const contents = {}
    const assets = []
    for (const [name, buffer] of Object.entries(buffers)) {
        contents[name] = buffer.toString('base64')
        assets.push({
            name,
            size: buffer.length,
            browser_download_url:
                'https://github.com/Example/Fork/releases/download/' + tag + '/' + name,
            // Only one asset advertises a digest so both verification paths are used.
            ...(name === INSTALLER
                ? { digest: 'sha256:' + crypto.createHash('sha256').update(buffer).digest('hex') }
                : {}),
        })
    }
    return {
        contents,
        version,
        assets,
        release: {
            id: 1234,
            tag_name: tag,
            name: 'Axolotl Launcher ' + tag,
            body: '## Axolotl Launcher ' + version,
            target_commitish: 'main',
            draft: false,
            prerelease: tag.includes('-'),
            published_at: '2026-10-23T10:00:00Z',
            assets,
        },
    }
}

function gitlinkRelease(versionId, tag, attachmentTitles, { draft = '稳定' } = {}) {
    return {
        version_id: versionId,
        id: String(versionId),
        tag_name: tag,
        name: 'Axolotl Launcher ' + tag,
        body: '## Axolotl Launcher ' + tag.replace(/^v/, ''),
        draft,
        attachments: attachmentTitles.map((title, index) => ({
            id: 5000 + index,
            title,
            filesize: '424 字节',
            url: '/' + REPO_SLUG + '/releases/download/' + tag + '/' + title,
        })),
    }
}

// Offline GitLink/GitHub double: the CLI never reaches the network in these tests.
function createStub({ tag, fixture, gitlinkReleases, writeStatus = null, uploadFailures = 0 }) {
    return String.raw`import assert from 'node:assert/strict'
import fs from 'node:fs'

const TAG = ${JSON.stringify(tag)}
const ASSETS = ${JSON.stringify(fixture.assets)}
const CONTENTS = ${JSON.stringify(fixture.contents)}
const GITHUB_RELEASE = ${JSON.stringify(fixture.release)}
const GITHUB_RELEASE_LIST = ${JSON.stringify(GITHUB_RELEASES)}
const GITLINK_RELEASES = ${JSON.stringify(gitlinkReleases)}
const WRITE_STATUS = ${JSON.stringify(writeStatus)}
const UPLOAD_FAILURES = ${uploadFailures}
const MANIFEST_VERSION = ${JSON.stringify(fixture.version)}
const STATE_FILE = process.env.GITLINK_TEST_STATE
const BASE = 'https://www.gitlink.org.cn'
const REPO = 'xinvxueyuan/Axolotl'
const LIST_URL = BASE + '/api/' + REPO + '/releases.json'

const state = {
    releases: GITLINK_RELEASES,
    requests: [],
    uploads: [],
    uploadedTitles: [],
    failedUploads: 0,
    posts: [],
    puts: [],
    deletes: [],
}
const record = () => {
    if (STATE_FILE) fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2))
}
record()

const json = (value, status = 200) => new Response(JSON.stringify(value), { status })
const ok = { status: 0, message: '发布成功' }

const attachUploaded = (release, ids) => {
    for (const id of ids) {
        const index = state.uploads.indexOf(id)
        if (index < 0) continue
        release.attachments.push({
            id: 9000 + index,
            title: state.uploadedTitles[index],
            filesize: '1 MiB',
            url: '/x',
        })
    }
}

globalThis.fetch = async (url, settings = {}) => {
    const value = String(url)
    const method = (settings.method || 'GET').toUpperCase()
    state.requests.push({
        method,
        url: value,
        authorization: settings.headers && settings.headers.Authorization ? 'present' : 'absent',
    })
    record()

    if (value === 'https://api.github.com/repos/Example/Fork/releases/tags/' + TAG) {
        return json(GITHUB_RELEASE)
    }
    if (value.startsWith('https://api.github.com/repos/Example/Fork/releases?')) {
        return json(GITHUB_RELEASE_LIST)
    }
    const asset = ASSETS.find((item) => item.browser_download_url === value)
    if (asset) return new Response(Buffer.from(CONTENTS[asset.name], 'base64'))

    if (value === BASE + '/api/attachments.json' && method === 'POST') {
        assert(settings.body instanceof FormData, 'attachment upload must be multipart')
        const file = settings.body.get('file')
        assert(
            file && typeof file.name === 'string' && file.name !== '',
            'field "file" is required',
        )
        if (state.failedUploads < UPLOAD_FAILURES) {
            state.failedUploads += 1
            record()
            return new Response('upload interrupted', { status: 500 })
        }
        const id = 'upload-' + state.uploads.length
        state.uploads.push(id)
        state.uploadedTitles.push(file.name)
        record()
        return json({ id })
    }
    if (value === LIST_URL && method === 'GET') {
        return json({ status: 0, message: '响应成功', releases: state.releases })
    }
    if (value === LIST_URL && method === 'POST') {
        const body = JSON.parse(settings.body)
        state.posts.push(body)
        const release = {
            version_id: 9001,
            id: '9001',
            tag_name: body.tag_name,
            name: body.name,
            body: body.body,
            draft: body.prerelease ? '预发行' : '稳定',
            attachments: [],
        }
        attachUploaded(release, body.attachment_ids)
        state.releases.push(release)
        record()
        return json(WRITE_STATUS || ok)
    }
    const target =
        /^https:\/\/www\.gitlink\.org\.cn\/api\/xinvxueyuan\/Axolotl\/releases\/(\d+)\.json$/.exec(
            value,
        )
    if (target && method === 'PUT') {
        const body = JSON.parse(settings.body)
        state.puts.push(body)
        const release = state.releases.find((item) => item.version_id === Number(target[1]))
        if (release && !WRITE_STATUS) {
            release.name = body.name
            release.body = body.body
            release.draft = body.prerelease ? '预发行' : '稳定'
            attachUploaded(release, body.attachment_ids)
        }
        record()
        return json(WRITE_STATUS || ok)
    }
    if (target && method === 'DELETE') {
        state.deletes.push(Number(target[1]))
        state.releases = state.releases.filter((item) => item.version_id !== Number(target[1]))
        record()
        return json({ status: 0, message: '删除成功' })
    }
    if (value === BASE + '/' + REPO + '/releases/download/' + TAG + '/latest.json') {
        const published = state.releases.some(
            (item) =>
                item.tag_name === TAG &&
                item.attachments.some((attachment) => attachment.title === 'latest.json'),
        )
        if (!published) return json({ status: -1, message: '接口方法异常' })
        return json({ version: MANIFEST_VERSION, notes: 'notes', platforms: {} })
    }
    throw new Error('Unexpected request: ' + method + ' ' + value)
}
`
}

function runScript(args, { preloader, env = {} }) {
    return spawnSync(
        process.execPath,
        ['--import', 'data:text/javascript,' + encodeURIComponent(preloader), SCRIPT, ...args],
        {
            encoding: 'utf8',
            timeout: 120000,
            env: {
                ...process.env,
                GITHUB_REPOSITORY: 'Example/Fork',
                GITHUB_API_BASE_URL: '',
                GITLINK_BASE_URL: '',
                GITLINK_REPO_SLUG: '',
                GITLINK_TOKEN: '',
                GITLINK_TEST_STATE: '',
                ...env,
            },
        },
    )
}

test('GitLink keeps the newest stable and beta and ignores drafts and excluded tags', () => {
    const entry = (tag_name, { prerelease = false, draft = false, created_at }) => ({
        tag_name,
        prerelease,
        draft,
        created_at,
    })
    const releases = [
        entry('v1.10.0', { created_at: '2026-10-23T10:00:00Z' }),
        entry('v1.11.0', { draft: true, created_at: '2026-10-26T10:00:00Z' }),
        entry('v1.9.9', { created_at: '2026-10-25T10:00:00Z' }),
        entry('v1.8.0', { created_at: '2026-09-01T10:00:00Z' }),
        entry('v1.10.0-beta.5', { prerelease: true, created_at: '2026-10-22T10:00:00Z' }),
        entry('v1.10.0-rc.1', { prerelease: true, created_at: '2026-10-24T10:00:00Z' }),
        entry('v1.12.0-beta.1', {
            prerelease: true,
            draft: true,
            created_at: '2026-10-27T10:00:00Z',
        }),
    ]
    const slots = selectReleaseSlots(releases)
    assert.equal(slots.stable.tag_name, 'v1.10.0')
    assert.equal(slots.beta.tag_name, 'v1.10.0-rc.1')
    assert.deepEqual(computeAllowedTags(releases).sort(), ['v1.10.0', 'v1.10.0-rc.1'])
    assert.deepEqual(computeAllowedTags([]), [])
})

test('Existing GitLink attachments are deduplicated by title', () => {
    const titles = collectAttachmentTitles({
        attachments: [{ title: INSTALLER }, { title: MANIFEST }, { title: INSTALLER }],
    })
    assert.deepEqual([...titles].sort(), [INSTALLER, MANIFEST].sort())
    const missing = selectMissingAssets(
        [{ name: INSTALLER }, { name: 'extra.msi' }, { name: MANIFEST }],
        titles,
    )
    assert.deepEqual(
        missing.map((asset) => asset.name),
        ['extra.msi'],
    )
    assert.deepEqual(selectMissingAssets([{ name: INSTALLER }], new Set()).length, 1)
})

test('GitLink business failures surface even though the HTTP call succeeds', () => {
    assert.throws(
        () => assertGitlinkStatus({ status: -1, message: '接口方法异常' }, 'POST release v1.10.0'),
        /POST release v1\.10\.0 failed: 接口方法异常/,
    )
    assert.throws(
        () => assertGitlinkStatus({ message: '名称不能为空' }, 'PUT release v1.10.0'),
        /名称不能为空/,
    )
    assert.equal(assertGitlinkStatus({ status: 0, message: '发布成功' }, 'PUT release').status, 0)
})

test('Release payloads always send a name because PUT rejects an empty one', () => {
    const payload = buildReleasePayload({
        release: { name: '   ', body: 'notes' },
        tag: 'v1.10.0',
        attachmentIds: ['upload-0', ''],
        prerelease: false,
    })
    assert.equal(payload.name, 'Axolotl Launcher v1.10.0')
    assert.equal(payload.tag_name, 'v1.10.0')
    assert.equal(payload.target_commitish, 'main')
    assert.equal(payload.draft, false)
    assert.equal(payload.prerelease, false)
    assert.deepEqual(payload.attachment_ids, ['upload-0'])
    validateReleasePayload(payload)
    assert.throws(() => validateReleasePayload({ tag_name: 'v1.10.0' }), /non-empty name/)
    assert.throws(() => validateReleasePayload({ ...payload, name: '' }), /non-empty name/)
})

test('Pruning only removes version-shaped tags outside the allowed slots', () => {
    const releases = [
        gitlinkRelease(1, 'v1.10.0', []),
        gitlinkRelease(2, 'v1.10.0-beta.5', []),
        gitlinkRelease(3, 'v1.9.0', []),
        gitlinkRelease(4, '1.8.0', []),
        gitlinkRelease(5, 'state', []),
        gitlinkRelease(6, 'probe-a', []),
        gitlinkRelease(7, 'v1.9.9', []),
    ]
    const candidates = selectPruneCandidates(releases, ['v1.10.0', 'v1.10.0-beta.5'])
    assert.deepEqual(
        candidates.map((release) => release.tag_name),
        ['v1.9.0', '1.8.0', 'v1.9.9'],
    )
    assert.deepEqual(
        selectPruneCandidates(
            releases,
            releases.map((r) => r.tag_name),
        ),
        [],
    )
})

test('Prerelease state follows the tag pattern over the GitHub flag', () => {
    assert.equal(isPrereleaseTag('v1.10.0'), false)
    assert.equal(isPrereleaseTag('v1.10.0-beta.5'), true)
    assert.equal(isPrereleaseTag('v1.10.0-rc.1'), true)
    assert.equal(isPrereleaseTag('v1.10.0-nightly'), true)
    assert.deepEqual(resolvePrerelease({ prerelease: false }, 'v1.10.0-beta.5'), {
        declared: false,
        byTag: true,
        prerelease: true,
        mismatch: true,
    })
    assert.deepEqual(resolvePrerelease({ prerelease: true }, 'v1.10.0'), {
        declared: true,
        byTag: false,
        prerelease: false,
        mismatch: true,
    })
    assert.deepEqual(resolvePrerelease({ prerelease: true }, 'v1.10.0-beta.5'), {
        declared: true,
        byTag: true,
        prerelease: true,
        mismatch: false,
    })
})

test('Duplicate GitLink tags prefer the published record', () => {
    const releases = [
        gitlinkRelease(1, 'v1.10.0', [], { draft: '草稿' }),
        gitlinkRelease(2, 'v1.10.0', []),
    ]
    assert.equal(findReleaseByTag(releases, 'v1.10.0').version_id, 2)
    assert.equal(findReleaseByTag(releases, 'v1.11.0'), null)
})

test('Downloaded assets are rejected when size or digest disagrees', () => {
    const sha256 = 'a'.repeat(64)
    verifyDownloadedAsset(
        { name: INSTALLER, size: 3, digest: 'sha256:' + sha256 },
        {
            bytes: 3,
            sha256,
        },
    )
    assert.throws(
        () => verifyDownloadedAsset({ name: INSTALLER, size: 3 }, { bytes: 4, sha256 }),
        /has 4 bytes, expected 3/,
    )
    assert.throws(
        () =>
            verifyDownloadedAsset(
                { name: INSTALLER, digest: 'sha256:' + sha256 },
                { bytes: 3, sha256: 'b'.repeat(64) },
            ),
        /does not match/,
    )
})

test('GitLink mirror appends missing assets, verifies the public release, and prunes', (t) => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-gitlink-test-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    const tag = 'v1.10.0'
    const fixture = createFixture(tag)
    const statePath = path.join(directory, 'state.json')
    const outputDirectory = path.join(directory, 'gitlink-release', 'final')
    const preloader = createStub({
        tag,
        fixture,
        gitlinkReleases: [
            gitlinkRelease(2560, tag, [MANIFEST]),
            gitlinkRelease(2570, 'v1.9.0', []),
            gitlinkRelease(2580, 'state', []),
        ],
    })
    const result = runScript(['finalize', tag, outputDirectory], {
        preloader,
        env: { GITLINK_TOKEN: 'test-token', GITLINK_TEST_STATE: statePath },
    })
    assert.equal(result.status, 0, result.stderr)

    const state = JSON.parse(fs.readFileSync(statePath, 'utf8'))
    assert.deepEqual(state.uploadedTitles, [INSTALLER])
    assert.equal(state.posts.length, 0)
    assert.equal(state.puts.length, 1)
    const put = state.puts[0]
    assert.equal(put.name, 'Axolotl Launcher ' + tag)
    assert.equal(put.tag_name, tag)
    assert.equal(put.target_commitish, 'main')
    assert.equal(put.draft, false)
    assert.equal(put.prerelease, false)
    assert.deepEqual(put.attachment_ids, ['upload-0'])
    assert.deepEqual(state.deletes, [2570])
    assert.deepEqual(
        state.requests
            .filter((request) => request.url === RELEASES_URL)
            .map((request) => request.authorization),
        ['present', 'absent', 'present'],
    )

    const summary = JSON.parse(
        fs.readFileSync(path.join(outputDirectory, 'gitlink-release.json'), 'utf8'),
    )
    assert.equal(summary.tag, tag)
    assert.equal(summary.version_id, 2560)
    assert.equal(summary.assetCount, 2)
    assert.deepEqual(summary.uploaded, [INSTALLER])
    assert.deepEqual(summary.pruned, ['v1.9.0'])
    assert.deepEqual(summary.allowed.sort(), ['v1.10.0', 'v1.10.0-beta.5'])
    assert.ok(Number.isFinite(Date.parse(summary.verifiedAt)))
})

test('GitLink creates beta releases with the tag-derived flag and retries uploads', (t) => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-gitlink-test-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    const tag = 'v1.10.0-beta.5'
    const fixture = createFixture(tag)
    // GitHub reports this beta as a stable release; the tag pattern has to win.
    fixture.release.prerelease = false
    const statePath = path.join(directory, 'state.json')
    const outputDirectory = path.join(directory, 'final')
    const preloader = createStub({
        tag,
        fixture,
        gitlinkReleases: [
            gitlinkRelease(2560, 'v1.10.0', [MANIFEST]),
            gitlinkRelease(2570, 'v1.9.0', []),
        ],
        uploadFailures: 1,
    })
    const result = runScript(['finalize', tag, outputDirectory], {
        preloader,
        env: { GITLINK_TOKEN: 'test-token', GITLINK_TEST_STATE: statePath },
    })
    assert.equal(result.status, 0, result.stderr)
    assert.match(result.stderr, /using the tag-derived prerelease=true/)

    const state = JSON.parse(fs.readFileSync(statePath, 'utf8'))
    assert.equal(state.failedUploads, 1)
    assert.deepEqual(state.uploadedTitles.sort(), [INSTALLER, MANIFEST].sort())
    assert.equal(state.posts.length, 1)
    assert.equal(state.puts.length, 0)
    assert.equal(state.posts[0].prerelease, true)
    assert.equal(state.posts[0].draft, false)
    assert.equal(state.posts[0].tag_name, tag)
    assert.equal(state.posts[0].attachment_ids.length, 2)
    assert.deepEqual(state.deletes, [2570])

    const summary = JSON.parse(
        fs.readFileSync(path.join(outputDirectory, 'gitlink-release.json'), 'utf8'),
    )
    assert.equal(summary.version_id, 9001)
    assert.equal(summary.assetCount, 2)
    assert.deepEqual(summary.pruned, ['v1.9.0'])
})

test('GitLink mirror aborts when a write returns status !== 0', (t) => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-gitlink-test-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    const tag = 'v1.10.0'
    const fixture = createFixture(tag)
    const outputDirectory = path.join(directory, 'final')
    const preloader = createStub({
        tag,
        fixture,
        gitlinkReleases: [gitlinkRelease(2560, tag, [MANIFEST])],
        writeStatus: { status: -1, message: '名称不能为空' },
    })
    const result = runScript(['finalize', tag, outputDirectory], {
        preloader,
        env: {
            GITLINK_TOKEN: 'test-token',
            GITLINK_TEST_STATE: path.join(directory, 'state.json'),
        },
    })
    assert.notEqual(result.status, 0)
    assert.match(result.stderr, /PUT release v1\.10\.0 failed: 名称不能为空/)
    assert.equal(fs.existsSync(path.join(outputDirectory, 'gitlink-release.json')), false)
})

test('GitLink verify reads the public release without a token', (t) => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-gitlink-test-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    const tag = 'v1.10.0'
    const fixture = createFixture(tag)
    const statePath = path.join(directory, 'state.json')
    const preloader = createStub({
        tag,
        fixture,
        gitlinkReleases: [gitlinkRelease(2560, tag, [INSTALLER, MANIFEST])],
    })
    const result = runScript(['verify', tag, '2'], {
        preloader,
        env: { GITLINK_TEST_STATE: statePath },
    })
    assert.equal(result.status, 0, result.stderr)
    assert.match(result.stdout, /attachments=2/)
    assert.ok(result.stdout.includes('latest.json version=' + fixture.version), result.stdout)

    const state = JSON.parse(fs.readFileSync(statePath, 'utf8'))
    assert.deepEqual(
        state.requests.map((request) => request.authorization),
        state.requests.map(() => 'absent'),
    )
    assert.deepEqual(state.uploadedTitles, [])
})

test('GitLink verify fails for missing, draft, and under-populated releases', (t) => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-gitlink-test-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    const tag = 'v1.10.0'
    const fixture = createFixture(tag)
    const run = (gitlinkReleases, args = ['verify', tag]) =>
        runScript(args, {
            preloader: createStub({ tag, fixture, gitlinkReleases }),
            env: { GITLINK_TEST_STATE: path.join(directory, 'state.json') },
        })

    const missing = run([])
    assert.notEqual(missing.status, 0)
    assert.match(missing.stderr, /is missing from/)

    const draft = run([gitlinkRelease(2560, tag, [MANIFEST], { draft: '草稿' })])
    assert.notEqual(draft.status, 0)
    assert.match(draft.stderr, /still a draft/)

    const short = run([gitlinkRelease(2560, tag, [MANIFEST])], ['verify', tag, '5'])
    assert.notEqual(short.status, 0)
    assert.match(short.stderr, /has 1 attachments, expected at least 5/)
})
