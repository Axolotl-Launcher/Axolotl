import '@/test/visual-styles.scss'

import { flushPromises, mount } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { expect, it, vi } from 'vitest'
import { nextTick } from 'vue'

import type { HomeWidgetPlacement } from './home-dashboard'
import HomeWidgetPickerModal from './HomeWidgetPickerModal.vue'

const runtime = vi.hoisted(() => ({
    getInstanceWorlds: vi.fn(),
    hide: vi.fn(),
}))

vi.mock('@/components/home/home-dashboard-runtime', async () => {
    const { ref } = await import('vue')
    return {
        useHomeDashboardRuntime: () => ({
            favoriteWorlds: ref([]),
            getInstanceWorlds: runtime.getInstanceWorlds,
        }),
    }
})
// The previews render the real widgets; here a stub says what it would draw.
vi.mock('@/components/home/HomeWidgetPreview.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: ['kind', 'size', 'target'],
            setup: (props) => () =>
                h('div', {
                    class: 'preview-stub',
                    'data-kind': props.kind,
                    'data-size': props.size,
                    'data-target': props.target?.fallbackLabel ?? '',
                    style: 'width:100%;height:100%',
                }),
        }),
    }
})
vi.mock('@/components/ui/InstanceIcon.vue', () => ({ default: { render: () => null } }))
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        Button: defineComponent({
            props: ['disabled'],
            setup:
                (props, { slots, attrs }) =>
                () =>
                    h('button', { ...attrs, disabled: props.disabled }, slots.default?.()),
        }),
        NewModal: defineComponent({
            props: ['header'],
            setup: (props, { slots, expose }) => {
                expose({ show: () => {}, hide: runtime.hide })
                return () =>
                    h('div', { class: 'modal-stub' }, [h('h2', props.header), slots.default?.()])
            },
        }),
        StyledInput: defineComponent({
            props: ['modelValue', 'placeholder'],
            emits: ['update:modelValue'],
            setup:
                (props, { emit }) =>
                () =>
                    h('input', {
                        value: props.modelValue,
                        placeholder: props.placeholder,
                        onInput: (event: Event) =>
                            emit('update:modelValue', (event.target as HTMLInputElement).value),
                    }),
        }),
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            locale: { value: 'en-US' },
            formatMessage: (
                message: { defaultMessage: string },
                values?: Record<string, unknown>,
            ) => new IntlMessageFormat(message.defaultMessage, 'en-US').format(values) as string,
        }),
    }
})

const instances = [
    {
        id: 'fo',
        name: 'Fabulously Optimized',
        loader: 'fabric',
        game_version: '1.21',
        icon_path: null,
    },
    { id: 'cp', name: 'Create+', loader: 'forge', game_version: '1.19.2', icon_path: null },
]

async function mountPicker(widgets: HomeWidgetPlacement[] = []) {
    runtime.hide.mockReset()
    const wrapper = mount(HomeWidgetPickerModal, {
        attachTo: document.body,
        props: { instances: instances as never, widgets },
        global: { directives: { tooltip: () => {} } },
    })
    wrapper.element.setAttribute('style', 'width:1000px')
    ;(wrapper.vm as unknown as { show: () => void }).show()
    await flushPromises()
    return wrapper
}

const card = (wrapper: Awaited<ReturnType<typeof mountPicker>>, kind: string) =>
    wrapper.get(`.home-picker-card[data-kind="${kind}"]`)

