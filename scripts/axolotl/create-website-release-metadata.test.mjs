import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { assetDownloadUrl } from './release-utils.mjs'

const digest = (character) => `sha256:${character.repeat(64)}`

const asset = (tag, name, character = 'a') => ({
    name,
    browser_download_url: assetDownloadUrl(tag, name),
    digest: digest(character),
})

const release = (tag, assets) => ({
    tag_name: tag,
    draft: false,
    published_at: '2026-10-22T12:00:00Z',
    assets,
})

const temporaryDirectory = (t) => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-website-release-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    return directory
}

const run = (script, args) =>
    spawnSync(process.execPath, [path.join(import.meta.dirname, script), ...args], {
        encoding: 'utf8',
    })

test('website recovery records asset names with SHA-256, preserves newer downloads and uses the requested announcement source', (t) => {
    const directory = temporaryDirectory(t)
    const releasePath = path.join(directory, 'release.json')
    const outputPath = path.join(directory, 'latest.json')
    fs.writeFileSync(
        outputPath,
        `${JSON.stringify({ tag_name: 'v1.10.0', assets: [{ name: 'newer.exe', sha256: digest('f') }] }, null, 2)}\n`,
    )
    const original = fs.readFileSync(outputPath, 'utf8')
    fs.writeFileSync(
        releasePath,
        JSON.stringify(release('v1.9.8', [asset('v1.9.8', 'installer.exe')])),
    )
    const older = run('create-website-release-metadata.mjs', [releasePath, outputPath])
    assert.equal(older.status, 0, older.stderr)
    assert.equal(fs.readFileSync(outputPath, 'utf8'), original)
    assert.match(older.stdout, /Preserved newer website release v1\.10\.0/)

    fs.writeFileSync(
        releasePath,
        JSON.stringify(
            release('v1.11.0', [
                asset('v1.11.0', 'Axolotl_1.11.0_x64-setup.exe', 'b'),
                asset('v1.11.0', 'Axolotl_1.11.0_universal.dmg', 'c'),
            ]),
        ),
    )
    const newer = run('create-website-release-metadata.mjs', [releasePath, outputPath])
    assert.equal(newer.status, 0, newer.stderr)
    const written = JSON.parse(fs.readFileSync(outputPath, 'utf8'))
    assert.equal(written.tag_name, 'v1.11.0')
    assert.deepEqual(written.assets, [
        { name: 'Axolotl_1.11.0_x64-setup.exe', sha256: 'b'.repeat(64) },
        { name: 'Axolotl_1.11.0_universal.dmg', sha256: 'c'.repeat(64) },
    ])
    assert.match(newer.stdout, /\(2 assets\)/)

    fs.writeFileSync(
        releasePath,
        JSON.stringify(release('v1.12.0-beta.1', [asset('v1.12.0-beta.1', 'installer.exe')])),
    )
    assert.notEqual(run('create-website-release-metadata.mjs', [releasePath, outputPath]).status, 0)

    const sourcePath = path.join(directory, 'announcements.ts')
    const catalogPath = path.join(directory, 'catalog.json')
    fs.writeFileSync(
        sourcePath,
        'export const launcherAnnouncements = [{ id: "fixture", version: "1.11.0" }]',
    )
    const catalog = run('create-website-release-catalog.mjs', [catalogPath, sourcePath])
    assert.equal(catalog.status, 0, catalog.stderr)
    assert.equal(JSON.parse(fs.readFileSync(catalogPath, 'utf8')).announcements[0].id, 'fixture')
})

for (const tag of ['v1.9.8', 'v1.10.0']) {
    test(`website metadata replaces the excluded baseline with ${tag}`, (t) => {
        const directory = temporaryDirectory(t)
        const releasePath = path.join(directory, 'release.json')
        const outputPath = path.join(directory, 'latest.json')
        fs.writeFileSync(outputPath, JSON.stringify({ tag_name: 'v1.9.9', assets: [] }))
        fs.writeFileSync(releasePath, JSON.stringify(release(tag, [asset(tag, 'installer.exe')])))

        const result = run('create-website-release-metadata.mjs', [releasePath, outputPath])

        assert.equal(result.status, 0, result.stderr)
        assert.deepEqual(JSON.parse(fs.readFileSync(outputPath, 'utf8')), {
            tag_name: tag,
            assets: [{ name: 'installer.exe', sha256: 'a'.repeat(64) }],
        })
    })
}

test('website metadata rejects an excluded incoming release without changing downloads', (t) => {
    const directory = temporaryDirectory(t)
    const releasePath = path.join(directory, 'release.json')
    const outputPath = path.join(directory, 'latest.json')
    const original = JSON.stringify({ tag_name: 'v1.9.8', assets: [] })
    fs.writeFileSync(outputPath, original)
    fs.writeFileSync(releasePath, JSON.stringify(release('v1.9.9', [])))

    const result = run('create-website-release-metadata.mjs', [releasePath, outputPath])

    assert.notEqual(result.status, 0)
    assert.match(result.stderr, /Expected an eligible version tag, received v1\.9\.9/)
    assert.equal(fs.readFileSync(outputPath, 'utf8'), original)
})

test('website metadata fails when an asset has no SHA-256 digest', (t) => {
    const directory = temporaryDirectory(t)
    const releasePath = path.join(directory, 'release.json')
    const outputPath = path.join(directory, 'latest.json')
    fs.writeFileSync(
        releasePath,
        JSON.stringify(
            release('v1.13.0', [
                asset('v1.13.0', 'ok.exe'),
                {
                    name: 'installer.exe',
                    browser_download_url: assetDownloadUrl('v1.13.0', 'installer.exe'),
                },
            ]),
        ),
    )
    const missing = run('create-website-release-metadata.mjs', [releasePath, outputPath])
    assert.notEqual(missing.status, 0)
    assert.match(missing.stderr, /installer\.exe/)
    assert.match(missing.stderr, /missing a SHA-256 digest/)
    assert.equal(fs.existsSync(outputPath), false)
})
