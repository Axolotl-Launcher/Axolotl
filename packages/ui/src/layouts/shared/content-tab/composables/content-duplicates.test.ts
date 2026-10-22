import assert from 'node:assert/strict'
import test from 'node:test'

import type { ContentItem } from '../types.ts'
import {
    confirmedContentGroupKey,
    duplicateContentGroups,
    duplicateContentRows,
    duplicateContentStatus,
    duplicateUpdateAnchor,
    visibleDuplicateGroups,
} from './content-duplicates.ts'

const item = (id: string, overrides: Partial<ContentItem> = {}) =>
    ({
        id,
        instanceResourceId: 'verified',
        instanceEntryId: id,
        project_type: 'mod',
        file_path: `mods/${id}.jar`,
        instanceMaterializationState: 'present',
        enabled: true,
        ...overrides,
    }) as ContentItem

test('duplicate groups use confirmed identity and scope, retaining actual entries across sources and versions', () => {
    const entries = [
        item('a'),
        item('b', { origin_provider: 'curseforge' }),
        item('b'),
        item('unconfirmed', { instanceResourceId: undefined }),
        item('different', { instanceResourceId: 'another' }),
    ]
    const groups = duplicateContentGroups(entries, (item) => item.instanceEntryId!)
    assert.equal(groups.size, 1)
    assert.deepEqual(
        [...groups.values()][0].map((item) => item.id),
        ['a', 'b'],
    )
    assert.equal(
        confirmedContentGroupKey(item('unconfirmed', { instanceResourceId: undefined })),
        null,
    )
    const a = item('a', { project_type: 'datapack', file_path: 'saves/one/datapacks/a.zip' })
    const b = item('b', { project_type: 'datapack', file_path: 'saves/two/datapacks/a.zip' })
    assert.notEqual(confirmedContentGroupKey(a), confirmedContentGroupKey(b))
    assert.equal(duplicateContentGroups([a, b], (item) => item.id).size, 0)
})

test('group summary retains missing, disabled, processing and update counts separately', () => {
    const update = { project: { source: 'modrinth', projectId: 'project' }, releaseId: 'next' }
    assert.deepEqual(
        duplicateContentStatus([
            item('a', { resourceUpdate: update }),
            item('b', { resourceUpdate: update, enabled: false }),
            item('c', { instanceMaterializationState: 'missing', resourceUpdate: update }),
            item('d', { installing: true }),
            item('locked', {
                resourceUpdate: update,
                instanceCapabilities: { canUpdate: false } as ContentItem['instanceCapabilities'],
            }),
        ]),
        { updates: 2, disabled: 1, missing: 1, installing: 1 },
    )
})

test('filtering keeps full duplicate identity and totals while group selection only addresses matching files', () => {
    const a = item('a'),
        b = item('b'),
        hidden = item('hidden', { enabled: false })
    const all = duplicateContentGroups([a, b, hidden], (item) => item.id)
    const filtered = visibleDuplicateGroups(all, [b], (item) => item.id)
    assert.equal(filtered.size, 1)
    assert.equal([...all.values()][0].length, 3)
    assert.deepEqual(
        [...filtered.values()][0].map((item) => item.id),
        ['b'],
    )
    assert.equal([...filtered.keys()][0], [...all.keys()][0])
    assert.equal(visibleDuplicateGroups(all, [], (item) => item.id).size, 0)
    assert.deepEqual(
        [...visibleDuplicateGroups(all, [b, a], (item) => item.id).values()][0].map(
            (item) => item.id,
        ),
        ['b', 'a'],
    )
})

test('duplicates start folded, expose every actual child when expanded, and keep expansion across refresh and filters', () => {
    const a = item('a'),
        b = item('b', { origin_provider: 'curseforge' })
    const options = {
        groups: duplicateContentGroups([a, b], (item) => item.id),
        visibleGroups: duplicateContentGroups([a, b], (item) => item.id),
        expanded: new Set<string>(),
        getId: (item: ContentItem) => item.id,
        mapItem: (item: ContentItem) => ({
            id: item.id,
            project: { id: 'project', slug: null, title: 'Project', icon_url: null },
            localPath: item.file_path,
            installationSource: item.origin_provider ?? undefined,
            version: { id: item.id, version_number: item.id, file_name: item.file_path! },
        }),
        summary: (_status: unknown, visible: number) => `${visible} visible`,
        updateLabel: (count: number) => `Review all ${count} copies`,
        canUpdate: true,
        isBusy: () => false,
    }
    const closed = duplicateContentRows(options)
    assert.equal(closed.length, 1)
    assert.equal(closed[0].groupItemCount, 2)
    assert.deepEqual(closed[0].groupChildIds, ['a', 'b'])
    options.expanded.add(closed[0].group!)
    const open = duplicateContentRows(options)
    assert.equal(duplicateUpdateAnchor(open, open[0].id), 'a')
    assert.equal(duplicateUpdateAnchor(open, 'a'), undefined)
    assert.deepEqual(
        open.map((row) => row.id),
        [closed[0].id, 'a', 'b'],
    )
    assert.equal(open[2].installationSource, 'curseforge')
    assert.equal(open[2].localPath, 'mods/b.jar')
    options.visibleGroups = visibleDuplicateGroups(options.groups, [b], options.getId)
    const filtered = duplicateContentRows(options)
    assert.equal(filtered[0].groupItemCount, 2)
    assert.deepEqual(filtered[0].groupChildIds, ['b'])
    assert.equal(filtered[0].updateLabel, 'Review all 2 copies')
    assert.equal(filtered[1].id, 'b')
    options.groups = duplicateContentGroups([{ ...a }, { ...b }], options.getId)
    assert.deepEqual(
        duplicateContentRows(options).map((row) => row.id),
        [closed[0].id, 'b'],
    )
})
