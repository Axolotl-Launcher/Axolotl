import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import type { GameInstance } from '@/helpers/types'

import InstancePickerList from './InstancePickerList.vue'

vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        EmptyState: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', slots.default?.()),
        }),
        StyledInput: defineComponent({
            props: ['modelValue'],
            setup: (props, { emit, expose }) => {
                expose({ focus: vi.fn() })
                return () =>
                    h('input', {
                        value: props.modelValue,
                        onInput: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLInputElement).value),
                    })
            },
        }),
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            locale: { value: 'en-US' },
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
    }
})

function instance(id: string, gameDirOverride?: string): GameInstance {
    return {
        id,
        name: id,
        loader: 'fabric',
        game_version: '1.21',
        game_dir_override: gameDirOverride,
    } as GameInstance
}

function mountPicker(instances: GameInstance[]) {
    return mount(InstancePickerList, {
        props: {
            instances,
            searchPlaceholder: 'Search',
            noInstancesMessage: 'No instances',
            noMatchesMessage: 'No matches',
            selectLabel: (instance: GameInstance) => instance.name,
        },
    })
}

it('shows directory badges from the full picker collection while filtering results', async () => {
    const picker = mountPicker([
        instance('managed'),
        instance('external', String.raw`D:\Minecraft\.minecraft`),
    ])
    const singleDirectory = mountPicker([instance('managed')])

    try {
        expect(picker.findAll('[data-game-directory-badge]')).toHaveLength(2)
        await picker.get('input').setValue('external')
        expect(picker.findAll('[data-game-directory-badge]')).toHaveLength(1)
        expect(singleDirectory.find('[data-game-directory-badge]').exists()).toBe(false)
    } finally {
        picker.unmount()
        singleDirectory.unmount()
    }
})
