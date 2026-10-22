import assert from 'node:assert/strict'
import test from 'node:test'

import { contentOwnership, installedSource } from './content-provenance.ts'

test('installation source is independent of ownership and recognized counterparts', () => {
    assert.equal(
        installedSource({ installedSource: 'modrinth', origin_provider: 'modrinth' }),
        'modrinth',
    )
    assert.equal(installedSource({ origin_provider: 'curseforge' }), 'curseforge')
    assert.equal(
        installedSource({ installedSource: 'mcarchive', origin_provider: null }),
        'mcarchive',
    )
    assert.equal(installedSource({ origin_provider: null }), 'unknown')
    assert.equal(installedSource({ installedSource: null, origin_provider: 'modrinth' }), 'unknown')
    assert.equal(
        contentOwnership({ instanceOwnershipKind: 'pack_managed', source_kind: 'curseforge' }),
        'pack_managed',
    )
    assert.equal(
        contentOwnership({ instanceOwnershipKind: 'user_added', source_kind: 'local' }),
        'user_added',
    )
    assert.equal(
        contentOwnership({ instanceOwnershipKind: 'local_discovered' }),
        'local_discovered',
    )
    assert.equal(contentOwnership({ source_kind: 'imported_modpack' }), 'pack_managed')
    assert.equal(contentOwnership({}), 'unknown')
})

test('a cross-source update changes installation source without changing pack ownership', () => {
    const item = {
        origin_provider: 'curseforge' as const,
        instanceOwnershipKind: 'pack_managed' as const,
    }
    assert.equal(installedSource(item), 'curseforge')
    const updated = { ...item, origin_provider: 'modrinth' as const }
    assert.equal(installedSource(updated), 'modrinth')
    assert.equal(contentOwnership(updated), 'pack_managed')
})
