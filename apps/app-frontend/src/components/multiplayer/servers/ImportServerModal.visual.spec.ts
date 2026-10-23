import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { PropType } from 'vue'

import ImportServerModal from './ImportServerModal.vue'

const fixture = vi.hoisted(() => ({
    inspect: vi.fn(),
    importServer: vi.fn(),
    refresh: vi.fn(),
    push: vi.fn(),
    games: vi.fn(),
    loaders: vi.fn(),
}))
vi.mock('@/helpers/servers', () => ({
    serverEventListener: vi.fn(),
    servers: { inspectImport: fixture.inspect, importExternal: fixture.importServer },
}))
vi.mock('@/helpers/metadata', () => ({
    get_game_versions: fixture.games,
    get_loader_versions: fixture.loaders,
}))
vi.mock('@/composables/useServers', () => ({ refresh: fixture.refresh }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: fixture.push }) }))
vi.mock('@/helpers/jre', () => ({
    get_java_default_versions: async () => [
        { parsed_version: 25, version: '25', path: '/fixture/java25' },
        { parsed_version: 17, version: '17', path: '/fixture/java17' },
        { parsed_version: 8, version: '1.8.0', path: '/fixture/java8' },
    ],
    find_filtered_jres: async () => [],
    get_max_memory: async () => 16 * 1024 * 1024,
}))
vi.mock('@/components/ui/JavaSelector.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            name: 'JavaSelector',
            props: ['modelValue'],
            setup: (props) => () => h('span', props.modelValue.path),
        }),
    }
})
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h, ref } = await import('vue')
    const { loaderVersionsForGameVersion } =
        await import('../../../../../../packages/ui/src/components/flows/creation-flow-modal/loader-metadata.ts')
    return {
        loaderVersionsForGameVersion,
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
        Combobox: defineComponent({
            name: 'Combobox',
            props: {
                modelValue: String,
                options: {
                    type: Array as PropType<Array<{ value: string; label: string }>>,
                    required: true,
                },
                disabled: Boolean,
                searchable: Boolean,
            },
            emits: ['update:modelValue', 'searchInput', 'searchBlur'],
            setup:
                (props, { emit }) =>
                () =>
                    props.searchable
                        ? h('input', {
                              value: props.modelValue,
                              disabled: props.disabled,
                              onInput: (event: Event) =>
                                  emit('searchInput', (event.target as HTMLInputElement).value),
                              onBlur: () => emit('searchBlur', props.modelValue),
                          })
                        : h(
                              'select',
                              {
                                  value: props.modelValue,
                                  disabled: props.disabled,
                                  onChange: (event: Event) =>
                                      emit(
                                          'update:modelValue',
                                          (event.target as HTMLSelectElement).value,
                                      ),
                              },
                              [
                                  h('option', { value: '' }, ''),
                                  ...props.options.map((option: { value: string; label: string }) =>
                                      h('option', { value: option.value }, option.label),
                                  ),
                              ],
                          ),
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
    fixture.games.mockResolvedValue({ versions: [{ id: '1.12.2' }, { id: '1.20.1' }] })
    fixture.loaders.mockResolvedValue({
        gameVersions: [{ id: '1.12.2', loaders: [{ id: '14.23.5.2860' }] }],
    })
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

function resourceOnlyPreview() {
    return {
        ...preview(),
        innerBase: '',
        serverType: 'unknown',
        gameVersion: '',
        loaderVersion: null,
        problem: 'configuration-required',
    }
}
it('requires explicit configuration and sends it separately from detected evidence', async () => {
    fixture.inspect.mockResolvedValue(resourceOnlyPreview())
    fixture.importServer.mockResolvedValue({ id: 'manual-id', installState: null })
    await open()
    expect(submitButton().attributes('disabled')).toBeDefined()
    await wrapper.get('#import-server-type').setValue('forge')
    await wrapper.get('#import-server-game').setValue('1.12.2')
    await wrapper.get('#import-server-game').trigger('blur')
    await flushPromises()
    expect(wrapper.text()).toContain('/fixture/java8')
    expect(submitButton().attributes('disabled')).toBeDefined()
    await wrapper.get('#import-server-loader').setValue('14.23.5.2864')
    await submitButton().trigger('click')
    await flushPromises()
    expect(fixture.importServer).toHaveBeenCalledWith({
        sourcePath: '/fixture/external.zip',
        expected: resourceOnlyPreview(),
        configuration: {
            serverType: 'forge',
            gameVersion: '1.12.2',
            loaderVersion: '14.23.5.2864',
        },
        name: 'external',
        javaPath: '/fixture/java8',
        memoryMb: 4096,
    })
})
it('rejects a newer Java for legacy Forge before importing', async () => {
    fixture.inspect.mockResolvedValue(resourceOnlyPreview())
    await open()
    await wrapper.get('#import-server-type').setValue('forge')
    await wrapper.get('#import-server-game').setValue('1.12.2')
    await wrapper.get('#import-server-game').trigger('blur')
    await wrapper.get('#import-server-loader').setValue('14.23.5.2864')
    await flushPromises()
    wrapper
        .findComponent({ name: 'JavaSelector' })
        .vm.$emit('update:modelValue', { path: '/fixture/java17', version: '17' })
    await flushPromises()
    await submitButton().trigger('click')
    await flushPromises()
    expect(fixture.importServer).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('Select Java {version}')
})

it('offers scoped versions without choosing a loader and preserves custom input', async () => {
    fixture.inspect.mockResolvedValue(resourceOnlyPreview())
    await open()
    const game = wrapper.findAllComponents({ name: 'Combobox' })
    expect(game.length).toBeGreaterThan(0)
    await wrapper.get('#import-server-type').setValue('forge')
    await wrapper.get('#import-server-game').setValue('1.12.2')
    await vi.waitFor(() => expect(fixture.loaders).toHaveBeenCalledWith('forge', '1.12.2'))
    await flushPromises()
    const loader = wrapper
        .findAllComponents({ name: 'Combobox' })
        .find((component) => component.attributes('id') === 'import-server-loader')!
    expect(loader.props('options')).toEqual([{ value: '14.23.5.2860', label: '14.23.5.2860' }])
    expect(loader.props('modelValue')).toBe('')
    await wrapper.get('#import-server-loader').setValue('14.23.5.custom')
    await wrapper.get('#import-server-loader').trigger('blur')
    expect(loader.props('modelValue')).toBe('14.23.5.custom')
})
it('keeps manual versions usable when metadata cannot be loaded', async () => {
    fixture.games.mockRejectedValue(new Error('Metadata offline'))
    fixture.loaders.mockRejectedValue(new Error('Metadata offline'))
    fixture.inspect.mockResolvedValue(resourceOnlyPreview())
    await open()
    await wrapper.get('#import-server-type').setValue('fabric')
    await wrapper.get('#import-server-game').setValue('custom-game')
    await vi.waitFor(() => expect(fixture.loaders).toHaveBeenCalledWith('fabric', 'custom-game'))
    await wrapper.get('#import-server-loader').setValue('custom-loader')
    expect(wrapper.get<HTMLInputElement>('#import-server-loader').element.value).toBe(
        'custom-loader',
    )
    expect(wrapper.text()).toContain('Metadata offline')
})

it('uses shared version-group metadata without choosing a loader automatically', async () => {
    fixture.inspect.mockResolvedValue(resourceOnlyPreview())
    fixture.loaders.mockResolvedValue({
        gameVersions: [{ id: '1.21.1', versionGroup: 'fabric', loaders: [] }],
        versionGroups: [{ id: 'fabric', loaders: [{ id: '0.16.14', stable: true }] }],
    })
    await open()
    await wrapper.get('#import-server-type').setValue('fabric')
    await wrapper.get('#import-server-game').setValue('1.21.1')
    await vi.waitFor(() => expect(fixture.loaders).toHaveBeenCalledWith('fabric', '1.21.1'))
    await flushPromises()
    const loader = wrapper
        .findAllComponents({ name: 'Combobox' })
        .find((component) => component.attributes('id') === 'import-server-loader')!
    expect(loader.props('options')).toEqual([{ value: '0.16.14', label: '0.16.14' }])
    expect(loader.props('modelValue')).toBe('')
})
