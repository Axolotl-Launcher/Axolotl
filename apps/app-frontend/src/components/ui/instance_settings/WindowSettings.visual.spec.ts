import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref, Suspense } from 'vue'

import type { GameInstance } from '@/helpers/types'

import WindowSettings from './WindowSettings.vue'

const backend = vi.hoisted(() => ({
    edit: vi.fn(),
    instance: null as unknown,
    persisted: {} as Partial<GameInstance>,
}))
vi.mock('@/providers/instance-settings', () => ({
    injectInstanceSettings: () => ({ instance: backend.instance }),
}))
vi.mock('@/helpers/instance', () => ({ edit: backend.edit }))
vi.mock('@tauri-apps/plugin-os', () => ({ platform: () => 'windows' }))
vi.mock('@/helpers/settings.ts', () => ({
    get: async () => ({
        force_fullscreen: true,
        maximize_window: true,
        game_resolution: [1280, 720],
        custom_window_title_enabled: true,
    }),
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const toggle = defineComponent({
        props: ['modelValue', 'label'],
        setup:
            (props, { emit }) =>
            () =>
                h('input', {
                    type: 'checkbox',
                    'aria-label': props.label,
                    checked: props.modelValue,
                    onChange: (event: Event) =>
                        emit('update:modelValue', (event.target as HTMLInputElement).checked),
                }),
    })
    return {
        Checkbox: toggle,
        Toggle: toggle,
        StyledInput: defineComponent({
            props: ['modelValue', 'type'],
            setup:
                (props, { emit }) =>
                () =>
                    h('input', {
                        value: props.modelValue,
                        type: props.type,
                        onInput: (event: Event) => {
                            const input = event.target as HTMLInputElement
                            emit(
                                'update:modelValue',
                                props.type === 'number' ? input.valueAsNumber : input.value,
                            )
                        },
                    }),
        }),
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))

async function settings(overrides: Partial<GameInstance>) {
    backend.persisted = {
        id: 'instance-a',
        force_fullscreen: null,
        maximize_window: null,
        game_resolution: null,
        window_title: null,
        ...overrides,
    }
    backend.edit
        .mockReset()
        .mockImplementation(async (id: string, patch: Partial<GameInstance>) => {
            expect(id).toBe('instance-a')
            Object.assign(backend.persisted, patch)
        })
    return openSettings()
}

async function openSettings() {
    backend.instance = ref(structuredClone(backend.persisted))
    const wrapper = mount(
        defineComponent({
            setup: () => () => h(Suspense, {}, { default: () => h(WindowSettings) }),
        }),
    )
    mounted.push(wrapper)
    await flushPromises()
    return wrapper
}

it.each([
    { force_fullscreen: false, maximize_window: false, game_resolution: null },
    {
        force_fullscreen: true,
        maximize_window: null,
        game_resolution: [1024, 768] as [number, number],
    },
    { force_fullscreen: null, maximize_window: true, game_resolution: null },
])(
    'keeps explicit boolean overrides and inherited fields when editing the title: %j',
    async (overrides) => {
        const wrapper = await settings(overrides)
        expect(
            wrapper.get<HTMLInputElement>('input[aria-label="Custom window settings"]').element
                .checked,
        ).toBe(true)
        await wrapper.get('#window-title').setValue('  Custom title  ')
        await flushPromises()
        expect(backend.persisted).toMatchObject({ ...overrides, window_title: 'Custom title' })
        expect(backend.edit).toHaveBeenLastCalledWith('instance-a', {
            window_title: 'Custom title',
        })
        const reopened = await openSettings()
        expect(reopened.get<HTMLInputElement>('#window-title').element.value).toBe('Custom title')
        expect(
            reopened.get<HTMLInputElement>('input[aria-label="Custom window settings"]').element
                .checked,
        ).toBe(true)
    },
)

it('updates resolution without changing boolean overrides or title', async () => {
    const wrapper = await settings({
        force_fullscreen: false,
        maximize_window: null,
        window_title: 'Existing title',
    })
    await wrapper.get('#width').setValue('1600')
    await flushPromises()
    expect(backend.persisted).toMatchObject({
        force_fullscreen: false,
        maximize_window: null,
        game_resolution: [1600, 720],
        window_title: 'Existing title',
    })
    expect(backend.edit).toHaveBeenLastCalledWith('instance-a', { game_resolution: [1600, 720] })
})

it('saves inheritance and explicit false as distinct states after reopening', async () => {
    const wrapper = await settings({ force_fullscreen: false, maximize_window: false })
    await wrapper.get('input[aria-label="Custom window settings"]').setValue(false)
    await flushPromises()
    expect(backend.persisted).toMatchObject({
        force_fullscreen: null,
        maximize_window: null,
        game_resolution: null,
    })
    const reopened = await openSettings()
    expect(
        reopened.get<HTMLInputElement>('input[aria-label="Custom window settings"]').element
            .checked,
    ).toBe(false)
    await reopened.get('input[aria-label="Custom window settings"]').setValue(true)
    await reopened.get('#fullscreen').setValue(false)
    await reopened.get('#maximize-window').setValue(false)
    await flushPromises()
    expect(backend.persisted).toMatchObject({ force_fullscreen: false, maximize_window: false })
    const final = await openSettings()
    expect(final.get<HTMLInputElement>('#fullscreen').element.checked).toBe(false)
    expect(final.get<HTMLInputElement>('#maximize-window').element.checked).toBe(false)
})

it('serializes rapid edits so the last title remains persisted', async () => {
    const wrapper = await settings({ force_fullscreen: false })
    let release!: () => void
    const save = backend.edit.getMockImplementation()!
    backend.edit.mockImplementationOnce(async (id, patch) => {
        await new Promise<void>((resolve) => (release = resolve))
        await save(id, patch)
    })
    await wrapper.get('#window-title').setValue('First')
    await wrapper.get('#window-title').setValue('Last')
    await flushPromises()
    expect(backend.edit).toHaveBeenCalledTimes(1)
    release()
    await flushPromises()
    expect(backend.persisted).toMatchObject({ window_title: 'Last', force_fullscreen: false })
})
