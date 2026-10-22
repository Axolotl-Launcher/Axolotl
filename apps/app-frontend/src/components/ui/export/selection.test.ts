import assert from 'node:assert/strict'
import test from 'node:test'

import type { ExportCategory, ExportFileSnapshot } from '@/helpers/resources'

import {
    defaultCategories,
    type ExportSelectionDraft,
    reviseCategorySelection,
    reviseFileSelection,
    selectedFiles,
} from './selection.ts'

const category = (
    id: string,
    selected = true,
    children: ExportCategory[] = [],
): ExportCategory => ({
    id,
    defaultSelected: selected,
    paths: [],
    children,
    title: null,
    modifiedAt: null,
})
const file = (path: string, categoryId: string, eligible = true): ExportFileSnapshot => ({
    relativePath: path,
    categoryId,
    categorySelected: eligible,
    integrity: { size: 5, hashes: [] },
    revision: 1,
    identifiable: false,
    modifiedAt: null,
})
const files = [
    file('mods/a.jar', 'mods'),
    file('mods/nested/b.jar', 'mods'),
    file('mods/runtime/cache', 'mods', false),
    file('mods/old.jar.old', 'old'),
    file('saves/world/datapacks/a.zip', 'world'),
    file('shaderpacks/a.zip', 'pack'),
    file('shaderpacks/a.zip.txt', 'settings'),
]
const initial = (): ExportSelectionDraft => ({
    categories: ['mods', 'pack', 'settings'],
    included: [],
    excluded: [],
})

test('defaults contain all nested files and omit runtime, old and unselected world files', () => {
    assert.deepEqual(selectedFiles(files, initial()), [
        'mods/a.jar',
        'mods/nested/b.jar',
        'shaderpacks/a.zip',
        'shaderpacks/a.zip.txt',
    ])
})
test('manual exclusions and inclusions survive changing categories and returning', () => {
    let draft = reviseFileSelection(files, initial(), [
        'mods/a.jar',
        'mods/runtime/cache',
        'shaderpacks/a.zip',
        'shaderpacks/a.zip.txt',
    ])
    draft = reviseCategorySelection(draft, category('mods'), false)
    assert.deepEqual(selectedFiles(files, draft), [
        'mods/runtime/cache',
        'shaderpacks/a.zip',
        'shaderpacks/a.zip.txt',
    ])
    draft = reviseCategorySelection(draft, category('mods'), true)
    assert.deepEqual(selectedFiles(files, draft), [
        'mods/a.jar',
        'mods/runtime/cache',
        'shaderpacks/a.zip',
        'shaderpacks/a.zip.txt',
    ])
    assert.ok(draft.excluded.includes('mods/nested/b.jar'))
})
test('shader settings and packs are separate leaves with independent selection', () => {
    const shaders = category('shaders', true, [category('pack'), category('settings')])
    assert.deepEqual(defaultCategories([shaders]), ['pack', 'settings'])
    const draft = reviseCategorySelection(initial(), shaders.children[1], false)
    assert.ok(selectedFiles(files, draft).includes('shaderpacks/a.zip'))
    assert.ok(!selectedFiles(files, draft).includes('shaderpacks/a.zip.txt'))
})
test('snapshot selection never accepts a new or forbidden path from a caller', () => {
    const draft = reviseFileSelection(files, initial(), [
        'mods/a.jar',
        'assets/secret',
        'mods/new.jar',
    ])
    assert.deepEqual(selectedFiles(files, draft), ['mods/a.jar'])
})
test('restore removes only manual changes and keeps the current category choices', () => {
    const draft = reviseCategorySelection(initial(), category('pack'), false)
    const changed = reviseFileSelection(files, draft, ['mods/a.jar'])
    assert.deepEqual(
        selectedFiles(files, { ...changed, included: [], excluded: [] }),
        selectedFiles(files, draft),
    )
})
