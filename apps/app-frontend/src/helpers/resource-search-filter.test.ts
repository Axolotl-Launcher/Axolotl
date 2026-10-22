import assert from 'node:assert/strict'
import test from 'node:test'

import {
    loadResourceSearchCategories,
    parseResourceSearchFilter,
} from './resource-search-filter.ts'

test('category failures do not prevent an independent source search', async () => {
    let calls = 0
    const load = async () => {
        calls += 1
        throw new Error('categories unavailable')
    }
    assert.equal(await loadResourceSearchCategories('modrinth', load), null)
    assert.equal(calls, 0)
    assert.equal(
        await loadResourceSearchCategories('all', load),
        'CurseForge: categories unavailable',
    )
    await assert.rejects(loadResourceSearchCategories('curseforge', load), /categories unavailable/)
})

test('preserves OR environment groups, license, negative categories and advanced exclusions', () => {
    assert.deepEqual(
        parseResourceSearchFilter(
            '(environment = `server_only` OR environment = `client_or_server`) AND license = `MIT` AND categories NOT IN [`unofficial`, `lite`] AND all_project_types NOT IN [`modpack`]',
        ),
        {
            kind: 'all',
            terms: [
                {
                    kind: 'any',
                    terms: [
                        {
                            kind: 'predicate',
                            field: 'environment',
                            values: ['server_only'],
                            excluded: false,
                        },
                        {
                            kind: 'predicate',
                            field: 'environment',
                            values: ['client_or_server'],
                            excluded: false,
                        },
                    ],
                },
                { kind: 'predicate', field: 'license', values: ['MIT'], excluded: false },
                {
                    kind: 'predicate',
                    field: 'category',
                    values: ['unofficial', 'lite'],
                    excluded: true,
                },
                { kind: 'predicate', field: 'allTypes', values: ['modpack'], excluded: true },
            ],
        },
    )
})

test('keeps complete category OR groups and provider category correspondence', () => {
    const result = parseResourceSearchFilter(
        'categories IN [`a`, `b`] AND categories = `fabric`',
        (value) => [`modrinth:${value}`, `curseforge:${value === 'a' ? 1 : 2}`],
        (value) => value === 'fabric',
    )
    assert.deepEqual(result, {
        kind: 'all',
        terms: [
            {
                kind: 'predicate',
                field: 'category',
                values: ['modrinth:a', 'curseforge:1', 'modrinth:b', 'curseforge:2'],
                excluded: false,
            },
            { kind: 'predicate', field: 'loader', values: ['fabric'], excluded: false },
        ],
    })
    assert.throws(() => parseResourceSearchFilter('environment = `client_only` unsupported syntax'))
    assert.throws(() => parseResourceSearchFilter('unknown = `value`'))
})

test('preserves the shared open-source boolean filter without treating it as an identifier', () => {
    assert.deepEqual(parseResourceSearchFilter('open_source = true AND project_types = `mod`'), {
        kind: 'all',
        terms: [
            { kind: 'predicate', field: 'openSource', values: ['true'], excluded: false },
            { kind: 'predicate', field: 'type', values: ['mod'], excluded: false },
        ],
    })
})
