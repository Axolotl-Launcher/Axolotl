import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import FileEditor from '../layouts/shared/files-tab/components/editor/FileEditor.vue'
import FilesLayout from '../layouts/shared/files-tab/layout.vue'
import { mountThemed, waitFor } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() => {
    cleanup.splice(0).forEach((fn) => fn())
})

function modalButton(label: string) {
    return [...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')].find(
        (button) => button.textContent?.trim() === label,
    )
}

it('keeps editing and asks again when saving before leaving fails', async () => {
    const writeFile = vi.fn<(path: string, content: string) => Promise<void>>()
    const stopEditing = vi.fn()
    const wrapper = await mountThemed(FilesLayout, {}, 'dark', {
        global: {
            provide: {
                [Symbol.for('modrinth:fileManagerContext')]: {
                    items: ref([]),
                    loading: ref(false),
                    error: ref(null),
                    currentPath: ref('/'),
                    navigateTo: vi.fn(),
                    editingFile: ref({ name: 'options.txt', path: 'options.txt' }),
                    startEditing: vi.fn(),
                    stopEditing,
                    readFile: async () => 'original',
                    readFileAsBlob: vi.fn(),
                    writeFile,
                    refresh: vi.fn(),
                    isBusy: ref(false),
                },
                [Symbol.for('modrinth:notificationManager')]: { addNotification: vi.fn() },
                [Symbol.for('modrinth:modrinthClient')]: {},
            },
        },
    })
    cleanup.push(() => wrapper.unmount())
    await flushPromises()
    const editor = wrapper.findComponent(FileEditor)
    editor.vm.fileContent = 'edited'
    await flushPromises()

    writeFile.mockRejectedValueOnce(new Error('disk full'))
    editor.vm.$emit('close')
    await waitFor(() => !!modalButton('Discard'))
    modalButton('Save')!.click()
    await waitFor(() => writeFile.mock.calls.length === 1)
    await flushPromises()
    expect(stopEditing).not.toHaveBeenCalled()
    expect(editor.vm.fileContent).toBe('edited')
    await waitFor(() => !!modalButton('Discard'))

    writeFile.mockResolvedValueOnce()
    modalButton('Save')!.click()
    await waitFor(() => stopEditing.mock.calls.length === 1)
    expect(writeFile).toHaveBeenLastCalledWith('/options.txt', 'edited')
})
