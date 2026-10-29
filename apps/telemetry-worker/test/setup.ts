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

/**
 * Data tables to clear between tests. `substr` is used instead of `NOT LIKE`
 * because `_` is a LIKE wildcard, so `'_cf_%'` would also match e.g. `XcfY`.
 * The migration ledger is preserved so migrations are applied exactly once.
 */
async function dataTables(): Promise<string[]> {
    const { results } = await env.DB.prepare(
        'SELECT name FROM sqlite_master' +
            " WHERE type = 'table'" +
            " AND substr(name, 1, 7) <> 'sqlite_'" +
            " AND substr(name, 1, 4) <> '_cf_'" +
            " AND name <> 'd1_migrations'" +
            ' ORDER BY name',
    ).all<{ name: string }>()
    return results.map((row) => row.name)
}

/** Parent table -> child tables that reference it. */
async function foreignKeys(tables: string[]): Promise<Map<string, string[]>> {
    const known = new Set(tables)
    const parents = new Map<string, string[]>()
    for (const table of tables) {
        const { results } = await env.DB.prepare(`PRAGMA foreign_key_list("${table}")`).all<{
            table: string
        }>()
        for (const { table: parent } of results) {
            if (parent !== table && known.has(parent)) {
                parents.set(parent, [...(parents.get(parent) ?? []), table])
            }
        }
    }
    return parents
}

/**
 * Children-first deletion order, derived from the foreign keys themselves, so
 * correctness does not depend on `PRAGMA defer_foreign_keys`.  This schema is
 * acyclic; if a cycle is ever introduced the remainder is appended and deleted
 * last, which surfaces the problem as a test failure rather than hiding it.
 */
function childrenFirst(tables: string[], parents: Map<string, string[]>): string[] {
    const remaining = new Set(tables)
    const order: string[] = []
    while (remaining.size > 0) {
        const ready = [...remaining].filter(
            (table) =>
                ![...remaining].some(
                    (child) => child !== table && (parents.get(table) ?? []).includes(child),
                ),
        )
        if (ready.length === 0) {
            order.push(...remaining)
            break
        }
        for (const table of ready) {
            order.push(table)
            remaining.delete(table)
        }
    }
    return order
}

// vitest 4's Workers pool isolates storage per test file, not per test. Clear the
// data tables before each test so tests stay independent of each other's writes.
beforeEach(async () => {
    const tables = await dataTables()
    if (tables.length === 0) return
    const parents = await foreignKeys(tables)
    for (const table of childrenFirst(tables, parents)) {
        await env.DB.prepare(`DELETE FROM "${table}"`).run()
    }
})
