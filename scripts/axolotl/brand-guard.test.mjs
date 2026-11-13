import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const guard = fileURLToPath(new URL('./brand-guard.mjs', import.meta.url))

async function runGuard(
    permissions,
    frontendConfig = 'Axolotl-Launcher/${version} (${os})\n',
) {
    const directory = await mkdtemp(path.join(tmpdir(), 'axolotl-brand-guard-'))
    try {
        for (const relative of [
            'apps/app/capabilities',
            'apps/app/src',
            'apps/app-frontend/src',
            'packages/app-lib',
        ]) {
            await mkdir(path.join(directory, relative), { recursive: true })
        }
        const files = {
            'apps/app/tauri.conf.json': JSON.stringify({
                productName: 'Axolotl Launcher',
                identifier: 'red.ghs.axolotl',
                plugins: { 'deep-link': { desktop: { schemes: ['axolotl'] } } },
            }),
            'apps/app-frontend/src/config.ts': frontendConfig,
            'apps/app/src/main.rs': '',
            'apps/app/build.rs': '',
            'apps/app/capabilities/plugins.json': JSON.stringify({ permissions }),
        }
        for (const [relative, contents] of Object.entries(files)) {
            await writeFile(path.join(directory, relative), contents)
        }
        return spawnSync(process.execPath, [guard], { cwd: directory, encoding: 'utf8' })
    } finally {
        await rm(directory, { recursive: true, force: true })
    }
}

test('rejects private login permissions in string and scoped object forms', async (t) => {
    for (const permission of ['mr-auth:default', { identifier: 'mr-auth:default', allow: [] }]) {
        await t.test(JSON.stringify(permission), async () => {
            const result = await runGuard([permission])
            assert.equal(result.status, 1, result.stderr)
            assert.match(result.stderr, /private Modrinth login removed/)
        })
    }
})

test('allows unrelated string and scoped object permissions', async () => {
    const result = await runGuard([
        'auth:default',
        { identifier: 'http:default', allow: [{ url: 'https://api.modrinth.com/*' }] },
    ])
    assert.equal(result.status, 0, result.stderr)
    assert.match(result.stdout, /Axolotl brand guard passed/)
})

test('rejects a configuration that turns GHS telemetry on', async () => {
    const result = await runGuard(
        [],
        'Axolotl-Launcher/${version} (${os})\nghsTelemetry: true\n',
    )
    assert.equal(result.status, 1, result.stderr)
    assert.match(result.stderr, /GHS telemetry disabled/)
})
