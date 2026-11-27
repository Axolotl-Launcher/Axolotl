import { expect, it, vi } from 'vitest'

import { invoke } from '@/helpers/invoke'

import { classifyDroppedItem, classifyDroppedItemWithExtraction } from './drop'

vi.mock('@/helpers/invoke', () => ({ invoke: vi.fn() }))

it('keeps candidate file paths when normalizing a dropped directory', async () => {
    const folder = 'D:\\Downloads\\Pack collection'
    const archive = `${folder}\\nested\\example.zip`
    vi.mocked(invoke).mockResolvedValueOnce({
        item_type: 'multiple',
        file_path: folder,
        candidates: [
            { itemType: 'modpack', filePath: archive, innerBase: 'nested/example.zip' },
            {
                itemType: 'world_save',
                filePath: archive,
                innerBase: 'nested/example.zip/saves/World',
            },
        ],
    })

    await expect(classifyDroppedItem(folder)).resolves.toEqual({
        item_type: 'multiple',
        file_path: folder,
        choices: [
            { itemType: 'modpack', filePath: archive },
            { itemType: 'world_save', filePath: archive, innerBase: 'saves/World' },
        ],
    })
})

it('normalizes choices in a shortcut without changing archive-relative paths', async () => {
    const archive = 'D:/Downloads/example.zip'
    const world = { itemType: 'world_save', filePath: archive, innerBase: 'saves/World' }
    vi.mocked(invoke).mockResolvedValueOnce({
        item_type: 'shortcut_resolved',
        original: 'D:/Downloads/example.lnk',
        resolved_to: {
            item_type: 'multiple',
            file_path: archive,
            candidates: [world],
        },
    })

    await expect(classifyDroppedItemWithExtraction(archive)).resolves.toEqual({
        item_type: 'shortcut_resolved',
        original: 'D:/Downloads/example.lnk',
        resolved_to: {
            item_type: 'multiple',
            file_path: archive,
            choices: [world],
        },
    })
})
