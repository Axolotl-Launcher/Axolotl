import '@/test/visual-styles.scss'

import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import type { GameDirectoryInfo } from '@/helpers/game-directory'
import { waitFor } from '@/test/visual-harness'

import HomeGameDirectoryBadge from './HomeGameDirectoryBadge.vue'
import HomeItemRow from './HomeItemRow.vue'

vi.mock('@/components/ui/InstanceIcon.vue', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        default: defineComponent({
            props: ['size'],
            setup: (props) => () =>
                h('img', { class: 'icon-stub', style: `width:${props.size};height:${props.size}` }),
        }),
    }
})
vi.mock('@modrinth/ui', async () => {
    const { defineComponent, h } = await import('vue')
    return {
        defineMessages: (messages: unknown) => messages,
        useVIntl: () => ({
            formatMessage: (message: { defaultMessage: string }) => message.defaultMessage,
        }),
        Avatar: defineComponent({
            props: ['size'],
            setup: (props) => () =>
                h('img', { class: 'avatar', style: `width:${props.size};height:${props.size}` }),
        }),
    }
})

function mountRow(props: Record<string, unknown>, directory?: GameDirectoryInfo) {
    const host = document.createElement('div')
    host.style.cssText = 'width:180px'
    document.body.append(host)
    const wrapper = mount(HomeItemRow, {
        attachTo: host,
        props: {
            title: 'Survival Island',
            meta: 'Survival',
            instance: { id: 'fo' },
            playLabel: 'Play',
            stopLabel: 'Stop',
            ...props,
        },
        slots: directory ? { 'meta-prefix': () => h(HomeGameDirectoryBadge, { directory }) } : {},
        global: {
            directives: { tooltip: () => {} },
            stubs: {
                RouterLink: defineComponent({
                    setup:
                        (_, { slots }) =>
                        () =>
                            h('a', slots.default?.()),
                }),
            },
        },
    })
    return {
        wrapper,
        done: () => {
            wrapper.unmount()
            host.remove()
        },
    }
}

it('draws a tile with the icon as its cover and the play button hidden until pointed at', async () => {
    const { wrapper, done } = mountRow({ variant: 'tile' })
    try {
        const row = wrapper.get('.home-item-row')
        expect(row.classes()).toContain('is-tile')
        expect(getComputedStyle(row.element).flexDirection).toBe('column')
        expect(wrapper.get('.icon-stub').element.getBoundingClientRect().width).toBe(52)
        const actions = wrapper.get('.home-item-row-actions').element
        expect(getComputedStyle(actions).position).toBe('absolute')
        expect(getComputedStyle(actions).opacity).toBe('0')
        // Keyboard focus shows it like a pointer does.
        ;(wrapper.get('.home-item-row-play').element as HTMLElement).focus()
        await new Promise((resolve) => setTimeout(resolve, 250))
        expect(getComputedStyle(actions).opacity).toBe('1')
    } finally {
        done()
    }
})

it('keeps a running tile’s stop button showing', () => {
    const { wrapper, done } = mountRow({ variant: 'tile', running: true })
    try {
        expect(wrapper.find('.home-item-row-play.is-stop').exists()).toBe(true)
        expect(getComputedStyle(wrapper.get('.home-item-row-actions').element).opacity).toBe('1')
    } finally {
        done()
    }
})

it('keeps rows and cards on one line, and greys out a dimmed icon', () => {
    const row = mountRow({ variant: 'row' })
    const card = mountRow({ variant: 'card', dimmed: true })
    try {
        expect(getComputedStyle(row.wrapper.get('.home-item-row').element).flexDirection).toBe(
            'row',
        )
        expect(row.wrapper.get('.icon-stub').element.getBoundingClientRect().width).toBe(32)
        expect(card.wrapper.get('.icon-stub').element.getBoundingClientRect().width).toBe(32)
        expect(getComputedStyle(card.wrapper.get('.icon-stub').element).filter).toContain(
            'grayscale',
        )
    } finally {
        row.done()
        card.done()
    }
})

it('hides the tile actions until pointed at only for a fine pointer that can hover', () => {
    const { done } = mountRow({ variant: 'tile' })
    try {
        // Every rule that hides the actions sits under a hover-capable, fine-pointer query,
        // so a touch screen (hover: none, pointer: coarse) always shows them.
        const hiding: string[] = []
        const walk = (rules: CSSRuleList, media: string | null) => {
            for (const rule of Array.from(rules)) {
                if (rule instanceof CSSMediaRule) walk(rule.cssRules, rule.conditionText)
                else if (
                    rule instanceof CSSStyleRule &&
                    rule.selectorText.includes('.home-item-row-actions') &&
                    rule.style.opacity === '0'
                )
                    hiding.push(media ?? 'none')
            }
        }
        for (const sheet of Array.from(document.styleSheets)) walk(sheet.cssRules, null)
        expect(hiding.length).toBeGreaterThan(0)
        for (const media of hiding) {
            expect(media).toContain('hover: hover')
            expect(media).toContain('pointer: fine')
        }
    } finally {
        done()
    }
})

