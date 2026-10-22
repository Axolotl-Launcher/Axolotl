import { I18N_INJECTION_KEY } from '@modrinth/ui'
import { afterEach, expect, it } from 'vitest'
import { page, userEvent } from 'vitest/browser'
import { h, ref } from 'vue'

import { mountThemed, waitFor } from '@/test/visual-harness'

import StorageDisclosure from './StorageDisclosure.vue'
import StorageTreeNode from './StorageTreeNode.vue'

const mounted: Awaited<ReturnType<typeof mountThemed>>[] = []
afterEach(() => mounted.splice(0).forEach((wrapper) => wrapper.unmount()))
const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))

it('bridges expansion and collapse without jumping or trapping rapid toggles', async () => {
    const wrapper = await mountThemed(StorageDisclosure, {}, 'dark', {
        slots: {
            summary: () => h('div', { style: { height: '40px' } }, 'Game versions'),
            default: () => h('div', { style: { height: '120px' } }, 'Minecraft 1.21.1'),
        },
    })
    mounted.push(wrapper)
    const details = wrapper.get('details').element as HTMLDetailsElement
    const height = () => details.getBoundingClientRect().height
    const closed = height()
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            element: wrapper.element,
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/expand-start.png`,
        })
    if (
        CSS.supports('interpolate-size', 'allow-keywords') &&
        !matchMedia('(prefers-reduced-motion: reduce)').matches
    ) {
        const contentStyle = getComputedStyle(details, '::details-content')
        expect(contentStyle.blockSize).toBe('0px')
        const properties = contentStyle.transitionProperty.split(',').map((value) => value.trim())
        const durations = contentStyle.transitionDuration.split(',').map(parseFloat)
        const sizeIndex = properties.indexOf('block-size')
        expect(sizeIndex).toBeGreaterThanOrEqual(0)
        expect(durations[sizeIndex % durations.length]).toBeGreaterThan(0)
    }
    await wrapper.get('summary').trigger('click')
    expect(details.open).toBe(true)
    await waitFor(() => height() > closed, { label: 'disclosure to start expanding' })
    expect(height()).toBeLessThanOrEqual(closed + 120)
    await waitFor(() => height() >= closed + 119)
    if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
        await page.screenshot({
            element: wrapper.element,
            path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/expand-end.png`,
        })
    await wrapper.get('summary').trigger('click')
    await frame()
    await wrapper.get('summary').trigger('click')
    await frame()
    expect(details.open).toBe(true)
    await waitFor(() => height() >= closed + 119)
    await wrapper.get('summary').trigger('click')
    await waitFor(() => height() <= closed + 1)
    expect(details.open).toBe(false)
})

it('category rows keep their full size while the pointer is held down', async () => {
    const wrapper = await mountThemed(
        StorageTreeNode,
        {
            node: {
                id: 'category',
                type: 'database',
                size: { actual: 10, symlink: 0 },
                paths: [],
                children: [
                    {
                        id: 'release',
                        type: 'database',
                        name: 'release',
                        size: { actual: 10, symlink: 0 },
                        paths: [],
                    },
                ],
            },
            depth: 0,
            parentTotal: 10,
        },
        'dark',
        {
            global: {
                directives: { tooltip: {} },
                provide: {
                    [I18N_INJECTION_KEY as symbol]: {
                        locale: ref('en-US'),
                        t: (key: string) => key,
                        setLocale: () => {},
                    },
                },
            },
        },
    )
    mounted.push(wrapper)
    const row = wrapper.get('.tree-row').element as HTMLElement
    const width = row.getBoundingClientRect().width
    let pressedWidth = 0
    let scale = ''
    let active = false
    row.addEventListener(
        'mousedown',
        () =>
            requestAnimationFrame(() => {
                pressedWidth = row.getBoundingClientRect().width
                scale = getComputedStyle(row).scale
                active = row.matches(':active')
            }),
        { once: true },
    )
    await userEvent.click(row, { delay: 120 })
    expect(active).toBe(true)
    expect(pressedWidth).toBeCloseTo(width, 1)
    expect(scale).toBe('none')
})