it('shows every widget as a preview, grouped, with what is already added marked', async () => {
    const wrapper = await mountPicker([
        { id: 'a', kind: 'calendar', size: '1x2' },
        { id: 'b', kind: 'calendar', size: '1x2' },
        { id: 'c', kind: 'launch', size: '2x2' },
    ])
    try {
        expect(wrapper.findAll('.home-picker-card')).toHaveLength(10)
        expect(wrapper.findAll('.home-picker-section-title').map((title) => title.text())).toEqual([
            'Start playing',
            'Play data',
            'Collections',
            'Shortcuts',
        ])
        // Each card draws the widget at the size it would be added at.
        expect(card(wrapper, 'recent').get('.preview-stub').attributes('data-size')).toBe('2x2')
        expect(card(wrapper, 'calendar').get('.home-picker-card-added').text()).toBe('Added ×2')
        expect(card(wrapper, 'launch').get('.home-picker-card-added').text()).toBe('Added')
        expect(card(wrapper, 'greeting').find('.home-picker-card-added').exists()).toBe(false)
        // A kind already on the Home can still be added again.
        await card(wrapper, 'calendar').get('.home-picker-card-main').trigger('click')
        expect(wrapper.get('.home-picker-add').text()).toBe('Add another')
    } finally {
        wrapper.unmount()
    }
})

it('filters by category and by search', async () => {
    const wrapper = await mountPicker()
    try {
        await wrapper.findAll('.home-picker-category')[4].trigger('click')
        expect(
            wrapper.findAll('.home-picker-card').map((item) => item.attributes('data-kind')),
        ).toEqual(['instance', 'world', 'server'])
        await wrapper.findAll('.home-picker-category')[0].trigger('click')
        await wrapper.get('.home-picker-toolbar input').setValue('server')
        expect(
            wrapper.findAll('.home-picker-card').map((item) => item.attributes('data-kind')),
        ).toEqual(['pinned-servers', 'server'])
        await wrapper.get('.home-picker-toolbar input').setValue('zzz')
        expect(wrapper.find('.home-picker-none').text()).toBe('No widgets match')
    } finally {
        wrapper.unmount()
    }
})

it('adds in one click, stays open, says so and can undo', async () => {
    const wrapper = await mountPicker()
    try {
        await card(wrapper, 'pinned-worlds').get('.home-picker-card-add').trigger('click')
        await card(wrapper, 'calendar').get('.home-picker-card-add').trigger('click')
        const added = wrapper.emitted('add')!.map(([widget]) => widget as HomeWidgetPlacement)
        expect(added.map((widget) => [widget.kind, widget.size])).toEqual([
            ['pinned-worlds', '1x2'],
            ['calendar', '1x2'],
        ])
        expect(runtime.hide).not.toHaveBeenCalled()
        const feedback = wrapper.get('.home-picker-feedback')
        expect(feedback.attributes('role')).toBe('status')
        expect(feedback.text()).toContain('Added Calendar to your Home')
        expect(card(wrapper, 'calendar').classes()).toContain('is-flashing')
        await wrapper.get('.home-picker-undo').trigger('click')
        expect(wrapper.emitted('remove')).toEqual([[added[1].id]])
        expect(wrapper.get('.home-picker-feedback').text()).toBe('')
    } finally {
        wrapper.unmount()
    }
})

it('lets a size be chosen first, with a preview that follows it', async () => {
    const wrapper = await mountPicker()
    try {
        await card(wrapper, 'recent').get('.home-picker-card-main').trigger('click')
        const sizes = wrapper.get('.home-picker-sizes')
        expect(sizes.attributes('role')).toBe('radiogroup')
        expect(sizes.findAll('[role="radio"]').map((size) => size.attributes('data-size'))).toEqual(
            ['2x1', '2x2', '2x3', '3x1', '3x2'],
        )
        // Only the checked size is a tab stop; arrow keys move the choice.
        expect(sizes.findAll('[tabindex="0"]').map((size) => size.attributes('data-size'))).toEqual(
            ['2x2'],
        )
        await sizes.get('[data-size="2x2"]').trigger('keydown', { key: 'ArrowRight' })
        expect(wrapper.get('.home-picker-stage .preview-stub').attributes('data-size')).toBe('2x3')
        await sizes.get('[data-size="3x1"]').trigger('click')
        expect(wrapper.get('.home-picker-stage .preview-stub').attributes('data-size')).toBe('3x1')
        expect(card(wrapper, 'recent').get('.preview-stub').attributes('data-size')).toBe('3x1')
        await wrapper.get('.home-picker-add').trigger('click')
        expect((wrapper.emitted('add')![0][0] as HomeWidgetPlacement).size).toBe('3x1')
    } finally {
        wrapper.unmount()
    }
})

