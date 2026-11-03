import { expect, it } from 'vitest'

import type { InstanceSchematicFile } from './backend.ts'
import {
    buildInstanceSchematicRows,
    collectSchematicFolders,
    type InstanceSchematicFolderRow,
} from './instance-files.ts'

function schematicFile(relativePath: string): InstanceSchematicFile {
    const segments = relativePath.split(/[\\/]/)
    return {
        relativePath,
        fileName: segments[segments.length - 1] ?? relativePath,
        format: relativePath.toLocaleLowerCase().endsWith('.schem') ? 'schem' : 'litematic',
        size: 1,
    }
}

function fileRows(rows: ReturnType<typeof buildInstanceSchematicRows>) {
    return rows.filter((row) => row.kind === 'file')
}

function folderRows(rows: ReturnType<typeof buildInstanceSchematicRows>) {
    return rows.filter((row): row is InstanceSchematicFolderRow => row.kind === 'folder')
}

it('root files and nested folders produce collapsible folder rows', () => {
    const files = [
        schematicFile('house.litematic'),
        schematicFile('redstone/clock.litematic'),
        schematicFile('redstone/contraptions/gear.schem'),
        schematicFile('builds/castle.schem'),
    ]
    const rows = buildInstanceSchematicRows(files, new Set(collectSchematicFolders(files)), '')

    expect(
        rows.map((row) =>
            row.kind === 'folder'
                ? `folder:${row.name}(${row.fileCount})`
                : `file:${row.file.fileName}`,
        ),
    ).toEqual([
        'folder:builds(1)',
        'file:castle.schem',
        'file:house.litematic',
        'folder:redstone(2)',
        'file:clock.litematic',
        'folder:contraptions(1)',
        'file:gear.schem',
    ])
})

it('collapsed folders hide nested rows but keep their file count', () => {
    const files = [
        schematicFile('redstone/clock.litematic'),
        schematicFile('redstone/contraptions/gear.schem'),
    ]
    const rows = buildInstanceSchematicRows(files, new Set(), '')

    expect(
        rows.map((row) =>
            row.kind === 'folder'
                ? `folder:${row.name}(${row.fileCount})`
                : `file:${row.file.fileName}`,
        ),
    ).toEqual(['folder:redstone(2)'])
})

it('nested rows track their depth and parent folder', () => {
    const files = [schematicFile('a/b/c/tower.litematic')]
    const rows = buildInstanceSchematicRows(files, new Set(collectSchematicFolders(files)), '')

    const folders = folderRows(rows)
    expect(folders.map((row) => [row.path, row.depth])).toEqual([
        ['a', 0],
        ['a/b', 1],
        ['a/b/c', 2],
    ])
    const [file] = fileRows(rows)
    expect(file.file.fileName).toBe('tower.litematic')
    expect(file.depth).toBe(3)
    expect(file.parentPath).toBe('a/b/c')
})

it('search flattens matching files and filters by relative path', () => {
    const files = [
        schematicFile('house.litematic'),
        schematicFile('redstone/clock.litematic'),
        schematicFile('builds/house.schem'),
    ]
    const rows = buildInstanceSchematicRows(files, new Set(), 'house')

    expect(rows.map((row) => row.kind === 'file' && row.file.relativePath)).toEqual([
        'house.litematic',
        'builds/house.schem',
    ])
    expect(folderRows(rows).length).toBe(0)
})

it('windows-style backslash paths are normalized into folders', () => {
    const files = [schematicFile('redstone\\clock.litematic')]
    const folders = collectSchematicFolders(files)

    expect(folders).toEqual(['redstone'])
    const rows = buildInstanceSchematicRows(files, new Set(folders), '')
    expect(folderRows(rows).map((row) => [row.path, row.name, row.depth])).toEqual([
        ['redstone', 'redstone', 0],
    ])
    expect(fileRows(rows)[0].file.fileName).toBe('clock.litematic')
})

it('folder paths are sorted and deduplicated', () => {
    const files = [
        schematicFile('b/x.litematic'),
        schematicFile('a/y.schem'),
        schematicFile('a/b/z.litematic'),
        schematicFile('b/x.litematic'),
    ]

    expect(collectSchematicFolders(files)).toEqual(['a', 'a/b', 'b'])
})
