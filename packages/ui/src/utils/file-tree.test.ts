import assert from 'node:assert/strict'
import test from 'node:test'

import { buildFileTreeRows, createFileTreeRowBuilder, type FileTreeEntry } from './file-tree.ts'

test('folder expansion reuses paths and ordering while retaining all descendant identities', () => {
    let pathReads = 0
    let nameReads = 0
    const files: FileTreeEntry[] = Array.from({ length: 5000 }, (_, index) => ({
        id: `entry-${index}`,
        get relativePath() {
            pathReads += 1
            return `建筑/${(index * 7919) % 5000}.litematic`
        },
        get fileName() {
            nameReads += 1
            return `${(index * 7919) % 5000}.litematic`
        },
    }))
    const rowsFor = createFileTreeRowBuilder(files, 'zh-CN')
    const collapsed = rowsFor(new Set())
    assert.equal(collapsed.length, 1)
    assert.equal(collapsed[0].kind, 'folder')
    if (collapsed[0].kind !== 'folder') assert.fail('Expected a folder')
    assert.equal(collapsed[0].fileCount, 5000)
    assert.deepEqual(
        collapsed[0].childIds,
        files.map((file) => file.id),
    )

    const expanded = rowsFor(new Set(['建筑']))
    assert.equal(expanded.length, 5001)
    const reads = { paths: pathReads, names: nameReads }
    assert.deepEqual(rowsFor(new Set()), collapsed)
    assert.deepEqual(rowsFor(new Set(['建筑'])), expanded)
    assert.equal(pathReads, reads.paths)
    assert.equal(nameReads, reads.names)
    assert.equal(
        new Set(expanded.filter((row) => row.kind === 'file').map((row) => row.file.id)).size,
        5000,
    )
})

test('folder and file rows retain locale ordering and flat path search', () => {
    const files = [
        { id: 'a', relativePath: 'ä/one.litematic', fileName: 'one.litematic' },
        { id: 'z', relativePath: 'z.litematic', fileName: 'z.litematic' },
    ]
    assert.deepEqual(
        buildFileTreeRows(files, new Set(), '', 'de').map((row) => row.kind),
        ['folder', 'file'],
    )
    assert.deepEqual(
        buildFileTreeRows(files, new Set(), '', 'sv').map((row) => row.kind),
        ['file', 'folder'],
    )
    assert.deepEqual(buildFileTreeRows(files, new Set(), 'Ä/', 'de'), [
        { kind: 'file', file: files[0], depth: 0, parentPath: 'ä' },
    ])
})
