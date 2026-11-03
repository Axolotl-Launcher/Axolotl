import { expect, it } from 'vitest'

import {
    loadResourceSearchCategories,
    parseResourceSearchFilter,
} from './resource-search-filter.ts'

it('category failures do not prevent an independent source search', async () => {
    let calls = 0
    const load = async () => {
        calls += 1
        throw new Error('categories unavailable')
    }
    expect(await loadResourceSearchCategories('modrinth', load)).toBe(null)
    expect(calls).toBe(0)
    expect(await loadResourceSearchCategories('all', load)).toBe(
        'CurseForge: categories unavailable',
    )
    await expect(loadResourceSearchCategories('curseforge', load)).rejects.toThrow(
        /categories unavailable/,
    )
})

it('preserves OR environment groups, license, negative categories and advanced exclusions', () => {
    expect(
        parseResourceSearchFilter(
            '(environment = `server_only` OR environment = `client_or_server`) AND license = `MIT` AND categories NOT IN [`unofficial`, `lite`] AND all_project_types NOT IN [`modpack`]',
        ),
    ).toEqual({
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
    })
})

it('keeps complete category OR groups and provider category correspondence', () => {
    const result = parseResourceSearchFilter(
        'categories IN [`a`, `b`] AND categories = `fabric`',
        (value) => [`modrinth:${value}`, `curseforge:${value === 'a' ? 1 : 2}`],
        (value) => value === 'fabric',
    )
    expect(result).toEqual({
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
    expect(() =>
        parseResourceSearchFilter('environment = `client_only` unsupported syntax'),
    ).toThrow()
    expect(() => parseResourceSearchFilter('unknown = `value`')).toThrow()
})

it('preserves the shared open-source boolean filter without treating it as an identifier', () => {
    expect(parseResourceSearchFilter('open_source = true AND project_types = `mod`')).toEqual({
        kind: 'all',
        terms: [
            { kind: 'predicate', field: 'openSource', values: ['true'], excluded: false },
            { kind: 'predicate', field: 'type', values: ['mod'], excluded: false },
        ],
    })
})
