import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import { readReleaseNotice, validateReleaseNotices } from './release-notices.mjs'

const script = fileURLToPath(new URL('./create-release-notes.mjs', import.meta.url))
const notice = {
    type: 'important',
    'zh-CN': '中文提示 **Markdown**\n\n第二段\n- 一项',
    'en-US': 'English notice\r\n\r\nSecond paragraph',
}
const announcement = {
    version: '2.3.4-beta.1',
    title: { 'zh-CN': 'Test release', 'en-US': 'Test release' },
    publishedAt: '2026-01-01',
    changes: { fixed: [{ 'zh-CN': '修复条目', 'en-US': 'A fix' }] },
    notes: { 'zh-CN': '原有说明', 'en-US': 'Existing notes' },
}

async function fixture(t, config) {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'axolotl-release-notes-test-'))
    t.after(async () => {
        assert.equal(path.dirname(directory), path.resolve(os.tmpdir()))
        assert.ok(path.basename(directory).startsWith('axolotl-release-notes-test-'))
        await fs.rm(directory, { recursive: true, force: true })
    })
    await fs.mkdir(path.join(directory, 'apps/app-frontend/src/announcements'), { recursive: true })
    await fs.mkdir(path.join(directory, '.github'))
    await fs.writeFile(
        path.join(directory, 'apps/app-frontend/src/announcements/catalog.ts'),
        `
        export const ANNOUNCEMENT_CHANGE_TYPES = ['added', 'changed', 'deprecated', 'removed', 'fixed', 'security']
        const announcement = ${JSON.stringify(announcement)}
        export function getAnnouncementByVersion(version: string) {
            return version === announcement.version ? announcement : null
        }
    `,
    )
    const noticePath = path.join(directory, '.github/release-notices.json')
    if (config !== undefined) await fs.writeFile(noticePath, JSON.stringify(config))
    const output = path.join(directory, 'notes.md')
    const run = (tag = 'v2.3.4-beta.1') =>
        spawnSync(process.execPath, [script, tag, output], {
            cwd: directory,
            encoding: 'utf8',
            timeout: 15000,
        })
    return { directory, noticePath, output, run }
}

const originalBody = `# Test release

发布日期 / Published: 2026-01-01

## 中文

### Bug 修复

- 修复条目

### 说明

原有说明

## English

### Fixed

- A fix

### Notes

Existing notes
`

for (const [label, config] of [
    ['missing configuration file', undefined],
    ['empty notices', { notices: {} }],
    ['notice for a different tag', { notices: { 'v2.3.4': notice } }],
]) {
    test(`${label} preserves the existing release body exactly`, async (t) => {
        const { run, output } = await fixture(t, config)
        const result = run()
        assert.equal(result.status, 0, result.stderr)
        assert.equal(await fs.readFile(output, 'utf8'), originalBody)
    })
}

for (const tag of ['v2.3.4-beta.1', '2.3.4-beta.1']) {
    test(`notice for ${tag} appears before changes and preserves existing notes`, async (t) => {
        const { run, output } = await fixture(t, { notices: { 'v2.3.4-beta.1': notice } })
        const result = run(tag)
        assert.equal(result.status, 0, result.stderr)
        const body = await fs.readFile(output, 'utf8')
        const block =
            '> [!IMPORTANT]\n> 中文提示 **Markdown**\n>\n> 第二段\n> - 一项\n>\n> English notice\n>\n> Second paragraph\n\n'
        assert.equal(body, originalBody.replace('## 中文', `${block}## 中文`))
        assert.equal(body.replace(block, ''), originalBody)
    })
}

for (const type of ['note', 'tip', 'important', 'warning', 'caution']) {
    test(`supports GitHub ${type} alerts`, async (t) => {
        const { run, output } = await fixture(t, {
            notices: { 'v2.3.4-beta.1': { ...notice, type } },
        })
        const result = run()
        assert.equal(result.status, 0, result.stderr)
        assert.ok((await fs.readFile(output, 'utf8')).includes(`> [!${type.toUpperCase()}]`))
    })
}

for (const [label, config] of [
    ['missing notices', {}],
    ['array instead of object', []],
    ['null notices', { notices: null }],
    ['unknown root key', { notices: {}, typo: 'x' }],
    ['invalid schema reference', { notices: {}, $schema: '' }],
    ['tag without v', { notices: { '2.3.4': notice } }],
    ['invalid tag', { notices: { latest: notice } }],
    ['missing Chinese', { notices: { 'v2.3.4': { type: 'note', 'en-US': 'English' } } }],
    ['blank English', { notices: { 'v2.3.4': { ...notice, 'en-US': ' \n ' } } }],
    ['unknown type', { notices: { 'v2.3.4': { ...notice, type: 'alert' } } }],
    ['unknown notice field', { notices: { 'v2.3.4': { ...notice, enabled: true } } }],
]) {
    test(`rejects ${label} even for another release and writes no output`, async (t) => {
        const { run, output } = await fixture(t, config)
        const result = run()
        assert.notEqual(result.status, 0)
        assert.match(result.stderr, /Invalid .github\/release-notices.json/)
        await assert.rejects(fs.access(output), { code: 'ENOENT' })
    })
}

test('rejects malformed JSON before writing release notes', async (t) => {
    const { run, noticePath, output } = await fixture(t)
    await fs.writeFile(noticePath, '{invalid JSON')
    const result = run()
    assert.notEqual(result.status, 0)
    assert.match(result.stderr, /Invalid .github\/release-notices.json/)
    await assert.rejects(fs.access(output), { code: 'ENOENT' })
})

test('a notice cannot replace a missing launcher announcement', async (t) => {
    const { run, output } = await fixture(t, { notices: { 'v2.3.4': notice } })
    const result = run('v2.3.4')
    assert.notEqual(result.status, 0)
    assert.match(result.stderr, /No bundled announcement found for release 2.3.4/)
    await assert.rejects(fs.access(output), { code: 'ENOENT' })
})

test('checked-in notice configuration is valid', async () => {
    const filename = new URL('../../.github/release-notices.json', import.meta.url)
    validateReleaseNotices(JSON.parse(await fs.readFile(filename, 'utf8')))
    assert.equal(await readReleaseNotice('v0.0.0-not-configured', filename), null)
})
