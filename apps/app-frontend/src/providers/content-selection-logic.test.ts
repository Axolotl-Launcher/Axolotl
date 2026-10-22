import assert from 'node:assert/strict'
import test from 'node:test'

import { getActiveDependencyConflictIdentities } from './content-selection-logic.ts'

function dependency(id: string, requiredBy: string, ownerKey: string) {
    return { id, title: id, requiredBy: [requiredBy], requiredByKeys: [ownerKey] }
}

test('removing one conflicting owner clears the active conflict', () => {
    const dependencies = [
        dependency('modrinth:shared:v1', 'First', 'modrinth:first'),
        dependency('modrinth:shared:v2', 'Second', 'modrinth:second'),
    ]

    assert.deepEqual(
        [
            ...getActiveDependencyConflictIdentities(
                dependencies,
                new Set(['modrinth:first', 'modrinth:second']),
            ),
        ],
        ['modrinth:shared'],
    )
    assert.equal(
        getActiveDependencyConflictIdentities(dependencies, new Set(['modrinth:second'])).size,
        0,
    )
})