const externalDirectory: GameDirectoryInfo = {
    key: 'external:long-path',
    kind: 'external',
    shortLabel: 'Roaming/.minecraft',
    path: String.raw`C:\Users\Player\AppData\Roaming\Minecraft installations\Survival\.minecraft`,
}

it.each(['row', 'card', 'tile'] as const)(
    'reveals the directory over the metadata in a %s without moving the title or actions',
    async (variant) => {
        const { wrapper, done } = mountRow({ variant, to: '/instance/test' }, externalDirectory)
        try {
            const badge = wrapper.getComponent(HomeGameDirectoryBadge)
            const metadata = wrapper.get('.home-item-row-meta').element
            const reveal = badge.get('.home-directory-reveal').element
            const title = wrapper.get('.home-item-row-title').element.getBoundingClientRect()
            const actions = wrapper.get('.home-item-row-actions').element.getBoundingClientRect()
            const badgeBounds = badge.element.getBoundingClientRect()
            const versionBounds = wrapper
                .get('.home-item-row-meta > .home-scrolling-text')
                .element.getBoundingClientRect()
            expect(badgeBounds.width).toBeLessThanOrEqual(
                metadata.getBoundingClientRect().width * 0.45 + 1,
            )
            expect(versionBounds.width).toBeGreaterThan(
                metadata.getBoundingClientRect().width / 2 - 4,
            )
            expect(
                badge.element.contains(
                    document.elementFromPoint(badgeBounds.x + 2, badgeBounds.y + 2),
                ),
            ).toBe(true)

            await badge.trigger('mouseenter')
            await waitFor(() => {
                const opacity = Number(getComputedStyle(reveal).opacity)
                return opacity > 0 && opacity < 1
            })
            const expanding = reveal.getBoundingClientRect()
            expect(expanding.x).toBeCloseTo(metadata.getBoundingClientRect().x, 0)
            expect(expanding.width).toBeLessThan(metadata.getBoundingClientRect().width)
            await waitFor(() => getComputedStyle(reveal).opacity === '1')
            await waitFor(
                () =>
                    Math.abs(
                        reveal.getBoundingClientRect().width -
                            metadata.getBoundingClientRect().width,
                    ) < 0.1,
            )
            const bounds = reveal.getBoundingClientRect()
            expect(bounds.width).toBeCloseTo(metadata.getBoundingClientRect().width, 0)
            expect(bounds.x).toBeCloseTo(metadata.getBoundingClientRect().x, 0)
            expect(reveal.textContent).toContain(externalDirectory.path)
            await waitFor(() =>
                reveal.contains(
                    document.elementFromPoint(bounds.right - 2, bounds.y + bounds.height / 2),
                ),
            )
            expect(wrapper.get('.home-item-row-title').element.getBoundingClientRect()).toEqual(
                title,
            )
            expect(wrapper.get('.home-item-row-actions').element.getBoundingClientRect()).toEqual(
                actions,
            )

            const text = reveal.querySelector('.w-max') as HTMLElement
            await waitFor(() => text.getAnimations().length === 1)
            const animation = text.getAnimations()[0]
            animation.pause()
            const timing = animation.effect!.getTiming()
            const duration = Number(timing.duration)
            const frames = (animation.effect as KeyframeEffect).getKeyframes()
            const travel = frames[1].computedOffset! * duration
            const pauseEnd = frames[2].computedOffset! * duration
            expect(pauseEnd - travel).toBeCloseTo(3000, 2)
            const translateX = async (time: number) => {
                animation.currentTime = Number(timing.delay) + time
                await new Promise(requestAnimationFrame)
                return new DOMMatrixReadOnly(getComputedStyle(text).transform).m41
            }
            const end = await translateX(travel)
            expect(end).toBeLessThan(0)
            const endingText = document.createRange()
            endingText.selectNodeContents(text.children[0])
            expect(
                text.parentElement!.getBoundingClientRect().right -
                    endingText.getBoundingClientRect().right,
            ).toBeGreaterThanOrEqual(7.5)
            expect(Math.abs(await translateX(travel / 4))).toBeLessThan(Math.abs(end) / 4)
            expect(await translateX(travel + 2990)).toBeCloseTo(end, 2)
            expect(await translateX(pauseEnd + (duration - pauseEnd) / 2)).toBeLessThan(end)
            const nextCopy = text.children[1] as HTMLElement
            const firstCopy = text.children[0] as HTMLElement
            const cycle = nextCopy.offsetLeft - firstCopy.offsetLeft
            expect(await translateX(duration - 1)).toBeCloseTo(-cycle, 1)
            expect(await translateX(duration + 1)).toBeCloseTo(0, 1)

            await badge.trigger('mouseleave')
            expect(text.getAnimations()).toHaveLength(0)
            await waitFor(() => getComputedStyle(reveal).opacity === '0')
            expect(badge.get('[data-game-directory-badge]').text()).toContain(
                externalDirectory.shortLabel,
            )
        } finally {
            done()
        }
    },
)

