import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import FileEditor from '../layouts/shared/files-tab/components/editor/FileEditor.vue'
import { mountThemed } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() => {
    cleanup.splice(0).forEach((fn) => fn())
})

function deferred<T = void>() {
    let resolve!: (value: T) => void
    let reject!: (error: unknown) => void
    const promise = new Promise<T>((done, fail) => {
        resolve = done
        reject = fail
    })
    return { promise, resolve, reject }
}

async function mountEditor(readFile: (path: string) => Promise<string>) {
    const writeFile = vi.fn<(path: string, content: string) => Promise<void>>()
    const notify = vi.fn()
    const wrapper = await mountThemed(
        FileEditor,
        { file: { name: 'options.txt', path: 'options.txt' }, editorComponent: null },
        'dark',
        {
            global: {
                provide: {
                    [Symbol.for('modrinth:fileManagerContext')]: {
                        isBusy: ref(false),
                        readFile,
                        readFileAsBlob: vi.fn(),
                        writeFile,
                    },
                    [Symbol.for('modrinth:notificationManager')]: { addNotification: notify },
                    [Symbol.for('modrinth:modrinthClient')]: {},
                },
            },
        },
    )
    cleanup.push(() => wrapper.unmount())
    await flushPromises()
    return { wrapper, writeFile, notify }
}

async function type(wrapper: Awaited<ReturnType<typeof mountEditor>>['wrapper'], text: string) {
    await wrapper.find('textarea').setValue(text)
}

it('keeps the edits unsaved and reports failure when the write fails', async () => {
    const { wrapper, writeFile, notify } = await mountEditor(async () => 'original')
    await type(wrapper, 'edited')
    writeFile.mockRejectedValueOnce(new Error('disk full'))
    await expect(wrapper.vm.saveFileContent(true)).resolves.toBe(false)
    expect(wrapper.vm.hasUnsavedChanges).toBe(true)
    expect(wrapper.emitted('close')).toBeUndefined()
    expect(notify).toHaveBeenCalledWith(expect.objectContaining({ type: 'error' }))
})

it('marks only the written snapshot as saved', async () => {
    const { wrapper, writeFile } = await mountEditor(async () => 'original')
    await type(wrapper, 'first')
    const write = deferred()
    writeFile.mockReturnValueOnce(write.promise)
    const saved = wrapper.vm.saveFileContent(true)
    expect(wrapper.vm.saving).toBe(true)
    await type(wrapper, 'first and more')
    write.resolve()
    await expect(saved).resolves.toBe(true)
    expect(writeFile).toHaveBeenCalledExactlyOnceWith('/options.txt', 'first')
    expect(wrapper.vm.hasUnsavedChanges).toBe(true)
    expect(wrapper.emitted('close')).toBeUndefined()
    wrapper.vm.revertChanges()
    await flushPromises()
    expect(wrapper.vm.fileContent).toBe('first')
})

it('a save requested while another writes waits for it and writes only what is still unsaved', async () => {
    const { wrapper, writeFile } = await mountEditor(async () => 'original')
    await type(wrapper, 'first')
    const write = deferred()
    writeFile.mockReturnValueOnce(write.promise).mockResolvedValueOnce()
    const first = wrapper.vm.saveFileContent(false)
    const repeated = wrapper.vm.saveFileContent(false)
    const later = wrapper.vm.saveFileContent(true)
    write.resolve()
    await expect(first).resolves.toBe(true)
    await expect(repeated).resolves.toBe(true)
    await expect(later).resolves.toBe(true)
    expect(writeFile).toHaveBeenCalledExactlyOnceWith('/options.txt', 'first')
    expect(wrapper.emitted('close')).toHaveLength(1)

    const second = deferred()
    writeFile.mockReturnValueOnce(second.promise).mockResolvedValueOnce()
    const pending = wrapper.vm.saveFileContent(false)
    await type(wrapper, 'second edit')
    const following = wrapper.vm.saveFileContent(false)
    second.resolve()
    await expect(pending).resolves.toBe(true)
    await expect(following).resolves.toBe(true)
    expect(writeFile).toHaveBeenLastCalledWith('/options.txt', 'second edit')
    expect(wrapper.vm.hasUnsavedChanges).toBe(false)
})

it('does not let a slow load of the previous file replace the file opened after it', async () => {
    const first = deferred<string>()
    const reads: Record<string, Promise<string>> = {
        '/options.txt': first.promise,
        '/servers.txt': Promise.resolve('servers'),
    }
    const { wrapper } = await mountEditor((path) => reads[path])
    await wrapper.setProps({ file: { name: 'servers.txt', path: 'servers.txt' } })
    await flushPromises()
    first.resolve('options')
    await flushPromises()
    expect(wrapper.vm.fileContent).toBe('servers')
    expect(wrapper.vm.hasUnsavedChanges).toBe(false)
})

it('keeps loading while a newer file is still being read after an earlier read finishes', async () => {
    const first = deferred<string>()
    const second = deferred<string>()
    const reads: Record<string, Promise<string>> = {
        '/options.txt': first.promise,
        '/servers.txt': second.promise,
    }
    const { wrapper } = await mountEditor((path) => reads[path])
    await wrapper.setProps({ file: { name: 'servers.txt', path: 'servers.txt' } })
    first.resolve('options')
    await flushPromises()
    expect(wrapper.find('textarea').exists()).toBe(false)
    second.resolve('servers')
    await flushPromises()
    expect((wrapper.find('textarea').element as HTMLTextAreaElement).value).toBe('servers')
})
