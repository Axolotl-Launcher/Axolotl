import { readFileSync } from 'node:fs'

import { expect, it } from 'vitest'

it('every resource IPC handler has a matching desktop permission declaration', () => {
    const bridge = readFileSync(
        new URL('../../../app/src/api/resources.rs', import.meta.url),
        'utf8',
    )
    const build = readFileSync(new URL('../../../app/build.rs', import.meta.url), 'utf8')
    const handlers = bridge.match(/generate_handler!\[([\s\S]*?)\]/)?.[1]
    const permissions = build.match(/"axolotl-resources",[\s\S]*?\.commands\(&\[([\s\S]*?)\]/)?.[1]
    expect(handlers).toBeTruthy()
    expect(permissions).toBeTruthy()
    const names = (source: string) => [...new Set(source.match(/\bresource_[a-z_]+\b/g))].sort()
    expect(names(handlers)).toEqual(names(permissions))
})
