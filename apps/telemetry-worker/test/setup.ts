import { applyD1Migrations, env, type D1Migration } from 'cloudflare:test'
import { beforeAll, beforeEach } from 'vitest'

declare global {
    namespace Cloudflare {
        interface Env {
            DB: D1Database
            TEST_MIGRATIONS: D1Migration[]
        }
    }
}

beforeAll(async () => {
    await applyD1Migrations(env.DB, env.TEST_MIGRATIONS)
})

// vitest 4's Workers pool isolates storage per test file, not per test. Clear the
// data tables before each test so tests stay independent of each other's writes.
beforeEach(async () => {
    const tables = await env.DB.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_cf_%' AND name != 'd1_migrations'",
    ).all<{ name: string }>()
    if (tables.results.length === 0) return
    await env.DB.batch([
        env.DB.prepare('PRAGMA defer_foreign_keys = ON'),
        ...tables.results.map(({ name }) => env.DB.prepare(`DELETE FROM "${name}"`)),
    ])
})
