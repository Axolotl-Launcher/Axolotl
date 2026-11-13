import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import { renderTranslatedDescription } from '@/helpers/translation'

import TranslatedProjectDescription from './TranslatedProjectDescription.vue'

const render = vi.hoisted(() => ({
    tasks: [] as Array<() => void>,
    markdown: vi.fn(
        (text: string) =>
            `<p>${text}</p><img src="https://example.invalid/image.png" width="100" height="50">`,
    ),
}))
vi.mock('@/helpers/page-transition', () => ({
    runAfterPageTransitionSettle: (task: () => void) => render.tasks.push(task),
}))
vi.mock('@modrinth/utils', () => ({
    configuredXss: { process: (text: string) => text },
    renderHighlightedString: render.markdown,
}))
vi.mock('@/helpers/translation', () => ({
    prepareDescription: vi.fn(),
    renderTranslatedDescription: vi.fn(),
}))
vi.mock('@modrinth/ui', async () => {
    const { h } = await import('vue')
    return { ProjectPageSkeleton: { render: () => h('div', { 'data-loading': 'description' }) } }
})

it('defers parsing, skips stale bodies and keeps image dimensions while loading images asynchronously', async () => {
    render.tasks = []
    render.markdown.mockClear()
    const wrapper = mount(TranslatedProjectDescription, {
        props: {
            description: 'First',
            active: false,
            translations: {},
            mode: 'bilingual',
            style: 'weakened',
        },
    })
    try {
        expect(render.markdown).not.toHaveBeenCalled()
        expect(wrapper.find('[data-loading="description"]').exists()).toBe(true)
        await wrapper.setProps({ description: 'Current' })
        render.tasks[0]()
        expect(render.markdown).not.toHaveBeenCalled()
        render.tasks[1]()
        await flushPromises()
        expect(wrapper.text()).toBe('Current')
        expect(wrapper.get('img').attributes()).toMatchObject({
            loading: 'lazy',
            decoding: 'async',
            width: '100',
            height: '50',
        })
        await wrapper.setProps({ description: 'Unmounted' })
        wrapper.unmount()
        render.tasks[2]()
        expect(render.markdown).toHaveBeenCalledTimes(1)
    } finally {
        wrapper.unmount()
    }
})

it('retains existing paragraph nodes and animations when another translation arrives', async () => {
    render.tasks = []
    vi.mocked(renderTranslatedDescription).mockImplementation((_prepared, translations) =>
        ['first', 'second']
            .map(
                (id) =>
                    `<p>${id}</p>${translations[id] ? `<div class="ax-translation-block">${translations[id]}</div>` : ''}`,
            )
            .join(''),
    )
    const wrapper = mount(TranslatedProjectDescription, {
        attachTo: document.body,
        props: {
            description: 'Body',
            active: true,
            translations: { first: 'First translation' },
            mode: 'bilingual',
            style: 'weakened',
        },
    })
    try {
        render.tasks.shift()!()
        await flushPromises()
        const first = wrapper.get('.ax-translation-block').element
        const original = wrapper.get('p').element
        const animation = first.getAnimations()[0]
        expect(animation).toBeDefined()
        await wrapper.setProps({
            translations: { first: 'First translation', second: 'Second translation' },
        })
        render.tasks.shift()!()
        await flushPromises()
        const translated = wrapper.findAll('.ax-translation-block')
        expect(translated).toHaveLength(2)
        expect(translated[0].element).toBe(first)
        expect(wrapper.get('p').element).toBe(original)
        expect(first.getAnimations()[0]).toBe(animation)
        expect(translated[1].element.getAnimations()[0]).toBeDefined()
        await wrapper.setProps({ active: false })
        render.tasks.shift()!()
        await flushPromises()
        expect(wrapper.find('.ax-translation-block').exists()).toBe(false)
    } finally {
        wrapper.unmount()
    }
})
