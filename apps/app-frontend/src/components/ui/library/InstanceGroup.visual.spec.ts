import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import type { GameInstance } from '@/helpers/types'

import InstanceGroup from './InstanceGroup.vue'
vi.mock('@/composables/useInstanceGroups', () => ({ FAVORITES_GROUP_ID: 'favorites' }))
vi.mock('@/composables/useGridGrouping', () => ({ UNGROUPED_GROUP_KEY: 'ungrouped' }))
vi.mock('@dnd-kit/vue', async () => {
    const { ref } = await import('vue')
    return { useDroppable: () => ({ isDropTarget: ref(false) }) }
})
vi.mock('@/components/ui/library/DraggableInstanceCard.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            setup:
                (_, { slots }) =>
                () =>
                    h('div', slots.default?.({ isDragging: false })),
        }),
    }
})
vi.mock('@/components/ui/Instance.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: ['playing', 'instance'],
            setup: (props) => () =>
                h('div', { 'data-playing': String(props.playing), 'data-id': props.instance.id }),
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
        InlineEditableText: control,
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
