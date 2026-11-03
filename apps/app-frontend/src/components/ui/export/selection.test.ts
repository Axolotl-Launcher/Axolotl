import { expect, it } from 'vitest'

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

it('defaults contain all nested files and omit runtime, old and unselected world files', () => {
    expect(selectedFiles(files, initial())).toEqual([
        'mods/a.jar',
        'mods/nested/b.jar',
        'shaderpacks/a.zip',
        'shaderpacks/a.zip.txt',
    ])
})
it('manual exclusions and inclusions survive changing categories and returning', () => {
    let draft = reviseFileSelection(files, initial(), [
        'mods/a.jar',
        'mods/runtime/cache',
        'shaderpacks/a.zip',
        'shaderpacks/a.zip.txt',
    ])
    draft = reviseCategorySelection(draft, category('mods'), false)
    expect(selectedFiles(files, draft)).toEqual([
        'mods/runtime/cache',
        'shaderpacks/a.zip',
        'shaderpacks/a.zip.txt',
    ])
    draft = reviseCategorySelection(draft, category('mods'), true)
    expect(selectedFiles(files, draft)).toEqual([
        'mods/a.jar',
        'mods/runtime/cache',
        'shaderpacks/a.zip',
        'shaderpacks/a.zip.txt',
    ])
    expect(draft.excluded.includes('mods/nested/b.jar')).toBeTruthy()
})
it('shader settings and packs are separate leaves with independent selection', () => {
    const shaders = category('shaders', true, [category('pack'), category('settings')])
    expect(defaultCategories([shaders])).toEqual(['pack', 'settings'])
    const draft = reviseCategorySelection(initial(), shaders.children[1], false)
    expect(selectedFiles(files, draft).includes('shaderpacks/a.zip')).toBeTruthy()
    expect(!selectedFiles(files, draft).includes('shaderpacks/a.zip.txt')).toBeTruthy()
})
it('snapshot selection never accepts a new or forbidden path from a caller', () => {
    const draft = reviseFileSelection(files, initial(), [
        'mods/a.jar',
        'assets/secret',
        'mods/new.jar',
    ])
    expect(selectedFiles(files, draft)).toEqual(['mods/a.jar'])
})
it('restore removes only manual changes and keeps the current category choices', () => {
    const draft = reviseCategorySelection(initial(), category('pack'), false)
    const changed = reviseFileSelection(files, draft, ['mods/a.jar'])
    expect(selectedFiles(files, { ...changed, included: [], excluded: [] })).toEqual(
        selectedFiles(files, draft),
    )
})
