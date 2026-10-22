import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import ImportServerModal from './ImportServerModal.vue'

const fixture = vi.hoisted(() => ({
    inspect: vi.fn(),
    importServer: vi.fn(),
    refresh: vi.fn(),
    push: vi.fn(),
}))
vi.mock('@/helpers/servers', () => ({
    serverEventListener: vi.fn(),
    servers: { inspectImport: fixture.inspect, importExternal: fixture.importServer },
}))
vi.mock('@/composables/useServers', () => ({ refresh: fixture.refresh }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: fixture.push }) }))
vi.mock('@/helpers/jre', () => ({
    get_java_default_versions: async () => [
        { parsed_version: 25, version: '25', path: '/fixture/java25' },
        { parsed_version: 17, version: '17', path: '/fixture/java17' },
    ],
    find_filtered_jres: async () => [],
    get_max_memory: async () => 16 * 1024 * 1024,
}))
vi.mock('@/components/ui/JavaSelector.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: ['modelValue'],
            setup: (props) => () => h('span', props.modelValue.path),
        }),
    }
})
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref } = await import('vue')
    return {
        defineMessages: (value: unknown) => value,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
        commonMessages: { cancelButton: { defaultMessage: 'Cancel' } },
        NewModal: defineComponent({
            setup: (_, { slots, expose }) => {
                const open = ref(false)
                expose({
                    show: () => {
                        open.value = true
                    },
                    hide: () => {
                        open.value = false
                    },
                })
                return () =>
                    open.value ? h('section', [slots.default?.(), slots.actions?.()]) : null
            },
        }),
        Button: defineComponent({
            props: ['disabled'],
            setup:
                (props, { slots }) =>
                () =>
                    h('button', { disabled: props.disabled }, slots.default?.()),
        }),
        StyledInput: defineComponent({
            props: ['modelValue', 'disabled'],
            emits: ['update:modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h('input', {
                        value: props.modelValue,
                        disabled: props.disabled,
                        onInput: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLInputElement).value),
                    }),
        }),
        Slider: defineComponent({ setup: () => () => h('div') }),
    }
})

let wrapper: VueWrapper
const preview = () => ({
    innerBase: 'wrapper',
    serverType: 'forge',
    gameVersion: '1.20.1',
    loaderVersion: '47.4.16',
    jarName: null,
    needsInstall: true,
    problem: null,
})
beforeEach(() => {
    vi.clearAllMocks()
    fixture.inspect.mockResolvedValue(preview())
    fixture.refresh.mockResolvedValue(undefined)
    fixture.push.mockResolvedValue(undefined)
})
afterEach(() => wrapper?.unmount())
async function open() {
    wrapper = mount(ImportServerModal)
    await (wrapper.vm as unknown as { show: (path: string) => Promise<void> }).show(
        '/fixture/external.zip',
    )
    await flushPromises()
}
function submitButton() {
    return wrapper.findAll('button').find((button) => button.text() === 'Import and install')!
}
it('uses matching Java and passes exact detected versions without accepting EULA or starting', async () => {
    fixture.importServer.mockResolvedValue({ id: 'new-id', installState: null })
    await open()
    expect(wrapper.text()).toContain('/fixture/java17')
    await submitButton().trigger('click')
    await flushPromises()
    expect(fixture.importServer).toHaveBeenCalledWith({
        sourcePath: '/fixture/external.zip',
        expected: preview(),
        name: 'external',
        javaPath: '/fixture/java17',
        memoryMb: 4096,
    })
    expect(fixture.push).toHaveBeenCalledWith('/multiplayer/servers/new-id')
})
it('blocks conflicting or unsupported packages instead of guessing', async () => {
    fixture.inspect.mockResolvedValue({ ...preview(), problem: 'conflicting-versions' })
    await open()
    expect(wrapper.text()).toContain('conflicting server versions')
    expect(submitButton().attributes('disabled')).toBeDefined()
    expect(fixture.importServer).not.toHaveBeenCalled()
})
it('keeps failed imports reviewable without duplicating the server', async () => {
    fixture.importServer.mockResolvedValue({
        id: 'failed-id',
        installState: 'failed',
        installError: 'fixture failure',
    })
    await open()
    await submitButton().trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('Your imported files were kept')
    expect(fixture.push).not.toHaveBeenCalled()
    expect(wrapper.findAll('button').some((button) => button.text() === 'View server')).toBe(true)
    expect(fixture.importServer).toHaveBeenCalledTimes(1)
})
it('shows an actionable error when a manually selected source has no server', async () => {
    fixture.inspect.mockResolvedValue(null)
    await open()
    expect(wrapper.text()).toContain('No supported server was found')
    expect(fixture.importServer).not.toHaveBeenCalled()
})
