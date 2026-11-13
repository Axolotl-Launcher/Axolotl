import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, Suspense } from 'vue'

import type { GameInstance } from '@/helpers/types'

import FileStudio from './FileStudio.vue'

const studio = vi.hoisted(() => ({
    readDirectory: vi.fn(),
    readFile: vi.fn(),
    readStudioBinary: vi.fn(),
    readStudioText: vi.fn(),
    writeStudioBinary: vi.fn(),
    openPath: vi.fn(),
    notify: vi.fn(),
}))

// A real 1x1 transparent PNG: the preview has to decode actual image bytes, so a
// placeholder string would only prove the element exists.
const PNG_BYTES = Uint8Array.from(
    atob(
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
    ),
    (character) => character.charCodeAt(0),
)

vi.mock('vue-router', () => ({
    useRouter: () => ({ push: vi.fn(async () => {}) }),
    onBeforeRouteLeave: () => undefined,
}))
vi.mock('@tauri-apps/api/path', () => ({
    join: (...parts: string[]) => Promise.resolve(parts.join('/')),
}))
// The preview must not depend on the asset protocol: this stub mirrors what the
// real helper produces, so a regression to path-based URLs fails the assertion
// instead of throwing on an uninitialised IPC bridge.
vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string) => `asset://localhost/${path}`,
}))
vi.mock('@tauri-apps/plugin-fs', () => ({
    copyFile: vi.fn(),
    mkdir: vi.fn(),
    readDir: studio.readDirectory,
    readFile: studio.readFile,
    readTextFile: vi.fn(),
    remove: vi.fn(),
    rename: vi.fn(),
    watch: vi.fn(async () => () => {}),
    writeFile: vi.fn(),
    writeTextFile: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({ get_full_path: async () => 'C:/instances/instance' }))
vi.mock('@/helpers/studio', () => ({
    listenStudioFilesChanged: vi.fn(async () => () => {}),
    readStudioBinary: studio.readStudioBinary,
    readStudioText: studio.readStudioText,
    registerStudioWatcher: vi.fn(async () => 'registration'),
    trashStudioFile: vi.fn(),
    unregisterStudioWatcher: vi.fn(async () => {}),
    writeStudioBinary: studio.writeStudioBinary,
}))
vi.mock('@/helpers/utils', () => ({ highlightInFolder: vi.fn(), openPath: studio.openPath }))
vi.mock('@/components/instance/studio/StudioEditor.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/instance/studio/NbtEditor.vue', () => ({ default: { render: () => null } }))
// The editor is a fabric canvas with its own test; this page only has to hand it
// the bytes and write the result back.
const EDITED_BYTES = Uint8Array.from([7, 7, 7])
vi.mock('@/components/instance/studio/StudioImageEditor.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            name: 'StudioImageEditor',
            props: { content: Object, mimeType: String, name: String },
            emits: ['save', 'cancel', 'error', 'dirty'],
            setup:
                (_, { emit }) =>
                () =>
                    h('div', [
                        h('button', {
                            'data-testid': 'editor-save',
                            onClick: () => emit('save', EDITED_BYTES),
                        }),
                        h('button', {
                            'data-testid': 'editor-dirty',
                            onClick: () => emit('dirty', true),
                        }),
                    ]),
        }),
    }
})
vi.mock('@/components/instance/studio/StudioEditorTabs.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent } = await import('vue')
    const passThrough = (name: string) =>
        defineComponent({
            name,
            setup: (_, { slots, expose }) => {
                // The page opens the discard prompt through the template ref.
                expose({ show: () => undefined, hide: () => undefined })
                return () => h('div', [slots.default?.(), slots.actions?.()])
            },
        })
    return {
        Button: defineComponent({
            name: 'Button',
            setup:
                (_, { slots, emit }) =>
                () =>
                    h('button', { onClick: () => emit('click') }, slots.default?.()),
        }),
        NewModal: passThrough('NewModal'),
        PointMenu: defineComponent({
            name: 'PointMenu',
            setup: (_, { expose }) => {
                expose({ show: () => undefined, close: () => undefined })
                return () => null
            },
        }),
        StyledInput: defineComponent({ name: 'StyledInput', setup: () => () => null }),
        commonMessages: {
            saveButton: { id: 'common.save', defaultMessage: 'Save' },
            deleteLabel: { id: 'common.delete', defaultMessage: 'Delete' },
        },
        defineMessages: (messages: unknown) => messages,
        getEditorLanguage: () => 'plaintext',
        injectNotificationManager: () => ({ addNotification: studio.notify }),
        useFormatBytes: () => (bytes: number) => `${bytes} bytes`,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const mounted: VueWrapper[] = []
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.restoreAllMocks()
})

beforeEach(() => {
    vi.clearAllMocks()
    studio.readDirectory.mockImplementation(async (path: string) =>
        path.endsWith('/screenshots')
            ? [
                  { name: 'block.png', isDirectory: false },
                  { name: 'pack.jar', isDirectory: false },
              ]
            : [{ name: 'screenshots', isDirectory: true }],
    )
    studio.readStudioBinary.mockResolvedValue(PNG_BYTES)
    studio.readFile.mockResolvedValue(PNG_BYTES)
    studio.readStudioText.mockRejectedValue(new Error('File is not a text file'))
})

async function mountStudio(props: Record<string, unknown>) {
    const wrapper = mount(
        defineComponent({
            setup: () => () => h(Suspense, {}, { default: () => h(FileStudio, props) }),
        }),
        { global: { directives: { tooltip: {} } } },
    )
    mounted.push(wrapper)
    await flushPromises()
    return wrapper
}

async function openScreenshot(wrapper: VueWrapper) {
    await openTreeFile(wrapper, 'screenshots')
    await openTreeFile(wrapper, 'screenshots/block.png')
}

