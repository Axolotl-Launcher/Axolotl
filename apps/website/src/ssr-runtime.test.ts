import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const websiteRequire = createRequire(new URL('../package.json', import.meta.url))
const vueEntry = websiteRequire.resolve('vue')

for (const [name, entry] of [
    ['Nuxt', 'nuxt/package.json'],
    ['shared UI', '@modrinth/ui'],
    ['server renderer', 'vue/server-renderer'],
]) {
    test(`${name} resolves the website's Vue runtime`, () => {
        const dependencyRequire = createRequire(websiteRequire.resolve(entry))
        assert.equal(
            dependencyRequire.resolve('vue'),
            vueEntry,
            `${name} must share the website's Vue runtime to preserve the SSR context`,
        )
    })
}
