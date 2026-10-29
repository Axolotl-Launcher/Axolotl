import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { finalizeGithubRelease } from './finalize-github-release.mjs'
import { assetDownloadUrl } from './release-utils.mjs'

const tag = 'v1.10.0-beta.5'
const names = [
    'Axolotl_universal.app.tar.gz',
    'Axolotl_aarch64.AppImage.tar.gz',
    'Axolotl_amd64.AppImage.tar.gz',
    'Axolotl_x64-setup.nsis.zip',
]

function fixture(t) {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-finalize-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    const state = {
        release: {
            id: 123,
            tag_name: tag,
            draft: true,
            created_at: '2026-10-22T10:00:00Z',
            assets: names
                .flatMap((name) => [name, `${name}.sig`])
                .map((name) => ({
                    name,
                    digest: `sha256:${createHash('sha256').update(name).digest('hex')}`,
                    browser_download_url: `https://github.com/Example/Axolotl/releases/download/untagged-example/${name}`,
                })),
        },
        manifest: undefined,
        calls: [],
        omitSignature: false,
    }
    const run = (args) => {
        state.calls.push(args)
        if (args[0] === 'api') {
            return JSON.stringify(args[1].includes('?') ? [state.release] : state.release)
        }
        assert.deepEqual(args.slice(2, 5), [
            tag,
            '--repo',
            process.env.GITHUB_REPOSITORY || 'Axolotl-Launcher/Axolotl',
        ])
        if (args[1] === 'download') {
            const output = args[args.indexOf('--dir') + 1]
            if (args.includes('*.sig')) {
                for (const name of names.slice(state.omitSignature ? 1 : 0)) {
                    fs.writeFileSync(
                        path.join(output, `${name}.sig`),
                        `${'signature'.repeat(12)}\n`,
                    )
                }
            } else {
                fs.writeFileSync(path.join(output, 'latest.json'), state.manifest)
            }
        } else if (args[1] === 'upload') {
            state.manifest = fs.readFileSync(args[5], 'utf8')
            state.release.assets.push({ name: 'latest.json' })
        } else if (args[1] === 'edit') {
            assert(args.includes('--prerelease'))
            assert(args.includes('--latest=false'))
            state.release.draft = false
            state.release.published_at = '2026-10-22T12:00:00Z'
            for (const asset of state.release.assets)
                asset.browser_download_url = assetDownloadUrl(tag, asset.name)
        } else {
            assert.fail(`Unexpected gh command: ${args}`)
        }
        return ''
    }
    return { directory, state, run }
}

test('publish draft assets, refresh public metadata and preserve the manifest on retry', (t) => {
    const { directory, state, run } = fixture(t)
    const release = finalizeGithubRelease(tag, directory, { run })
    assert.equal(release.draft, false)
    assert.equal(release.assets.length, 9)
    assert.equal(
        JSON.parse(state.manifest).platforms['darwin-aarch64'].url,
        assetDownloadUrl(tag, names[0]),
    )
    assert.equal(
        JSON.parse(fs.readFileSync(path.join(directory, 'release.json'))).published_at,
        release.published_at,
    )
    const original = state.manifest
    const mutationCount = () =>
        state.calls.filter((args) => ['upload', 'edit'].includes(args[1])).length
    assert.equal(mutationCount(), 2)
    finalizeGithubRelease(tag, directory, { run, publishedOnly: true })
    finalizeGithubRelease(tag, directory, { run })
    assert.equal(mutationCount(), 2)
    assert.equal(fs.readFileSync(path.join(directory, 'latest.json'), 'utf8'), original)
})

test('downstream-only recovery refuses a draft without mutating it', (t) => {
    const { directory, state, run } = fixture(t)
    assert.throws(
        () => finalizeGithubRelease(tag, directory, { run, publishedOnly: true }),
        /must be published/,
    )
    assert.equal(state.calls.length, 2)
})

test('missing signatures stop publication before uploading a manifest', (t) => {
    const { directory, state, run } = fixture(t)
    state.omitSignature = true
    assert.throws(() => finalizeGithubRelease(tag, directory, { run }), /Missing updater signature/)
    assert(!state.calls.some((args) => ['upload', 'edit'].includes(args[1])))
})

test('a damaged published manifest is not silently overwritten', (t) => {
    const { directory, state, run } = fixture(t)
    finalizeGithubRelease(tag, directory, { run })
    const manifest = JSON.parse(state.manifest)
    manifest.platforms['darwin-aarch64'].signature = 'different signature'.repeat(4)
    state.manifest = JSON.stringify(manifest)
    state.calls.length = 0
    assert.throws(() => finalizeGithubRelease(tag, directory, { run }), /does not match/)
    assert(!state.calls.some((args) => ['upload', 'edit'].includes(args[1])))
})

test('a published manifest without SHA-256 entries is preserved', (t) => {
    const { directory, state, run } = fixture(t)
    finalizeGithubRelease(tag, directory, { run })
    const manifest = JSON.parse(state.manifest)
    for (const update of Object.values(manifest.platforms)) delete update.sha256
    state.manifest = JSON.stringify(manifest)
    state.calls.length = 0
    const logs = []
    const log = console.log
    console.log = (message) => logs.push(String(message))
    try {
        finalizeGithubRelease(tag, directory, { run })
    } finally {
        console.log = log
    }
    assert(!state.calls.some((args) => ['upload', 'edit'].includes(args[1])))
    assert.equal(fs.readFileSync(path.join(directory, 'latest.json'), 'utf8'), state.manifest)
    for (const platform of Object.keys(manifest.platforms))
        assert(logs.includes(`Preserved published manifest without SHA-256 for ${platform}`))
})

test('a published manifest with a divergent SHA-256 is rejected', (t) => {
    const { directory, state, run } = fixture(t)
    finalizeGithubRelease(tag, directory, { run })
    const manifest = JSON.parse(state.manifest)
    manifest.platforms['windows-x86_64'].sha256 = createHash('sha256').update('other').digest('hex')
    state.manifest = JSON.stringify(manifest)
    state.calls.length = 0
    assert.throws(() => finalizeGithubRelease(tag, directory, { run }), /does not match/)
    assert(!state.calls.some((args) => ['upload', 'edit'].includes(args[1])))
})

test('excluded or malformed tags cannot issue GitHub commands', (t) => {
    const { directory, state, run } = fixture(t)
    for (const value of [
        'v1.9.9',
        'main',
        'v1.2.3/../other',
        'v1.2.3\n',
        'v01.2.3',
        'v1.2.3-beta.01',
    ]) {
        assert.throws(
            () => finalizeGithubRelease(value, directory, { run }),
            /eligible version tag/,
        )
    }
    assert.equal(state.calls.length, 0)
})
