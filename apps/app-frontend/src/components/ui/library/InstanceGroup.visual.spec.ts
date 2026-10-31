import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import type { GameInstance } from '@/helpers/types'

import InstanceGroup from './InstanceGroup.vue'
const dnd = vi.hoisted(() => ({
    options: undefined as { disabled?: { value: boolean } } | undefined,
}))
vi.mock('@/composables/useInstanceGroups', () => ({ FAVORITES_GROUP_ID: 'favorites' }))
vi.mock('@/composables/useGridGrouping', () => ({ UNGROUPED_GROUP_KEY: 'ungrouped' }))
vi.mock('@dnd-kit/vue', async () => {
    const { ref } = await import('vue')
    return {
        useDroppable: (options: { disabled?: { value: boolean } }) => {
            dnd.options = options
            return { isDropTarget: ref(false) }
        },
    }
})
vi.mock('@/components/ui/library/DraggableInstanceCard.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', { 'data-draggable-card': '' }, slots.default?.({ isDragging: false })),
        }),
    }
})
vi.mock('@/components/ui/Instance.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: ['playing', 'instance', 'showGameDirectory'],
            setup: (props) => () =>
                h('div', {
                    'data-playing': String(props.playing),
                    'data-id': props.instance.id,
                    'data-show-game-directory': String(props.showGameDirectory),
                }),
        }),
    }
})
vi.mock('@/components/ui/ContextMenu.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    const control = defineComponent({
        setup:
            (_, { slots }) =>
            () =>
                h('div', slots.default?.()),
    })
    return {
        Button: control,
        Checkbox: control,
        IconButton: control,
        InlineEditableText: defineComponent({
            setup: () => () => h('input', { 'data-inline-editable': '' }),
        }),
        NewModal: control,
        TagItem: control,
        commonMessages: {},
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message?.defaultMessage ?? '',
        }),
    }
})
it.each(['Group', 'None'])(
    'passes explicit running state to every card in %s layout',
    (grouping) => {
        const wrapper = mount(InstanceGroup, {
            props: {
                sectionKey: 'a',
                groupName: 'A',
                grouping,
                instances: [
                    { id: 'running', name: 'Running' },
                    { id: 'idle', name: 'Idle' },
                ] as GameInstance[],
                runningInstanceIds: new Set(['running']),
                isCollapsed: false,
                displayMode: 'cards',
                selectMode: false,
                selectedInstanceIds: new Set(),
            },
            global: { directives: { tooltip: () => {} } },
        })
        try {
            expect(wrapper.get('[data-id="running"]').attributes('data-playing')).toBe('true')
            expect(wrapper.get('[data-id="idle"]').attributes('data-playing')).toBe('false')
        } finally {
            wrapper.unmount()
        }
    },
)

it('keeps game-directory sections automatic and non-draggable', () => {
    const wrapper = mount(InstanceGroup, {
        props: {
            sectionKey: 'external:d:/minecraft/.minecraft',
            groupName: String.raw`D:\Minecraft\.minecraft`,
            grouping: 'Game directory',
            instances: [{ id: 'one', name: 'One' }] as GameInstance[],
            runningInstanceIds: new Set(),
            isCollapsed: false,
            displayMode: 'cards',
            selectMode: false,
            selectedInstanceIds: new Set(),
            showGameDirectory: true,
        },
        global: { directives: { tooltip: () => {} } },
    })
    try {
        expect(wrapper.find('[data-inline-editable]').exists()).toBe(false)
        expect(wrapper.find('[data-testid="instance-group-actions"]').exists()).toBe(false)
        expect(wrapper.get('[data-testid="instance-group-name"]').text()).toBe(
            String.raw`D:\Minecraft\.minecraft`,
        )
        expect(wrapper.find('[data-draggable-card]').exists()).toBe(false)
        expect(wrapper.get('[data-id="one"]').attributes('data-show-game-directory')).toBe('true')
        expect(dnd.options?.disabled?.value).toBe(true)
    } finally {
        wrapper.unmount()
    }
})
