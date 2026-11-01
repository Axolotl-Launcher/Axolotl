import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { createLabPlugin, scaffoldFiles, toPascalCase } from './new-lab-plugin.mjs'

async function temporaryRoot(t) {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'axolotl-lab-plugin-test-'))
    t.after(async () => {
        assert.equal(path.dirname(directory), path.resolve(os.tmpdir()))
        await fs.rm(directory, { recursive: true, force: true })
    })
    return directory
}

test('toPascalCase turns a kebab-case id into a component name', () => {
    assert.equal(toPascalCase('motd-preview'), 'MotdPreview')
    assert.equal(toPascalCase('seed'), 'Seed')
})

test('scaffold uses the id as the manifest id and the folder contract', () => {
    const { index, componentName } = scaffoldFiles({ id: 'demo-plugin', name: 'Demo plugin' })
    assert.match(index, /id: 'demo-plugin'/)
    assert.match(index, /import DemoPluginTool from '\.\/DemoPluginTool\.vue'/)
    assert.equal(componentName, 'DemoPluginTool')
})

test('createLabPlugin writes an index and a component', async (t) => {
    const root = await temporaryRoot(t)
    const result = await createLabPlugin({ root, id: 'demo-plugin', name: 'Demo plugin' })

    const folder = path.join(root, 'apps/app-frontend/src/lab/plugins/packages/demo-plugin')
    assert.equal(result.folder, folder)
    const files = await fs.readdir(folder)
    assert.deepEqual(files.sort(), ['DemoPluginTool.vue', 'index.ts'])
})

test('createLabPlugin refuses a non kebab-case id', async (t) => {
    const root = await temporaryRoot(t)
    await assert.rejects(
        () => createLabPlugin({ root, id: 'DemoPlugin', name: 'Demo' }),
        /not a kebab-case id/,
    )
})

test('createLabPlugin refuses to overwrite an existing plugin', async (t) => {
    const root = await temporaryRoot(t)
    await createLabPlugin({ root, id: 'demo-plugin', name: 'Demo plugin' })
    await assert.rejects(
        () => createLabPlugin({ root, id: 'demo-plugin', name: 'Demo plugin' }),
        /already exists/,
    )
})