it('takes a shortcut through its target in the same panel', async () => {
    runtime.getInstanceWorlds.mockResolvedValue([
        { type: 'singleplayer', name: 'Island', path: 'saves/Island', game_mode: 'survival' },
        { type: 'server', name: 'Hub', address: 'hub.example', index: 0 },
    ])
    const wrapper = await mountPicker()
    try {
        // The plus asks for the target instead of adding nothing.
        await card(wrapper, 'world').get('.home-picker-card-add').trigger('click')
        await nextTick()
        expect(wrapper.emitted('add')).toBeUndefined()
        expect(wrapper.get('.home-picker-add').attributes('disabled')).toBeDefined()
        const options = () => wrapper.findAll('.home-picker-target-option')
        expect(options().map((option) => option.text())).toEqual([
            expect.stringContaining('Fabulously Optimized'),
            expect.stringContaining('Create+'),
        ])
        await options()[0].trigger('click')
        await flushPromises()
        expect(runtime.getInstanceWorlds).toHaveBeenCalledWith('fo')
        // Only worlds for a world shortcut.
        expect(options().map((option) => option.text())).toEqual([
            expect.stringContaining('Island'),
        ])
        await options()[0].trigger('click')
        expect(options()[0].attributes('aria-selected')).toBe('true')
        expect(wrapper.get('.home-picker-stage .preview-stub').attributes('data-target')).toBe(
            'Island',
        )
        await wrapper.get('.home-picker-add').trigger('click')
        expect(wrapper.emitted('add')![0][0]).toMatchObject({
            kind: 'world',
            target: { instanceId: 'fo', path: 'saves/Island', fallbackLabel: 'Island' },
        })
        expect(wrapper.get('.home-picker-feedback').text()).toContain('Added Island')
    } finally {
        wrapper.unmount()
    }
})

it('moves between cards with the arrow keys', async () => {
    const wrapper = await mountPicker()
    try {
        const mains = () => wrapper.findAll<HTMLElement>('.home-picker-card-main')
        mains()[0].element.focus()
        await mains()[0].trigger('keydown', { key: 'ArrowRight' })
        expect(document.activeElement).toBe(mains()[1].element)
        await mains()[1].trigger('keydown', { key: 'ArrowLeft' })
        expect(document.activeElement).toBe(mains()[0].element)
        await mains()[0].trigger('keydown', { key: 'ArrowDown' })
        const below = document.activeElement as HTMLElement
        expect(below.getBoundingClientRect().top).toBeGreaterThan(
            mains()[0].element.getBoundingClientRect().bottom - 1,
        )
    } finally {
        wrapper.unmount()
    }
})

it('replaces a target with only the panel, then closes', async () => {
    const wrapper = await mountPicker()
    try {
        ;(wrapper.vm as unknown as { show: (kind: string, replace: boolean) => void }).show(
            'instance',
            true,
        )
        await flushPromises()
        expect(wrapper.find('.home-picker-catalog').exists()).toBe(false)
        expect(wrapper.find('.home-picker-sizes').exists()).toBe(false)
        expect(wrapper.get('h2').text()).toBe('Replace target')
        await wrapper.findAll('.home-picker-target-option')[1].trigger('click')
        await wrapper.get('.home-picker-add').trigger('click')
        expect(wrapper.emitted('add')![0][0]).toMatchObject({
            kind: 'instance',
            target: { instanceId: 'cp' },
        })
        expect(runtime.hide).toHaveBeenCalled()
    } finally {
        wrapper.unmount()
    }
})
