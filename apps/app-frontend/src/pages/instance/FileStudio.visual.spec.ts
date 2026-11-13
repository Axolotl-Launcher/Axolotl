import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, Suspense } from 'vue'

import type { GameInstance } from '@/helpers/types'

import FileStudio from './FileStudio.vue'

const studio = vi.hoisted(() => ({
    push: vi.fn(async () => {}),
    readDirectory: vi.fn(),
    readTextFile: vi.fn(),
    readStudioText: vi.fn(),
    openPath: vi.fn(),
}))

vi.mock('vue-router', () => ({
    useRouter: () => ({ push: studio.push }),
    onBeforeRouteLeave: () => undefined,
}))
vi.mock('@tauri-apps/api/path', () => ({
    join: (...parts: string[]) => Promise.resolve(parts.join('/')),
}))
vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: (path: string) => path }))
vi.mock('@tauri-apps/plugin-fs', () => ({
    copyFile: vi.fn(),
    mkdir: vi.fn(),
    readDir: studio.readDirectory,
    readFile: vi.fn(),
    readTextFile: studio.readTextFile,
    remove: vi.fn(),
    rename: vi.fn(),
    watch: vi.fn(async () => () => {}),
    writeFile: vi.fn(),
    writeTextFile: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({ get_full_path: async () => 'C:/instances/instance' }))
vi.mock('@/helpers/studio', () => ({
    listenStudioFilesChanged: vi.fn(async () => () => {}),
    readStudioBinary: vi.fn(),
    readStudioText: studio.readStudioText,
    registerStudioWatcher: vi.fn(async () => 'registration'),
    trashStudioFile: vi.fn(),
    unregisterStudioWatcher: vi.fn(async () => {}),
    writeStudioBinary: vi.fn(),
}))
vi.mock('@/helpers/utils', () => ({ highlightInFolder: vi.fn(), openPath: studio.openPath }))
vi.mock('@/components/instance/studio/StudioEditor.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/instance/studio/NbtEditor.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/instance/studio/StudioImageEditor.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/instance/studio/StudioEditorTabs.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent } = await import('vue')
    const passThrough = (name: string) =>
        defineComponent({
            name,
            setup:
                (_, { slots }) =>
                () =>
                    h('div', slots.default?.()),
        })
    return {
        // The button stub keeps the modifiers the page chooses visible as attributes, so
        // the emphasis can be asserted instead of inferred from a snapshot.
        Button: defineComponent({
            name: 'Button',
            props: { type: String, color: String, size: String, disabled: Boolean },
            setup:
                (props, { slots }) =>
                () =>
                    h(
                        'button',
                        {
                            'data-button-type': props.type ?? 'standard',
                            'data-button-color': props.color ?? '',
                        },
                        slots.default?.(),
                    ),
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
        injectNotificationManager: () => ({ addNotification: vi.fn() }),
        useFormatBytes: () => (bytes: number) => `${bytes} bytes`,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const mounted: VueWrapper[] = []
afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
})

beforeEach(() => {
    vi.clearAllMocks()
    studio.readDirectory.mockImplementation(async (path: string) =>
        path.endsWith('/schematics')
            ? [
                  { name: 'castle.litematic', isDirectory: false },
                  { name: 'region.mca', isDirectory: false },
              ]
            : [{ name: 'schematics', isDirectory: true }],
    )
    studio.readStudioText.mockRejectedValue(new Error('File is not a text file'))
    studio.readTextFile.mockRejectedValue(new Error('File is not a UTF-8 text file'))
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

function treeNode(wrapper: VueWrapper, path: string) {
    const node = wrapper
        .findAll('[data-studio-path]')
        .find((candidate) => candidate.attributes('data-studio-path') === path)
    if (!node) throw new Error(`the explorer does not list ${path}`)
    return node
}

async function openBinaryFile(wrapper: VueWrapper, name: string) {
    await treeNode(wrapper, 'schematics').trigger('click')
    await flushPromises()
    await treeNode(wrapper, `schematics/${name}`).trigger('click')
    await flushPromises()
}

function buttonLabelled(wrapper: VueWrapper, label: string) {
    return wrapper.findAll('button').find((button) => button.text() === label)
}

it('hands a schematic Studio cannot read to the workshop instead of only the file manager', async () => {
    const wrapper = await mountStudio({ instance: { id: 'instance' } as unknown as GameInstance })
    await openBinaryFile(wrapper, 'castle.litematic')

    expect(wrapper.text()).toContain('This file is not a text file and cannot be opened.')

    const workshop = buttonLabelled(wrapper, 'Open with schematic workshop')
    expect(workshop).toBeDefined()
    expect(workshop!.attributes('data-button-type')).toBe('colored')
    expect(workshop!.attributes('data-button-color')).toBe('brand')
    expect(buttonLabelled(wrapper, 'Open in system')).toBeDefined()

    await workshop!.trigger('click')
    await flushPromises()
    expect(studio.push).toHaveBeenCalledWith({
        name: 'SchematicWorkshop',
        query: { instance: 'instance', path: 'schematics/castle.litematic' },
    })
})

it('leaves a binary file the workshop cannot open to the file manager alone', async () => {
    const wrapper = await mountStudio({ instance: { id: 'instance' } as unknown as GameInstance })
    await openBinaryFile(wrapper, 'region.mca')

    expect(wrapper.text()).toContain('This file is not a text file and cannot be opened.')
    expect(buttonLabelled(wrapper, 'Open in system')).toBeDefined()
    expect(buttonLabelled(wrapper, 'Open with schematic workshop')).toBeUndefined()
})

it('does not offer the workshop from a server workspace, which has no instance to resolve', async () => {
    const wrapper = await mountStudio({
        server: { id: 'server', path: 'C:/servers/survival', running: false, starting: false },
    })
    await openBinaryFile(wrapper, 'castle.litematic')

    expect(buttonLabelled(wrapper, 'Open in system')).toBeDefined()
    expect(buttonLabelled(wrapper, 'Open with schematic workshop')).toBeUndefined()
})
