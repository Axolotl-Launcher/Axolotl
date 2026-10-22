import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref, Suspense } from 'vue'

import { parseEnvVars, serializeEnvVars } from '@/helpers/settings'
import type { GameInstance } from '@/helpers/types'

import EnvironmentVariablesInput from './EnvironmentVariablesInput.vue'
import InstanceJavaSettings from './instance_settings/JavaSettings.vue'
import DefaultInstanceSettings from './settings/DefaultInstanceSettings.vue'

const backend = vi.hoisted(() => ({
    global: {} as Record<string, unknown>,
    instance: {} as Partial<GameInstance>,
    instanceRef: null as unknown,
}))
vi.mock('@/store/theme.ts', () => ({ DEFAULT_FEATURE_FLAGS: {} }))
vi.mock('@/helpers/settings', async (importOriginal) => ({
    ...(await importOriginal<typeof import('@/helpers/settings')>()),
    get: async () => structuredClone(backend.global),
    update: async (patch: Record<string, unknown>) => {
        Object.assign(backend.global, patch)
        return structuredClone(backend.global)
    },
}))
vi.mock('@/helpers/instance', () => ({
    edit: async (id: string, patch: Partial<GameInstance>) => {
        expect(id).toBe('instance-a')
        Object.assign(backend.instance, JSON.parse(JSON.stringify(patch)))
    },
    get_content_snapshot: async () => ({ items: [] }),
    get_optimal_jre_key: async () => ({ path: 'java', parsed_version: '21' }),
}))
vi.mock('@/providers/instance-settings', () => ({
    injectInstanceSettings: () => ({ instance: backend.instanceRef }),
}))
vi.mock('@tauri-apps/plugin-os', () => ({ platform: () => 'windows' }))
vi.mock('@/composables/useMemorySlider', () => ({
    default: () => ({ maxMemory: 8192, snapPoints: [] }),
}))
vi.mock('@/components/ui/JavaArgumentsInput.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/JavaSelector.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/MemoryAllocationDisplay.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui/settings/LogShareSettings.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@/components/ui/settings/SharedLogsSettings.vue', () => ({
    default: { render: () => null },
}))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const card = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
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
        Card: card,
        Checkbox: toggle,
        Toggle: toggle,
        Slider: { render: () => null },
        StyledInput: defineComponent({
            props: ['modelValue', 'type', 'inputAttrs'],
            setup:
                (props, { emit }) =>
                () =>
                    h('input', {
                        ...props.inputAttrs,
                        value: props.modelValue,
                        type: props.type ?? 'text',
                        onInput: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLInputElement).value),
                    }),
        }),
        defineMessage: (message: unknown) => message,
        defineMessages: (messages: unknown) => messages,
        injectNotificationManager: () => ({ handleError: vi.fn() }),
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

const mounted: ReturnType<typeof mount>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))

it.each([
    [
        'TOKEN=a=b EMPTY=',
        [
            ['TOKEN', 'a=b'],
            ['EMPTY', ''],
        ],
    ],
    [
        ' TOKEN==  EMPTY=\tOTHER=x\n',
        [
            ['TOKEN', '='],
            ['EMPTY', ''],
            ['OTHER', 'x'],
        ],
    ],
    ['', []],
    [' \t\n ', []],
])('preserves valid environment content: %s', (input, expected) => {
    expect(parseEnvVars(input as string)).toEqual(expected)
    expect(parseEnvVars(serializeEnvVars(expected as [string, string][]))).toEqual(expected)
})

it.each(['BROKEN', '=value', 'TOKEN=x BROKEN', 'KEY\0=value', 'KEY=val\0ue'])(
    'rejects malformed entries without partially accepting them: %s',
    (input) => {
        expect(() => parseEnvVars(input)).toThrow()
    },
)

it('keeps the input draft unchanged while typing whitespace between variables', async () => {
    const value = ref<[string, string][]>([])
    const wrapper = mount(
        defineComponent({
            setup: () => () =>
                h(EnvironmentVariablesInput, {
                    modelValue: value.value,
                    'onUpdate:modelValue': (next: [string, string][]) => (value.value = next),
                }),
        }),
    )
    mounted.push(wrapper)
    await wrapper.get('input').setValue('TOKEN=a=b ')
    expect(wrapper.get<HTMLInputElement>('input').element.value).toBe('TOKEN=a=b ')
    await wrapper.get('input').setValue('TOKEN=a=b EMPTY=')
    expect(value.value).toEqual([
        ['TOKEN', 'a=b'],
        ['EMPTY', ''],
    ])
})

