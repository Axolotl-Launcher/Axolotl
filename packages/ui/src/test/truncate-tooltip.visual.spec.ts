import { afterEach, expect, it } from 'vitest'
import { defineComponent, h, nextTick, ref, withDirectives } from 'vue'

import { tooltipDirective } from '../directives/tooltip'
import { isElementTruncated } from '../utils/truncate'
import { mountThemed, waitFor } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn()),
)

async function textLabel(nested = false) {
    const text = ref('A long filename here')
    const fragment = ref<HTMLElement | null>(null)
    const Harness = defineComponent({
        setup: () => () =>
            withDirectives(
                h(
                    'button',
                    {
                        style: 'position:fixed;top:150px;left:20px;width:100px;display:flex;',
                    },
                    nested
                        ? h(
                              'span',
                              {
                                  ref: fragment,
                                  style: 'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0',
                              },
                              text.value,
                          )
                        : text.value,
                ),
                [
                    [
                        tooltipDirective,
                        {
                            content: text.value,
                            onlyWhenTruncated: true,
                            overflowTarget: nested ? fragment.value : undefined,
                        },
                    ],
                ],
            ),
    })
    const wrapper = await mountThemed(Harness, {}, 'dark')
    cleanup.push(() => wrapper.unmount())
    const trigger = wrapper.get('button').element as HTMLButtonElement
    if (!nested)
        trigger.style.cssText +=
            'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;display:block'
    await nextTick()
    trigger.focus()
    await waitFor(() => !!document.querySelector('[role="tooltip"]'))
    return { wrapper, trigger, text, fragment }
}

for (const nested of [false, true]) {
    it(`updates an active tooltip when ${nested ? 'the nested text fragment' : 'the label'} is resized or changed`, async () => {
        const { trigger, text } = await textLabel(nested)
        expect(document.querySelector('[role="tooltip"]')?.textContent).toContain(text.value)
        trigger.style.width = 'calc(100vw - 40px)'
        await waitFor(() => !document.querySelector('[role="tooltip"]'))
        expect(trigger.hasAttribute('aria-describedby')).toBe(false)
        trigger.style.width = '80px'
        await waitFor(() => !!document.querySelector('[role="tooltip"]'))
        text.value = 'Short'
        await waitFor(() => !document.querySelector('[role="tooltip"]'))
        text.value = 'Changed long filename that should show its complete content'
        await waitFor(
            () =>
                document
                    .querySelector('[role="tooltip"]')
                    ?.textContent?.includes('Changed long filename') ?? false,
        )
    })
}

it('remeasures after font changes and releases its observers on unmount', async () => {
    const { trigger, wrapper } = await textLabel()
    trigger.style.width = '220px'
    trigger.style.fontSize = '2px'
    await waitFor(() => !document.querySelector('[role="tooltip"]'))
    trigger.style.fontSize = '28px'
    document.fonts.dispatchEvent(new Event('loadingdone'))
    await waitFor(() => !!document.querySelector('[role="tooltip"]'))
    wrapper.unmount()
    document.fonts.dispatchEvent(new Event('loadingdone'))
    trigger.style.width = '10px'
    await new Promise((resolve) => setTimeout(resolve, 250))
    expect(document.querySelector('[role="tooltip"]')).toBeNull()
})

it('remeasures inherited font styles without waiting for a font download', async () => {
    const { trigger } = await textLabel()
    trigger.style.cssText += ';height:40px;width:200px;font-size:inherit'
    const parent = trigger.parentElement!
    parent.style.fontSize = '2px'
    await waitFor(() => !document.querySelector('[role="tooltip"]'))
    parent.style.fontSize = '32px'
    await waitFor(() => !!document.querySelector('[role="tooltip"]'))
    parent.style.fontSize = '2px'
    await waitFor(() => !document.querySelector('[role="tooltip"]'))
})

it('detects real horizontal overflow of one pixel', () => {
    const label = document.createElement('div')
    label.style.cssText = 'position:fixed;width:100px;overflow:hidden;white-space:nowrap'
    const content = document.createElement('span')
    content.style.cssText = 'display:inline-block;width:100px;height:20px'
    label.append(content)
    document.body.append(label)
    cleanup.push(() => label.remove())
    expect(isElementTruncated(label)).toBe(false)
    content.style.width = '101px'
    expect(isElementTruncated(label)).toBe(true)
})
