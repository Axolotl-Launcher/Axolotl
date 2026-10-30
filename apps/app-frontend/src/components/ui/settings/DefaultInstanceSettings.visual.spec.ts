import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref, Suspense } from 'vue'

import DefaultInstanceSettings from './DefaultInstanceSettings.vue'

const backend = vi.hoisted(() => ({
    settings: {} as Record<string, unknown>,
    patches: [] as Record<string, unknown>[],
}))

vi.mock('@/helpers/settings.ts', () => ({
    get: async () => structuredClone(backend.settings),
    update: async (patch: Record<string, unknown>) => {
        backend.patches.push(patch)
        Object.assign(backend.settings, structuredClone(patch))
    },
}))
vi.mock('@tauri-apps/plugin-os', () => ({ platform: () => 'windows' }))
vi.mock('@/components/ui/EnvironmentVariablesInput.vue', () => ({
    default: { render: () => null },
}))
vi.mock('./LogShareSettings.vue', () => ({ default: { render: () => null } }))
vi.mock('./SharedLogsSettings.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const toggle = defineComponent({
        props: ['modelValue'],
        setup:
            (props, { emit }) =>
            () =>
                h('input', {
                    type: 'checkbox',
                    'aria-label': 'toggle',
                    checked: props.modelValue,
                    onChange: (event: Event) =>
                        emit('update:modelValue', (event.target as HTMLInputElement).checked),
                }),
    })
    const combobox = defineComponent({
        props: ['modelValue', 'options'],
        setup:
            (props, { emit }) =>
            () =>
                h(
                    'select',
                    {
                        value: props.modelValue,
                        onChange: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLSelectElement).value),
                    },
                    (
                        props.options as {
                            value: string
                            label: string
                        }[]
                    ).map((option) => h('option', { value: option.value }, option.label)),
                ),
    })
    return {
        Combobox: combobox,
        Toggle: toggle,
        Card: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', slots.default?.()),
        }),
        StyledInput: defineComponent({
            props: ['modelValue', 'type'],
            setup(props, { emit, expose }) {
                const input = ref<HTMLInputElement>()
                // The real StyledInput exposes both, and the component uses them to put
                // the caret back into a field it had to re-mount.
                expose({
                    focus: () => input.value?.focus(),
                    select: () => input.value?.select(),
                })
                return () =>
                    h('input', {
                        ref: input,
                        value: props.modelValue,
                        type: props.type,
                        // Mirrors the real StyledInput: a cleared number field reports
                        // `undefined`, unparsable text reports `NaN`.
                        onInput: (event: Event) => {
                            const element = event.target as HTMLInputElement
                            emit(
                                'update:modelValue',
                                props.type === 'number'
                                    ? element.value === ''
                                        ? undefined
                                        : Number(element.value)
                                    : element.value,
                            )
                        },
                    })
            },
        }),
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))

async function openSettings(resolution: [number, number], overrides: Record<string, unknown> = {}) {
    backend.settings = {
        default_game_language: '',
        force_fullscreen: false,
        maximize_window: false,
        game_resolution: [...resolution],
        custom_window_title_enabled: false,
        hooks: { pre_launch: null, wrapper: null, post_exit: null },
        custom_dir: null,
        ...overrides,
    }
    backend.patches = []
    const wrapper = mount(
        defineComponent({
            setup: () => () => h(Suspense, {}, { default: () => h(DefaultInstanceSettings) }),
        }),
    )
    mounted.push(wrapper)
    await flushPromises()
    return wrapper
}

it('never sends a cleared window dimension to the settings command', async () => {
    const wrapper = await openSettings([1920, 1080])
    // `game_resolution` deserializes into `WindowSize(u16, u16)`; a cleared number input
    // reports `undefined`, which becomes JSON `null` and makes `settings_set` fail.
    await wrapper.get('#width').setValue('')
    await flushPromises()
    expect(backend.patches).toEqual([])
    expect(backend.settings.game_resolution).toEqual([1920, 1080])
    expect(wrapper.get<HTMLInputElement>('#width').element.value).toBe('1920')

    await wrapper.get('#height').setValue('')
    await flushPromises()
    expect(backend.patches).toEqual([])
    expect(backend.settings.game_resolution).toEqual([1920, 1080])
    expect(wrapper.get<HTMLInputElement>('#height').element.value).toBe('1080')
})

it('never sends a dimension the u16 field cannot hold', async () => {
    const wrapper = await openSettings([1920, 1080])
    for (const invalid of ['1920.5', '-1', '0', '65536']) {
        await wrapper.get('#width').setValue(invalid)
        await flushPromises()
    }
    expect(backend.patches).toEqual([])
    expect(backend.settings.game_resolution).toEqual([1920, 1080])
})

it('saves a valid window dimension change', async () => {
    const wrapper = await openSettings([1920, 1080])
    await wrapper.get('#width').setValue('1280')
    await flushPromises()
    expect(backend.patches).toEqual([{ game_resolution: [1280, 1080] }])
    expect(backend.settings.game_resolution).toEqual([1280, 1080])
})

it('defaults the game language to following the launcher', async () => {
    const wrapper = await openSettings([1920, 1080])
    const select = wrapper.get<HTMLSelectElement>('#default-game-language')

    expect(select.element.value).toBe('follow-launcher')
    expect(select.find('option[value="follow-launcher"]').text()).toBe('Follow launcher language')
})

it('saves the selected default game language and the follow option', async () => {
    const wrapper = await openSettings([1920, 1080])
    const select = wrapper.get<HTMLSelectElement>('#default-game-language')

    await select.setValue('en_us')
    await flushPromises()
    expect(backend.patches).toEqual([{ default_game_language: 'en_us' }])

    await select.setValue('follow-launcher')
    await flushPromises()
    expect(backend.patches).toEqual([
        { default_game_language: 'en_us' },
        { default_game_language: '' },
    ])
})

it('keeps an unknown stored game language selectable', async () => {
    const wrapper = await openSettings([1920, 1080], {
        default_game_language: 'custom_locale',
    })
    const select = wrapper.get<HTMLSelectElement>('#default-game-language')

    expect(select.element.value).toBe('custom_locale')
    expect(select.find('option[value="custom_locale"]').text()).toBe('custom_locale')
})