function initialize() {
    backend.global = {
        custom_env_vars: [['OLD', 'value']],
        extra_launch_args: [],
        memory: { maximum: 2048, automatic: true },
        hooks: { pre_launch: null, wrapper: null, post_exit: null },
        game_resolution: [1280, 720],
        force_fullscreen: false,
        maximize_window: false,
    }
    backend.instance = {
        id: 'instance-a',
        loader: 'fabric',
        custom_env_vars: [['OLD', 'value']],
        memory: null,
        extra_launch_args: [],
        java_path: 'custom/java.exe',
    }
}

async function openSettings(scope: 'instance' | 'defaults') {
    backend.instanceRef = ref(structuredClone(backend.instance))
    const component = scope === 'instance' ? InstanceJavaSettings : DefaultInstanceSettings
    const wrapper = mount(
        defineComponent({ setup: () => () => h(Suspense, {}, { default: () => h(component) }) }),
    )
    mounted.push(wrapper)
    await flushPromises()
    return wrapper
}

it.each(['instance', 'defaults'] as const)(
    'saves and reloads equals signs and empty values in %s settings',
    async (scope) => {
        initialize()
        const wrapper = await openSettings(scope)
        await wrapper.get('#env-vars').setValue('TOKEN=a=b EMPTY=')
        await flushPromises()
        const persisted = scope === 'instance' ? backend.instance : backend.global
        expect(persisted.custom_env_vars).toEqual([
            ['TOKEN', 'a=b'],
            ['EMPTY', ''],
        ])
        if (scope === 'instance') {
            expect(persisted).toMatchObject({
                extra_launch_args: [],
                java_path: 'custom/java.exe',
                memory: null,
            })
        }
        const reopened = await openSettings(scope)
        expect(reopened.get<HTMLInputElement>('#env-vars').element.value).toBe('TOKEN=a=b EMPTY=')
    },
)

it.each(['instance', 'defaults'] as const)(
    'keeps invalid content as a local draft while other %s settings save',
    async (scope) => {
        initialize()
        const wrapper = await openSettings(scope)
        await wrapper.get('#env-vars').setValue('TOKEN=a=b EMPTY=')
        await flushPromises()
        await wrapper.get('#env-vars').setValue('TOKEN=unsaved BROKEN')
        await flushPromises()
        expect(wrapper.find('[role="alert"]').exists()).toBe(true)
        if (scope === 'instance')
            await wrapper.get('input[aria-label="Custom memory allocation"]').setValue(true)
        else await wrapper.get('#fullscreen').setValue(true)
        await flushPromises()
        const persisted = scope === 'instance' ? backend.instance : backend.global
        expect(persisted.custom_env_vars).toEqual([
            ['TOKEN', 'a=b'],
            ['EMPTY', ''],
        ])
        expect(wrapper.get<HTMLInputElement>('#env-vars').element.value).toBe(
            'TOKEN=unsaved BROKEN',
        )
        await wrapper.get('#env-vars').setValue('CORRECTED=')
        await flushPromises()
        expect(wrapper.find('[role="alert"]').exists()).toBe(false)
        expect(persisted.custom_env_vars).toEqual([['CORRECTED', '']])
    },
)

it.each(['instance', 'defaults'] as const)(
    'persists clearing all variables in %s settings',
    async (scope) => {
        initialize()
        const wrapper = await openSettings(scope)
        await wrapper.get('#env-vars').setValue('')
        await flushPromises()
        const persisted = scope === 'instance' ? backend.instance : backend.global
        expect(persisted.custom_env_vars).toEqual([])
        const reopened = await openSettings(scope)
        expect(reopened.get<HTMLInputElement>('#env-vars').element.value).toBe('')
        if (scope === 'instance')
            expect(
                reopened.get<HTMLInputElement>('input[aria-label="Custom environment variables"]')
                    .element.checked,
            ).toBe(true)
    },
)
