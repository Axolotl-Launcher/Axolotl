import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

test('every resource IPC handler has a matching desktop permission declaration', () => {
    const bridge = readFileSync(
        new URL('../../../app/src/api/resources.rs', import.meta.url),
        'utf8',
    )
    const build = readFileSync(new URL('../../../app/build.rs', import.meta.url), 'utf8')
    const handlers = bridge.match(/generate_handler!\[([\s\S]*?)\]/)?.[1]
    const permissions = build.match(/"axolotl-resources",[\s\S]*?\.commands\(&\[([\s\S]*?)\]/)?.[1]
    assert.ok(handlers)
    assert.ok(permissions)
    const names = (source: string) => [...new Set(source.match(/\bresource_[a-z_]+\b/g))].sort()
    assert.deepEqual(names(handlers), names(permissions))
})