it('keeps fitting paths still and cancels a long path animation when unmounted', async () => {
    const short = mountRow({}, { ...externalDirectory, path: 'D:\\MC', shortLabel: 'MC' })
    const long = mountRow({}, externalDirectory)
    try {
        await short.wrapper.getComponent(HomeGameDirectoryBadge).trigger('mouseenter')
        await long.wrapper.getComponent(HomeGameDirectoryBadge).trigger('mouseenter')
        const shortText = short.wrapper.get('.home-directory-reveal .w-max').element
        const longText = long.wrapper.get('.home-directory-reveal .w-max').element
        await waitFor(() => longText.getAnimations().length === 1)
        expect(shortText.getAnimations()).toHaveLength(0)
        const animation = longText.getAnimations()[0]
        long.wrapper.unmount()
        expect(animation.playState).toBe('idle')
    } finally {
        short.done()
        long.done()
    }
})

it('expands without automatic scrolling when reduced motion is requested', async () => {
    const matchMedia = window.matchMedia.bind(window)
    const spy = vi.spyOn(window, 'matchMedia').mockImplementation((query: string) => {
        const media = matchMedia(query)
        if (query.includes('prefers-reduced-motion')) {
            Object.defineProperty(media, 'matches', { value: true })
        }
        return media
    })
    const { wrapper, done } = mountRow({}, externalDirectory)
    try {
        await wrapper.getComponent(HomeGameDirectoryBadge).trigger('mouseenter')
        const reveal = wrapper.get('.home-directory-reveal')
        expect(reveal.classes()).toContain('is-expanded')
        expect(reveal.get('.w-max').element.getAnimations()).toHaveLength(0)
    } finally {
        done()
        spy.mockRestore()
    }
})

it('pauses metadata at the readable beginning before scrolling left with faded edges', async () => {
    const { wrapper, done } = mountRow(
        { meta: 'Fabric 26.1.2 · Played yesterday · A very long instance description' },
        externalDirectory,
    )
    try {
        const metadata = wrapper.get('.home-item-row-meta > .home-scrolling-text').element
        const text = metadata.querySelector('.w-max') as HTMLElement
        await waitFor(() => text.getAnimations().length === 1)
        expect(getComputedStyle(metadata).maskImage).toContain('linear-gradient')
        expect(wrapper.get('.home-directory-reveal').classes()).not.toContain('is-expanded')

        const animation = text.getAnimations()[0]
        animation.pause()
        const duration = Number(animation.effect!.getTiming().duration)
        const frames = (animation.effect as KeyframeEffect).getKeyframes()
        const pauseEnd = frames[1].computedOffset! * duration
        expect(pauseEnd).toBeCloseTo(3000, 2)
        const position = async (time: number) => {
            animation.currentTime = time
            await new Promise(requestAnimationFrame)
            return new DOMMatrixReadOnly(getComputedStyle(text).transform).m41
        }
        expect(await position(0)).toBeCloseTo(0, 2)
        const beginning = document.createRange()
        beginning.selectNodeContents(text.children[0])
        expect(
            beginning.getBoundingClientRect().left - metadata.getBoundingClientRect().left,
        ).toBeGreaterThanOrEqual(7.5)
        expect(await position(2990)).toBeCloseTo(0, 2)
        expect(await position(pauseEnd + (duration - pauseEnd) / 2)).toBeLessThan(0)
        expect(await position(duration + 1000)).toBeCloseTo(0, 2)
    } finally {
        done()
    }
})

it('keeps fitting metadata still without faded edges or duplicate text', async () => {
    const { wrapper, done } = mountRow({ meta: 'Fabric' })
    try {
        const metadata = wrapper.get('.home-item-row-meta > .home-scrolling-text').element
        await new Promise(requestAnimationFrame)
        await new Promise(requestAnimationFrame)
        expect(metadata.textContent).toBe('Fabric')
        expect(metadata.querySelector('.w-max')!.getAnimations()).toHaveLength(0)
        expect(getComputedStyle(metadata).maskImage).toBe('none')
    } finally {
        done()
    }
})
