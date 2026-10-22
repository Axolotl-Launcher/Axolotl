import { afterEach, expect, it } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { mountThemed } from '@/test/visual-harness'

import { createScreenshotSelection, isSelectionModifier } from './screenshot-selection'
import { useScreenshotMarquee } from './use-screenshot-marquee'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).forEach((fn) => fn()))

async function workspace() {
    const selection = createScreenshotSelection()
    const disabled = ref(false)
    let previews = 0
    let moves = 0
    const Harness = defineComponent({
        setup() {
            const container = ref<HTMLElement>()
            const marquee = useScreenshotMarquee({
                container,
                disabled: () => disabled.value,
                snapshot: () => selection.selectedKeys.value,
                targets: () => [
                    { key: 'a', left: 10, top: 10, width: 60, height: 40 },
                    { key: 'b', left: 90, top: 10, width: 60, height: 40 },
                ],
                apply: selection.box,
            })
            return () =>
                h(
                    'div',
                    {
                        ref: container,
                        style: 'position:fixed;left:0;top:0;width:220px;height:120px',
                        onPointerdownCapture: marquee.start,
                        onClickCapture: marquee.click,
                    },
                    [
                        h(
                            'article',
                            {
                                'data-screenshot-card': '',
                                role: 'button',
                                onClick: () => previews++,
                                onPointerdown: (event: PointerEvent) => {
                                    if (!isSelectionModifier(event)) moves++
                                },
                                style: 'position:absolute;left:10px;top:10px;width:60px;height:40px',
                            },
                            [h('button', {}, 'Action')],
                        ),
                        h('output', {}, [...selection.selectedKeys.value].join(',')),
                        marquee.rectangle.value ? h('aside', { 'data-marquee': '' }) : null,
                    ],
                )
        },
    })
    const wrapper = await mountThemed(Harness, {}, 'dark')
    cleanup.push(() => wrapper.unmount())
    await new Promise((resolve) => requestAnimationFrame(resolve))
    return { wrapper, selection, disabled, counts: () => ({ previews, moves }) }
}

function pointer(
    target: EventTarget,
    type: string,
    x: number,
    y: number,
    modifiers: Record<string, boolean> = {},
) {
    target.dispatchEvent(
        new PointerEvent(type, {
            bubbles: true,
            cancelable: true,
            pointerId: 1,
            pointerType: 'mouse',
            button: 0,
            clientX: x,
            clientY: y,
            ...modifiers,
        }),
    )
}

it('selects a rectangle and suppresses its click without activating file movement', async () => {
    const { wrapper, selection, counts } = await workspace()
    pointer(wrapper.element, 'pointerdown', 0, 0)
    pointer(document, 'pointermove', 155, 55)
    await wrapper.vm.$nextTick()
    expect(wrapper.find('[data-marquee]').exists()).toBe(true)
    expect([...selection.selectedKeys.value]).toEqual(['a', 'b'])
    pointer(document, 'pointerup', 155, 55)
    wrapper.get('article').element.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    expect(counts()).toEqual({ previews: 0, moves: 0 })
    await wrapper.vm.$nextTick()
    expect(wrapper.find('[data-marquee]').exists()).toBe(false)
})

it('keeps normal card movement and embedded controls separate from modified-card selection', async () => {
    const { wrapper, selection, counts } = await workspace()
    const card = wrapper.get('article').element
    pointer(card, 'pointerdown', 20, 20)
    pointer(document, 'pointermove', 150, 55)
    pointer(document, 'pointerup', 150, 55)
    expect(selection.selectedKeys.value.size).toBe(0)
    expect(counts().moves).toBe(1)
    pointer(card, 'pointerdown', 20, 20, { shiftKey: true })
    pointer(document, 'pointermove', 155, 55, { shiftKey: true })
    expect([...selection.selectedKeys.value]).toEqual(['a', 'b'])
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    expect(selection.selectedKeys.value.size).toBe(0)
    pointer(document, 'pointerup', 155, 55)
    card.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    expect(counts().previews).toBe(0)
    pointer(wrapper.get('button').element, 'pointerdown', 20, 20, { shiftKey: true })
    pointer(document, 'pointermove', 155, 55)
    expect(selection.selectedKeys.value.size).toBe(0)
})

it('restores cancelled selection, respects busy state, and releases listeners on unmount', async () => {
    const { wrapper, selection, disabled } = await workspace()
    selection.box(['outside'], new Set())
    pointer(wrapper.element, 'pointerdown', 0, 0, { ctrlKey: true })
    pointer(document, 'pointermove', 155, 55)
    expect(selection.selectedKeys.value.size).toBe(3)
    pointer(document, 'pointercancel', 155, 55)
    expect([...selection.selectedKeys.value]).toEqual(['outside'])
    disabled.value = true
    pointer(wrapper.element, 'pointerdown', 0, 0)
    pointer(document, 'pointermove', 155, 55)
    expect([...selection.selectedKeys.value]).toEqual(['outside'])
    disabled.value = false
    pointer(wrapper.element, 'pointerdown', 0, 0)
    wrapper.unmount()
    pointer(document, 'pointermove', 155, 55)
    expect([...selection.selectedKeys.value]).toEqual(['outside'])
})

it('does not resurrect hidden selections when cancellation repeats before pointer release', async () => {
    const { wrapper, selection } = await workspace()
    selection.box(['outside'], new Set())
    pointer(wrapper.element, 'pointerdown', 0, 0)
    pointer(document, 'pointermove', 155, 55)
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    selection.reconcile(['a', 'b'])
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    window.dispatchEvent(new Event('blur'))
    pointer(document, 'pointerup', 155, 55)
    expect(selection.selectedKeys.value.size).toBe(0)
})
