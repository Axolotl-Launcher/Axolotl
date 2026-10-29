import { expect, it } from 'vitest'

import { getActiveDependencyConflictIdentities } from './content-selection-logic.ts'

function dependency(id: string, requiredBy: string, ownerKey: string) {
    return { id, title: id, requiredBy: [requiredBy], requiredByKeys: [ownerKey] }
}

it('removing one conflicting owner clears the active conflict', () => {
    const dependencies = [
        dependency('modrinth:shared:v1', 'First', 'modrinth:first'),
        dependency('modrinth:shared:v2', 'Second', 'modrinth:second'),
    ]

    expect([
        ...getActiveDependencyConflictIdentities(
            dependencies,
            new Set(['modrinth:first', 'modrinth:second']),
        ),
    ]).toEqual(['modrinth:shared'])
    expect(
        getActiveDependencyConflictIdentities(dependencies, new Set(['modrinth:second'])).size,
    ).toBe(0)
})
