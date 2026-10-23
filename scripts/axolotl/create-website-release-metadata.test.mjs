import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { assetDownloadUrl } from './release-utils.mjs'

test('website recovery preserves newer stable downloads and uses the requested announcement source', (t) => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-website-release-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    const releasePath = path.join(directory, 'release.json')
    const outputPath = path.join(directory, 'latest.json')
    const run = (script, args) =>
        spawnSync(process.execPath, [path.join(import.meta.dirname, script), ...args], {
            encoding: 'utf8',
        })
    const release = (tag) => ({
        tag_name: tag,
        draft: false,
        published_at: '2026-10-22T12:00:00Z',
        assets: [
            { name: 'installer.exe', browser_download_url: assetDownloadUrl(tag, 'installer.exe') },
        ],
    })
    fs.writeFileSync(outputPath, JSON.stringify({ tag_name: 'v1.10.0', assets: ['newer.exe'] }))
    const original = fs.readFileSync(outputPath, 'utf8')
    fs.writeFileSync(releasePath, JSON.stringify(release('v1.9.8')))
    const older = run('create-website-release-metadata.mjs', [releasePath, outputPath])
    assert.equal(older.status, 0, older.stderr)
    assert.equal(fs.readFileSync(outputPath, 'utf8'), original)
    fs.writeFileSync(releasePath, JSON.stringify(release('v1.11.0')))
    const newer = run('create-website-release-metadata.mjs', [releasePath, outputPath])
    assert.equal(newer.status, 0, newer.stderr)
    assert.equal(JSON.parse(fs.readFileSync(outputPath, 'utf8')).tag_name, 'v1.11.0')
    fs.writeFileSync(releasePath, JSON.stringify(release('v1.12.0-beta.1')))
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