async function openTreeFile(wrapper: VueWrapper, path: string) {
    const node = wrapper
        .findAll('[data-studio-path]')
        .find((candidate) => candidate.attributes('data-studio-path') === path)
    if (!node) throw new Error(`the explorer does not list ${path}`)
    await node.trigger('click')
    await flushPromises()
}

it('previews an instance image from the bytes it reads through the file IPC', async () => {
    const wrapper = await mountStudio({ instance: { id: 'instance' } as unknown as GameInstance })
    await openScreenshot(wrapper)

    const image = wrapper.find('img')
    expect(image.exists()).toBe(true)
    // The asset protocol refuses files outside its scope, which is what left the
    // webview showing its broken-image placeholder with the file name.
    expect(image.attributes('src')).toMatch(/^blob:/)
    expect(image.attributes('alt')).toBe('block.png')
    expect(studio.readStudioBinary).toHaveBeenCalledWith('instance', 'screenshots/block.png')
})

it('previews a server workspace image through the scoped file read', async () => {
    const wrapper = await mountStudio({
        server: { id: 'server', path: 'E:/servers/survival', running: false, starting: false },
    })
    await openScreenshot(wrapper)

    expect(wrapper.find('img').attributes('src')).toMatch(/^blob:/)
    expect(studio.readFile).toHaveBeenCalledWith('E:/servers/survival/screenshots/block.png')
})

it('reports an unreadable image instead of leaving an empty image element', async () => {
    studio.readStudioBinary.mockRejectedValue(new Error('permission denied'))
    const wrapper = await mountStudio({ instance: { id: 'instance' } as unknown as GameInstance })
    await openScreenshot(wrapper)

    expect(wrapper.text()).toContain('This file could not be previewed.')
    expect(wrapper.find('img').exists()).toBe(false)
    expect(wrapper.findAll('button').some((button) => button.text() === 'Open in system')).toBe(
        true,
    )
})

it('releases the previous preview when another file becomes active', async () => {
    const revoke = vi.spyOn(URL, 'revokeObjectURL')
    const wrapper = await mountStudio({ instance: { id: 'instance' } as unknown as GameInstance })
    await openScreenshot(wrapper)
    const previewUrl = wrapper.find('img').attributes('src')

    await openTreeFile(wrapper, 'screenshots/pack.jar')

    expect(revoke).toHaveBeenCalledWith(previewUrl)
})

it('refuses to write an edit over a file that changed since it was opened', async () => {
    const wrapper = await mountStudio({ instance: { id: 'instance' } as unknown as GameInstance })
    await openScreenshot(wrapper)

    // Another program replaced the file while the editor was still open, so the
    // bytes the edit was drawn over are no longer what the file holds.
    studio.readStudioBinary.mockResolvedValue(Uint8Array.from([...PNG_BYTES, 1]))

    const edit = wrapper.findAll('button').find((button) => button.text() === 'Edit')
    if (!edit) throw new Error('the preview does not offer the editor')
    await edit.trigger('click')
    await wrapper.find('[data-testid="editor-save"]').trigger('click')
    await flushPromises()

    expect(studio.writeStudioBinary).not.toHaveBeenCalled()
    expect(studio.notify).toHaveBeenCalled()
})

it('asks before a file switch discards an image edit', async () => {
    const wrapper = await mountStudio({ instance: { id: 'instance' } as unknown as GameInstance })
    await openScreenshot(wrapper)

    const edit = wrapper.findAll('button').find((button) => button.text() === 'Edit')
    if (!edit) throw new Error('the preview does not offer the editor')
    await edit.trigger('click')
    await wrapper.find('[data-testid="editor-dirty"]').trigger('click')

    await openTreeFile(wrapper, 'screenshots/pack.jar')
    // The switch waits for an answer instead of unmounting the canvas, which
    // would take the edit with it.
    expect(wrapper.find('[data-testid="editor-save"]').exists()).toBe(true)

    const discard = wrapper.findAll('button').find((button) => button.text() === 'Discard')
    if (!discard) throw new Error('the prompt does not offer discarding')
    await discard.trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-testid="editor-save"]').exists()).toBe(false)
})

it('keeps the edit open when the write fails', async () => {
    studio.writeStudioBinary.mockRejectedValue(new Error('disk full'))
    const wrapper = await mountStudio({ instance: { id: 'instance' } as unknown as GameInstance })
    await openScreenshot(wrapper)

    const edit = wrapper.findAll('button').find((button) => button.text() === 'Edit')
    if (!edit) throw new Error('the preview does not offer the editor')
    await edit.trigger('click')
    await wrapper.find('[data-testid="editor-save"]').trigger('click')
    await flushPromises()

    expect(studio.notify).toHaveBeenCalled()
    // The file was not written, so the canvas still holds the only copy of the
    // edit; closing it here would lose the user's work without a way back.
    expect(wrapper.find('[data-testid="editor-save"]').exists()).toBe(true)
})

it('writes an image edit to the file the editor was opened for', async () => {
    const wrapper = await mountStudio({ instance: { id: 'instance' } as unknown as GameInstance })
    await openScreenshot(wrapper)

    const edit = wrapper.findAll('button').find((button) => button.text() === 'Edit')
    if (!edit) throw new Error('the preview does not offer the editor')
    await edit.trigger('click')
    await wrapper.find('[data-testid="editor-save"]').trigger('click')
    await flushPromises()

    // The instance is addressed by the id captured with the file, so a page that
    // navigated to another instance while the edit was encoding still writes
    // where the edit came from.
    expect(studio.writeStudioBinary).toHaveBeenCalledWith(
        'instance',
        'screenshots/block.png',
        EDITED_BYTES,
    )
})
