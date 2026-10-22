import '@modrinth/ui/styles/overlays.css'

import {
    TooltipArrow,
    TooltipContent,
    TooltipPortal,
    TooltipProvider,
    TooltipRoot,
    TooltipTrigger,
} from 'reka-ui'
import { afterEach, expect, it } from 'vitest'
import { defineComponent, h } from 'vue'

import HeadlessTooltip from '@/components/ui/headless/HeadlessTooltip.vue'

import { mountThemed, waitFor } from './visual-harness'

const cleanup: (() => void)[] = []
afterEach(() =>
    cleanup
        .splice(0)
        .reverse()
        .forEach((fn) => fn()),
)

for (const headless of [false, true]) {
    for (const text of [
        'An English tooltip with several words',
        '这是正常宽度的中文提示，不应逐字竖排',
        ['PlayerOne', 'AnotherPlayer'],
    ]) {
        it(`measures ${Array.isArray(text) ? 'player names' : text} inside ${headless ? 'HeadlessTooltip' : 'Reka TooltipContent'}`, async () => {
            const Harness = defineComponent({
                setup: () => () => {
                    const content = () =>
                        Array.isArray(text)
                            ? h(
                                  'div',
                                  { class: 'flex flex-col gap-1' },
                                  text.map((name) => h('span', name)),
                              )
                            : text
                    const trigger = () =>
                        h(
                            'button',
                            { style: 'position:fixed;top:120px;left:160px;width:40px;height:32px' },
                            'Show',
                        )
                    return headless
                        ? h(
                              HeadlessTooltip,
                              {
                                  contentClass: 'tooltip-surface',
                                  delayMs: 0,
                                  arrowClass: 'tooltip-arrow',
                              },
                              { default: trigger, content },
                          )
                        : h(
                              TooltipProvider,
                              {},
                              {
                                  default: () =>
                                      h(
                                          TooltipRoot,
                                          { delayDuration: 0 },
                                          {
                                              default: () => [
                                                  h(
                                                      TooltipTrigger,
                                                      { asChild: true },
                                                      { default: trigger },
                                                  ),
                                                  h(
                                                      TooltipPortal,
                                                      {},
                                                      {
                                                          default: () =>
                                                              h(
                                                                  TooltipContent,
                                                                  {
                                                                      class: 'tooltip-surface',
                                                                      side: 'top',
                                                                      sideOffset: 6,
                                                                      collisionPadding: 8,
                                                                  },
                                                                  {
                                                                      default: () => [
                                                                          content(),
                                                                          h(TooltipArrow, {
                                                                              class: 'tooltip-arrow',
                                                                              width: 14,
                                                                              height: 7,
                                                                          }),
                                                                      ],
                                                                  },
                                                              ),
                                                      },
                                                  ),
                                              ],
                                          },
                                      ),
                              },
                          )
                },
            })
            const wrapper = await mountThemed(Harness, {}, 'dark')
            cleanup.push(() => wrapper.unmount())
            wrapper.get<HTMLButtonElement>('button').element.focus()
            await waitFor(() => !!document.querySelector('.tooltip-surface'))
            const surface = document.querySelector('.tooltip-surface') as HTMLElement
            const positioner = surface.closest('[data-reka-popper-content-wrapper]') as HTMLElement
            await waitFor(() => !!positioner && positioner.getBoundingClientRect().width > 100)
            const box = surface.getBoundingClientRect()
            expect(getComputedStyle(surface).position).not.toBe('fixed')
            expect(box.width).toBeGreaterThan(100)
            expect(box.height).toBeLessThan(90)
            expect(positioner.getBoundingClientRect().width).toBeCloseTo(box.width, 0)
            expect(positioner.getBoundingClientRect().height).toBeCloseTo(box.height, 0)
            expect(
                surface.querySelector('svg.tooltip-arrow')?.getBoundingClientRect().width,
            ).toBeGreaterThan(0)
            const triggerElement = wrapper.get('button').element as HTMLElement
            triggerElement.style.cssText =
                'position:fixed;left:calc(100vw - 40px);top:0;width:32px;height:32px'
            window.dispatchEvent(new Event('resize'))
            await waitFor(() => surface.dataset.side === 'bottom')
            await waitFor(() => surface.getBoundingClientRect().right <= innerWidth)
            expect(surface.getBoundingClientRect().left).toBeGreaterThanOrEqual(0)
            wrapper.unmount()
            expect(document.querySelector('.tooltip-surface')).toBeNull()
        })
    }
}

it('wraps a long unbroken Reka tooltip within the viewport while the wrapper retains its size', async () => {
    const wrapper = await mountThemed(
        HeadlessTooltip,
        { contentClass: 'tooltip-surface', delayMs: 0 },
        'dark',
        {
            slots: {
                default: () =>
                    h('button', { style: 'position:fixed;left:150px;top:150px' }, 'Show'),
                content: () => 'LongText'.repeat(80),
            },
        },
    )
    cleanup.push(() => wrapper.unmount())
    wrapper.get<HTMLButtonElement>('button').element.focus()
    await waitFor(() => !!document.querySelector('.tooltip-surface'))
    const surface = document.querySelector('.tooltip-surface') as HTMLElement
    await waitFor(() => surface.getBoundingClientRect().width > 100)
    expect(surface.getBoundingClientRect().width).toBeLessThanOrEqual(innerWidth - 32)
    expect(surface.scrollWidth).toBeLessThanOrEqual(surface.clientWidth)
    expect(surface.parentElement!.getBoundingClientRect().height).toBeCloseTo(
        surface.getBoundingClientRect().height,
        0,
    )
})
