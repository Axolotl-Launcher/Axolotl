import assert from 'node:assert/strict'
import test from 'node:test'

import type { ContentItem } from '@modrinth/ui'

import { mergeContentItemMetadata } from './content-item-metadata.ts'

test('lazy display metadata retains the resource update and history of the stable entry', () => {
    const item = {
        instanceEntryId: 'entry-a',
        instanceMaterializationState: 'present',
        instanceCapabilities: { canUpdate: false, canChangeVersion: true },
        update: null,
        resourceUpdate: { project: { source: 'curseforge', projectId: '1' }, releaseId: '42' },
        rollback: { file_name: 'old.jar' },
    } as ContentItem
    for (const update of [
        { provider: 'modrinth', target_version_id: 'new-version' },
        { provider: 'curseforge', target_file_id: 42 },
    ]) {
        const refreshed = mergeContentItemMetadata(item, { update } as ContentItem)
        assert.equal(refreshed.instanceEntryId, 'entry-a')
        assert.equal(refreshed.instanceCapabilities?.canUpdate, true)
        assert.equal(refreshed.instanceCapabilities?.canChangeVersion, true)
        assert.equal(refreshed.update, null)
        assert.deepEqual(refreshed.resourceUpdate, item.resourceUpdate)
        assert.deepEqual(refreshed.rollback, item.rollback)
        assert.equal(
            mergeContentItemMetadata(refreshed, { update: null } as ContentItem)
                .instanceCapabilities?.canUpdate,
            true,
        )
        assert.equal(item.instanceCapabilities?.canUpdate, false)
    }
})

test('lazy metadata never replaces actual path, entry identity or connector origin with stale presentation', () => {
    const item = {
        instanceEntryId: 'entry',
        instanceFileId: 'file',
        installedSource: 'planet_minecraft',
        origin_provider: 'planet_minecraft',
        file_path: 'mods/custom.jar.disabled',
        file_name: 'custom.jar.disabled',
        enabled: false,
        instanceMaterializationState: 'present',
    } as ContentItem
    const merged = mergeContentItemMetadata(item, {
        instanceEntryId: 'other',
        origin_provider: null,
        file_path: 'mods/wrong.jar',
        file_name: 'wrong.jar',
        enabled: true,
    } as ContentItem)
    for (const key of [
        'instanceEntryId',
        'instanceFileId',
        'origin_provider',
        'installedSource',
        'file_path',
        'file_name',
        'enabled',
        'instanceMaterializationState',
    ] as const)
        assert.equal(merged[key], item[key])
})

test('update metadata does not enable mutation of a missing or removed file', () => {
    for (const instanceMaterializationState of ['missing', 'removed', 'pending_manual']) {
        const item = {
            instanceMaterializationState,
            instanceCapabilities: { canUpdate: false },
        } as ContentItem
        const metadata = {
            update: { provider: 'modrinth', target_version_id: 'new' },
        } as ContentItem
        assert.equal(
            mergeContentItemMetadata(item, metadata).instanceCapabilities?.canUpdate,
            false,
        )
    }
})

test('lazy recognition preserves actual provenance, ownership and compatibility', () => {
    for (const installedSource of [null, 'curseforge']) {
        const item = {
            instanceEntryId: 'entry-a',
            installedSource,
            instanceOwnershipKind: 'pack_managed',
            compatibility: {
                gameVersion: 'incompatible',
                loader: 'unknown',
                reasons: ['gameVersion', 'loader'],
            },
        } as ContentItem
        const merged = mergeContentItemMetadata(item, {
            origin_provider: 'modrinth',
            installedSource: 'modrinth',
            instanceOwnershipKind: 'user_added',
            compatibility: null,
        } as ContentItem)
        assert.equal(merged.installedSource, installedSource)
        assert.equal(merged.instanceOwnershipKind, 'pack_managed')
        assert.deepEqual(merged.compatibility, item.compatibility)
    }
})
