import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { gzipSync } from 'node:zlib'

import {
    blockbenchBuildInputs,
    buildBlockbenchSkinEditor,
} from './build-blockbench-skin-editor.mjs'
import { root } from './desktop-build-utils.mjs'

function fixture(t) {
    const repositoryRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-blockbench-'))
    t.after(() => fs.rmSync(repositoryRoot, { recursive: true, force: true }))
    const blockbench = path.join(repositoryRoot, 'third-party/blockbench')
    const outDir = path.join(repositoryRoot, 'out')
    const destination = path.join(repositoryRoot, 'apps/app/resources/blockbench-skin')
    const scripts = path.join(repositoryRoot, 'scripts/axolotl')
    fs.mkdirSync(scripts, { recursive: true })
    for (const name of [
        'build-blockbench-skin-editor.mjs',
        'desktop-build-utils.mjs',
        'sync-blockbench-skin-editor.mjs',
    ]) {
        fs.copyFileSync(path.join(root, 'scripts/axolotl', name), path.join(scripts, name))
    }
    for (const input of [...blockbenchBuildInputs, 'css', 'font', 'index.html']) {
        const file = path.join(blockbench, input)
        if (input.includes('.')) {
            fs.mkdirSync(path.dirname(file), { recursive: true })
            fs.writeFileSync(file, '{}')
        } else {
            fs.mkdirSync(file, { recursive: true })
            fs.writeFileSync(path.join(file, 'input.txt'), input)
        }
    }
    fs.mkdirSync(path.join(blockbench, 'node_modules'))
    const invocations = []
    let fail
    const execute = (command, args, options) => {
        invocations.push([command, ...args])
        if (fail === command) throw new Error('Injected build failure')
        if (command === 'npm') {
            fs.mkdirSync(path.join(blockbench, 'dist'), { recursive: true })
            fs.writeFileSync(path.join(blockbench, 'dist/skin.bundle.js'), 'deterministic bundle')
        } else {
            const result = spawnSync(command, args, { ...options, encoding: 'utf8' })
            assert.equal(result.status, 0, result.stderr)
        }
    }
    return {
        blockbench,
        destination,
        scripts,
        receipt: path.join(outDir, 'blockbench-skin-inputs.json'),
        build() {
            invocations.length = 0
            buildBlockbenchSkinEditor({ repositoryRoot, outDir, execute })
            return invocations.filter(([command]) => command === 'npm').length
        },
        failWith(command) {
            fail = command
        },
    }
}

test('unchanged content stays fresh even when synced gzip is older than the bundle', (t) => {
    const f = fixture(t)
    assert.equal(f.build(), 1)
    const gzip = path.join(f.destination, 'dist/skin.bundle.js.gz')
    const old = new Date(2000, 0, 1)
    fs.utimesSync(gzip, old, old)
    const receiptTime = fs.statSync(f.receipt).mtimeMs
    assert.equal(f.build(), 0)
    assert.equal(fs.statSync(gzip).mtimeMs, old.getTime())
    assert.equal(fs.statSync(f.receipt).mtimeMs, receiptTime)
})

test('input timestamps alone do not rebuild, but content changes at the same timestamp do', (t) => {
    const f = fixture(t)
    f.build()
    for (const input of ['js', 'lang', 'themes', 'keymaps']) {
        const file = path.join(f.blockbench, input, 'input.txt')
        const future = new Date(2035, 0, 1)
        fs.utimesSync(file, future, future)
        assert.equal(f.build(), 0)
        fs.writeFileSync(file, 'changed')
        fs.utimesSync(file, future, future)
        assert.equal(f.build(), 1, input)
        assert.equal(f.build(), 0)
    }
})

test('sync repairs missing, corrupt and stale output without rebuilding the bundle', (t) => {
    const f = fixture(t)
    f.build()
    fs.rmSync(path.join(f.destination, 'css'), { recursive: true })
    fs.writeFileSync(path.join(f.destination, 'font/input.txt'), 'corrupt')
    fs.writeFileSync(path.join(f.destination, 'dist/skin.bundle.js.gz'), 'corrupt')
    fs.writeFileSync(path.join(f.destination, 'stale.txt'), 'stale')
    assert.equal(f.build(), 0)
    assert.equal(fs.readFileSync(path.join(f.destination, 'css/input.txt'), 'utf8'), 'css')
    assert.equal(fs.readFileSync(path.join(f.destination, 'font/input.txt'), 'utf8'), 'font')
    assert.deepEqual(
        fs.readFileSync(path.join(f.destination, 'dist/skin.bundle.js.gz')),
        gzipSync('deterministic bundle', { level: 9 }),
    )
    assert.equal(fs.existsSync(path.join(f.destination, 'stale.txt')), false)
})

test('sync-only inputs and script changes are applied without an npm rebuild', (t) => {
    const f = fixture(t)
    f.build()
    fs.writeFileSync(path.join(f.blockbench, 'css/input.txt'), 'updated css')
    fs.appendFileSync(path.join(f.scripts, 'sync-blockbench-skin-editor.mjs'), '\n// changed\n')
    assert.equal(f.build(), 0)
    assert.equal(fs.readFileSync(path.join(f.destination, 'css/input.txt'), 'utf8'), 'updated css')
})

test('missing or modified bundle and malformed receipts require a successful rebuild', (t) => {
    const f = fixture(t)
    f.build()
    const bundle = path.join(f.blockbench, 'dist/skin.bundle.js')
    fs.rmSync(bundle)
    assert.equal(f.build(), 1)
    fs.writeFileSync(bundle, 'other build')
    assert.equal(f.build(), 1)
    fs.writeFileSync(f.receipt, 'invalid json')
    assert.equal(f.build(), 1)
    assert.equal(f.build(), 0)
})

test('failed build or sync cannot advance the freshness receipt', (t) => {
    const f = fixture(t)
    f.build()
    const receipt = fs.readFileSync(f.receipt, 'utf8')
    fs.writeFileSync(path.join(f.blockbench, 'js/input.txt'), 'changed')
    f.failWith('npm')
    assert.throws(() => f.build(), /Injected build failure/)
    assert.equal(fs.readFileSync(f.receipt, 'utf8'), receipt)
    f.failWith(process.execPath)
    assert.throws(() => f.build(), /Injected build failure/)
    assert.equal(fs.readFileSync(f.receipt, 'utf8'), receipt)
    f.failWith(undefined)
    assert.equal(f.build(), 1)
    assert.equal(f.build(), 0)
})
