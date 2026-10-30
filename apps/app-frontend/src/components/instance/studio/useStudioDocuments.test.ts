import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { expect, it } from 'vitest'

import { type StudioDocument, useStudioDocuments } from './useStudioDocuments.ts'

function document(path: string, kind: StudioDocument['kind'] = 'text'): StudioDocument {
    return { kind, path, name: path, content: 'saved', savedContent: 'saved', saving: false }
}

function deferred() {
    let resolve!: () => void
    const promise = new Promise<void>((done) => {
        resolve = done
    })
    return { promise, resolve }
}

for (const kind of ['text', 'nbt'] as const) {
    it(`deleting a dirty ${kind} document cannot recreate its file`, async () => {
        const directory = await mkdtemp(join(tmpdir(), 'axolotl-studio-'))
        const path = join(directory, 'document')
        try {
            await writeFile(path, 'saved')
            const studio = useStudioDocuments(async (_, content) => {
                await writeFile(path, content)
            }, expect.fail)
            await studio.open(document('document', kind))
            studio.updateActiveContent('unsaved')
            const old = studio.activeDocument.value!
            expect(await studio.deletePath('document', () => rm(path))).toBe(true)
            expect(studio.documents.value.length).toBe(0)
            expect(studio.activePath.value).toBe('')
            expect(await studio.saveDocument(old)).toBe(false)
            await studio.saveAll()
            await expect(readFile(path)).rejects.toMatchObject({ code: 'ENOENT' })
        } finally {
            await rm(directory, { recursive: true, force: true })
        }
    })
}

it('deletion waits for an in-flight write and blocks saves until it finishes', async () => {
    const write = deferred()
    const deletion = deferred()
    const events: string[] = []
    const studio = useStudioDocuments(async () => {
        events.push('write')
        await write.promise
        events.push('written')
    }, expect.fail)
    await studio.open(document('level.dat', 'nbt'))
    studio.updateActiveContent('edited')
    const saving = studio.saveActive()
    const deleting = studio.deletePath('level.dat', async () => {
        events.push('delete')
        await deletion.promise
    })
    expect(await studio.saveActive()).toBe(false)
    expect(events).toEqual(['write'])
    write.resolve()
    await saving
    await Promise.resolve()
    expect(events).toEqual(['write', 'written', 'delete'])
    expect(await studio.close('level.dat')).toBe(false)
    deletion.resolve()
    await deleting
    expect(events).toEqual(['write', 'written', 'delete'])
    expect(studio.documents.value.length).toBe(0)
})

it('directory deletion removes active and inactive descendants but keeps adjacent paths', async () => {
    const studio = useStudioDocuments(async () => {}, expect.fail)
    for (const path of ['config/a', 'config/b', 'configuration/keep'])
        await studio.open(document(path))
    studio.documents.value[0].content = 'dirty inactive'
    studio.documents.value[1].content = 'dirty active'
    await studio.activate('config/b')
    await studio.deletePath('config', async () => {})
    expect(studio.documents.value.map((document) => document.path)).toEqual(['configuration/keep'])
    expect(studio.activePath.value).toBe('configuration/keep')
})

it('failed deletion retains dirty content and allows a later save', async () => {
    const writes: string[] = []
    const studio = useStudioDocuments(async (_, content) => {
        writes.push(content)
    }, expect.fail)
    await studio.open(document('settings.json'))
    studio.updateActiveContent('unsaved content')
    await expect(
        studio.deletePath('settings.json', async () => {
            throw new Error('delete failed')
        }),
    ).rejects.toThrow(/delete failed/)
    expect(studio.activeDocument.value?.content).toBe('unsaved content')
    expect(studio.hasUnsavedChanges.value).toBe(true)
    expect(await studio.saveActive()).toBe(true)
    expect(writes).toEqual(['unsaved content'])
})

it('a read started before deletion cannot reopen a deleted descendant', async () => {
    const studio = useStudioDocuments(async () => {}, expect.fail)
    const version = studio.pathVersion('config/new.json')
    await studio.deletePath('config', async () => {})
    expect(await studio.open(document('config/new.json'), version)).toBe(false)
    expect(studio.documents.value.length).toBe(0)
    expect(await studio.open(document('config/new.json'))).toBe(true)
})

it('ordinary close saves edits and concurrent closes do not remove another tab', async () => {
    const pending = deferred()
    const writes: string[] = []
    const studio = useStudioDocuments(async (document, content) => {
        writes.push(`${document.path}:${content}`)
        await pending.promise
    }, expect.fail)
    await studio.open(document('a'))
    await studio.open(document('b'))
    studio.documents.value[0].content = 'edited'
    const first = studio.close('a')
    const second = studio.close('a')
    pending.resolve()
    await Promise.all([first, second])
    expect(writes).toEqual(['a:edited'])
    expect(studio.documents.value.map((document) => document.path)).toEqual(['b'])
    expect(studio.activePath.value).toBe('b')
})

it('a deletion finishing after a workspace reset does not close the new workspace document', async () => {
    const pending = deferred()
    const studio = useStudioDocuments(async () => {}, expect.fail)
    await studio.open(document('same-path'))
    const version = studio.pathVersion('same-path')
    const deleting = studio.deletePath('same-path', () => pending.promise)
    await Promise.resolve()
    studio.reset()
    await studio.open(document('same-path'))
    studio.updateActiveContent('new workspace edits')
    pending.resolve()
    await deleting
    expect(studio.activeDocument.value?.content).toBe('new workspace edits')
    expect(studio.canOpen('same-path', version)).toBe(false)
    expect(studio.isDeleting('same-path')).toBe(false)
})
