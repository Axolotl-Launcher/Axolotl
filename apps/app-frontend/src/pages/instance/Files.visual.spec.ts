import type { FileManagerContext } from '@modrinth/ui'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, Suspense } from 'vue'

import Files from './Files.vue'

const filesystem = vi.hoisted(() => ({
    readDir: vi.fn(),
    stat: vi.fn(),
    context: undefined as FileManagerContext | undefined,
    unlisten: vi.fn(),
    frames: [] as FrameRequestCallback[],
}))

vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/path', () => ({
    join: async (...parts: string[]) => parts.join('/'),
    sep: () => '/',
}))
vi.mock('@tauri-apps/plugin-fs', () => ({
    mkdir: vi.fn(),
    readDir: filesystem.readDir,
    stat: filesystem.stat,
    readFile: vi.fn(),
    readTextFile: vi.fn(),
    remove: vi.fn(),
    rename: vi.fn(),
    writeTextFile: vi.fn(),
}))
vi.mock('@/helpers/instance', () => ({ get_full_path: async () => '/instance' }))
vi.mock('@/helpers/events', () => ({ instance_listener: async () => filesystem.unlisten }))
vi.mock('@/helpers/utils', () => ({ highlightInFolder: vi.fn() }))
vi.mock('@modrinth/ui', () => {
    const passThrough = (name: string) =>
        defineComponent({
            name,
            setup:
                (_, { slots }) =>
                () =>
                    h('div', { 'data-component': name }, slots.default?.()),
        })
    return {
        Button: passThrough('Button'),
        ReadyTransition: passThrough('ReadyTransition'),
        FilePageLayout: passThrough('FilePageLayout'),
        commonMessages: { beta: { defaultMessage: 'Beta' } },
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ addNotification: vi.fn() }),
        provideFileManager: (context: FileManagerContext) => (filesystem.context = context),
        useDebugLogger: () => () => {},
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const wrappers: VueWrapper[] = []
afterEach(() => {
    wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
    vi.unstubAllGlobals()
})

beforeEach(() => {
    vi.clearAllMocks()
    filesystem.context = undefined
    filesystem.frames = []
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
        filesystem.frames.push(callback)
        return filesystem.frames.length
    })
    filesystem.readDir.mockResolvedValue([{ name: 'schematics', isDirectory: true }])
    filesystem.stat.mockResolvedValue({ size: 0 })
})

async function mountFiles() {
    const wrapper = mount(
        defineComponent({
            setup: () => () =>
                h(
                    Suspense,
                    {},
                    {
                        default: () =>
                            h(Files, {
                                instance: { id: 'instance' } as never,
                                options: {},
                                offline: false,
                                playing: false,
                                installed: false,
                                isServerInstance: false,
                            }),
                    },
                ),
        }),
    )
    wrappers.push(wrapper)
    await flushPromises()
    return wrapper
}

it('mounts the file page while directory metadata is still loading', async () => {
    let complete: (value: unknown) => void = () => {}
    filesystem.stat.mockImplementation(() => new Promise((resolve) => (complete = resolve)))
    const wrapper = await mountFiles()

    expect(wrapper.find('[data-component="FilePageLayout"]').exists()).toBe(true)
    expect(filesystem.context?.loading.value).toBe(true)
    complete({ size: 0 })
    await flushPromises()
    expect(filesystem.context?.loading.value).toBe(false)
    expect(filesystem.readDir.mock.calls).toEqual([['/instance']])
})

it('does not commit a directory listing after navigation changes the current directory', async () => {
    let completeOld: (value: unknown) => void = () => {}
    filesystem.stat.mockImplementation((path: string) =>
        path === '/instance/schematics'
            ? new Promise((resolve) => (completeOld = resolve))
            : Promise.resolve({ size: 3 }),
    )
    filesystem.readDir.mockImplementation(async (path: string) =>
        path === '/instance'
            ? [{ name: 'schematics', isDirectory: true }]
            : [{ name: 'castle.litematic', isDirectory: false }],
    )
    await mountFiles()
    const context = filesystem.context!
    context.navigateTo('schematics')
    await flushPromises()
    expect(context.items.value.map((item) => item.path)).toEqual(['schematics/castle.litematic'])
    completeOld({ size: 0 })
    await flushPromises()
    expect(context.items.value.map((item) => item.path)).toEqual(['schematics/castle.litematic'])
})

it('stops old metadata workers when leaving the file page', async () => {
    filesystem.readDir.mockResolvedValue(
        Array.from({ length: 5000 }, (_, index) => ({
            name: `${index}.litematic`,
            isDirectory: false,
        })),
    )
    const completions: Array<(value: unknown) => void> = []
    filesystem.stat.mockImplementation(() => new Promise((resolve) => completions.push(resolve)))
    const wrapper = await mountFiles()
    const context = filesystem.context!
    expect(filesystem.stat).toHaveBeenCalledTimes(16)
    wrapper.unmount()
    wrappers.splice(wrappers.indexOf(wrapper), 1)
    completions.forEach((complete) => complete({ size: 1 }))
    await flushPromises()
    expect(filesystem.unlisten).toHaveBeenCalledOnce()
    expect(filesystem.stat).toHaveBeenCalledTimes(16)
    expect(context.items.value).toEqual([])
})

it('paints the file list before filling folder counts and cancels obsolete counting', async () => {
    filesystem.readDir.mockImplementation(async (path: string) =>
        path === '/instance'
            ? [{ name: 'schematics', isDirectory: true }]
            : [{ name: 'castle.litematic', isDirectory: false }],
    )
    await mountFiles()
    const context = filesystem.context!
    expect(context.loading.value).toBe(false)
    expect(context.items.value[0].count).toBeUndefined()
    expect(filesystem.readDir.mock.calls).toEqual([['/instance']])
    filesystem.frames.splice(0).forEach((callback) => callback(0))
    await new Promise((resolve) => setTimeout(resolve, 0))
    await flushPromises()
    expect(context.items.value[0].count).toBe(1)

    await context.refresh()
    context.navigateTo('schematics')
    await flushPromises()
    const calls = filesystem.readDir.mock.calls.length
    filesystem.frames.splice(0).forEach((callback) => callback(0))
    await new Promise((resolve) => setTimeout(resolve, 0))
    await flushPromises()
    expect(filesystem.readDir).toHaveBeenCalledTimes(calls)
    expect(context.items.value[0].path).toBe('schematics/castle.litematic')
})
