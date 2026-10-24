import assert from 'node:assert/strict'
import test from 'node:test'

import type { ContentItem } from '../../../lib/state/downloads'
import type { FsDir } from '../files/instance-files'
import { reconcileContentFiles } from './content-files.ts'

test('upgrade reconciles managed files while preserving visitor edits and disabled state', () => {
    const custom = {
        kind: 'file' as const,
        name: 'notes.txt',
        size: 4,
        text: 'keep',
        created: 1,
        modified: 1,
    }
    const config = {
        kind: 'file' as const,
        name: 'options.txt',
        size: 3,
        text: 'edited',
        created: 1,
        modified: 2,
    }
    const tree: FsDir = {
        kind: 'dir',
        name: 'Test',
        created: 1,
        modified: 1,
        children: [
            config,
            {
                kind: 'dir',
                name: 'mods',
                created: 1,
                modified: 1,
                children: [
                    custom,
                    { kind: 'file', name: 'old.jar', size: 10, created: 1, modified: 1 },
                ],
            },
        ],
    }
    const before = [
        { file_name: 'old.jar', enabled: true, project_type: 'mod', size: 10 },
    ] as ContentItem[]
    const after = [{ ...before[0], file_name: 'new.jar', enabled: false, size: 20 }]
    reconcileContentFiles(tree, before, after, 3)
    assert.equal(tree.children![0], config)
    const mods = tree.children![1] as FsDir
    assert.equal(mods.children![0], custom)
    assert.deepEqual(
        mods.children!.map((file) => file.name),
        ['notes.txt', 'new.jar.disabled'],
    )
})
