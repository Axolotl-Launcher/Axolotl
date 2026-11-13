import assert from 'node:assert/strict'
import test from 'node:test'

import type { ContentItem } from '../types.ts'
import { sortContentItems } from './content-sorting.ts'

function item(id: string, fileName: string, projectName?: string, dateAdded?: string): ContentItem {
    return {
        id,
        file_name: fileName,
        project_type: 'mod',
        update: null,
        origin_provider: null,
        date_added: dateAdded,
        project: projectName
            ? {
                  id,
                  slug: id,
                  title: projectName,
                  icon_url: null,
              }
            : undefined,
    }
}

test('sorts file names naturally and treats disabled suffix as state', () => {
    const sorted = sortContentItems(
        [
            item('ten', '中文模组-10.jar'),
            item('disabled', '中文模组-2.jar.disabled'),
            item('two', '中文模组-2.jar'),
        ],
        'file-name-asc',
        'zh-CN',
    )

    assert.deepEqual(
        sorted.map((entry) => entry.id),
        ['two', 'disabled', 'ten'],
    )
})

test('uses file names and stable ids to break project-name ties', () => {
    const sorted = sortContentItems(
        [
            item('shared-hash', 'zeta.jar', 'Same project'),
            item('shared-hash', 'alpha.jar', 'Same project', 'entry-b'),
            item('shared-hash', 'alpha.jar', 'Same project', 'entry-a'),
        ],
        'project-name-asc',
        'en-US',
        (entry) => entry.date_added ?? 'entry-c',
    )

    assert.deepEqual(
        sorted.map((entry) => entry.date_added),
        ['entry-a', 'entry-b', undefined],
    )
})

test('sorts added dates in both directions with deterministic missing-date fallback', () => {
    const items = [
        item('missing', 'missing.jar', undefined),
        item('older', 'older.jar', undefined, '2026-01-01T00:00:00Z'),
        item('newer', 'newer.jar', undefined, '2026-02-01T00:00:00Z'),
    ]

    assert.deepEqual(
        sortContentItems(items, 'date-added-newest').map((entry) => entry.id),
        ['newer', 'older', 'missing'],
    )
    assert.deepEqual(
        sortContentItems(items, 'date-added-oldest').map((entry) => entry.id),
        ['missing', 'older', 'newer'],
    )
})

test('keeps locale-specific ordering and the input collection intact for large schematic lists', () => {
    const items = Array.from({ length: 5000 }, (_, index) =>
        item(String(index), `建筑-${(index * 7919) % 5000}.litematic`),
    )
    const original = [...items]
    assert.deepEqual(
        sortContentItems(items, 'file-name-asc', 'zh-CN').map((entry) => entry.file_name),
        Array.from({ length: 5000 }, (_, index) => `建筑-${index}.litematic`),
    )
    assert.deepEqual(items, original)

    const localized = [item('a', 'ä.litematic'), item('z', 'z.litematic')]
    assert.deepEqual(
        sortContentItems(localized, 'file-name-asc', 'de').map((entry) => entry.id),
        ['a', 'z'],
    )
    assert.deepEqual(
        sortContentItems(localized, 'file-name-asc', 'sv').map((entry) => entry.id),
        ['z', 'a'],
    )
})
