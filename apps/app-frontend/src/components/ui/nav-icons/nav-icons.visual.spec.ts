import { afterEach, expect, it, vi } from 'vitest'
import { type Component, h, nextTick } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import { mountThemed, waitFor } from '@/test/visual-harness'

import NavButton from '../NavButton.vue'
import {
    CreateNavIcon,
    DiscoverNavIcon,
    HomeNavIcon,
    LabNavIcon,
    LibraryNavIcon,
    MultiplayerNavIcon,
    NAV_ICON_ANIMATION_PREFIX,
    ScreenshotsNavIcon,
    SettingsNavIcon,
    SkinsNavIcon,
    WorldsNavIcon,
} from '.'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).forEach((fn) => fn()))

const ICONS: [string, Component][] = [
    ['home', HomeNavIcon],
    ['screenshots', ScreenshotsNavIcon],
    ['worlds', WorldsNavIcon],
    ['discover', DiscoverNavIcon],
    ['skins', SkinsNavIcon],
    ['multiplayer', MultiplayerNavIcon],
    ['library', LibraryNavIcon],
    ['lab', LabNavIcon],
    ['create', CreateNavIcon],
    ['settings', SettingsNavIcon],
]

type Kind = 'enter' | 'idle' | 'press' | 'activate'

function navAnimations(element: Element, kind?: Kind): Animation[] {
    return element
        .getAnimations({ subtree: true })
        .filter((animation) =>
            kind
                ? animation.id === `${NAV_ICON_ANIMATION_PREFIX}${kind}`
                : animation.id.startsWith(NAV_ICON_ANIMATION_PREFIX),
        )
}

function targetOf(animation: Animation): Element | null {
    return animation.effect instanceof KeyframeEffect ? animation.effect.target : null
}

function loopsForever(animation: Animation): boolean {
    return animation.effect?.getTiming().iterations === Infinity
}

/**
 * True if the clip itself takes its part through a pose other than the resting
 * one. Read from the clip's own keyframes, so a loop running on the same part
 * cannot make a motionless clip look as if it moved.
 */
function leavesRest(animation: Animation): boolean {
    if (!(animation.effect instanceof KeyframeEffect)) return false
    return animation.effect.getKeyframes().some((keyframe) => {
        const transform = keyframe.transform
        return (
            typeof transform === 'string' &&
            transform !== 'none' &&
            !new DOMMatrixReadOnly(transform).isIdentity
        )
    })
}

async function mountNavButton(
    icon: Component,
    props: { to?: string | (() => void); disabled?: boolean } = {},
) {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: ['/', '/target'].map((path) => ({ path, component: { render: () => null } })),
    })
    await router.push('/')
    const wrapper = await mountThemed(NavButton, { to: '/target', ...props }, 'dark', {
        global: { plugins: [router] },
        slots: { default: () => h(icon) },
    })
    cleanup.push(() => wrapper.unmount())
    return wrapper.element as HTMLElement
}

function pointerEnter(element: Element) {
    element.dispatchEvent(new PointerEvent('pointerenter', { pointerType: 'mouse' }))
}

function pointerLeave(element: Element) {
    element.dispatchEvent(new PointerEvent('pointerleave', { pointerType: 'mouse' }))
}

function press(element: Element) {
    element.dispatchEvent(
        new PointerEvent('pointerdown', {
            bubbles: true,
            button: 0,
            pointerType: 'mouse',
            isPrimary: true,
        }),
    )
}

function click(element: Element) {
    element.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 }))
}

it.each(ICONS)(
    'the %s icon pops on entry, keeps moving while hovered and settles after leaving',
    async (_name, icon) => {
        const button = await mountNavButton(icon)
        const svg = button.querySelector('svg')
        expect(navAnimations(button)).toHaveLength(0)

        pointerEnter(button)
        await nextTick()
        const entry = navAnimations(button, 'enter')
        // The shared pop on the whole icon, plus the icon's own entry motion.
        expect(
            entry.some((animation) => targetOf(animation) === svg && leavesRest(animation)),
        ).toBe(true)
        expect(
            entry.some((animation) => targetOf(animation) !== svg && leavesRest(animation)),
        ).toBe(true)
        const loops = navAnimations(button, 'idle')
        expect(loops.length).toBeGreaterThan(0)
        expect(loops.every(loopsForever)).toBe(true)
        expect(loops.some(leavesRest)).toBe(true)

        await waitFor(() => navAnimations(button, 'enter').length === 0, {
            timeoutMs: 3000,
            label: 'the entry motion to finish',
        })
        const stillLooping = navAnimations(button, 'idle')
        expect(stillLooping).toHaveLength(loops.length)
        stillLooping.forEach((loop, index) => expect(loop).toBe(loops[index]))

        pointerLeave(button)
        await waitFor(() => navAnimations(button).length === 0, {
            timeoutMs: 4000,
            label: 'the loops to wind down',
        })
    },
)

it('picks a winding-down loop back up when the pointer returns', async () => {
    const button = await mountNavButton(SettingsNavIcon)
    pointerEnter(button)
    await nextTick()
    const loops = navAnimations(button, 'idle')
    await waitFor(
        () => loops.every((loop) => loop.effect?.getComputedTiming().currentIteration != null),
        { timeoutMs: 3000, label: 'the loops to start' },
    )

    pointerLeave(button)
    await nextTick()
    expect(loops.some(loopsForever)).toBe(false)

    pointerEnter(button)
    await nextTick()
    const resumed = navAnimations(button, 'idle')
    expect(resumed).toHaveLength(loops.length)
    resumed.forEach((loop, index) => expect(loop).toBe(loops[index]))
    expect(resumed.every(loopsForever)).toBe(true)
    // ...and back at its normal pace, not the faster wind-down one.
    await Promise.all(resumed.map((loop) => loop.ready))
    expect(resumed.map((loop) => loop.playbackRate)).toEqual(resumed.map(() => 1))
})

it('squashes the icon on press and answers every click with motion', async () => {
    const button = await mountNavButton(DiscoverNavIcon)
    const svg = button.querySelector('svg')

    press(button)
    await nextTick()
    expect(navAnimations(button, 'press').map(targetOf)).toEqual([svg])

    click(button)
    await nextTick()
    const clickMotion = navAnimations(button, 'activate')
    expect(clickMotion.length).toBeGreaterThan(0)
    expect(clickMotion.every((animation) => targetOf(animation) !== svg)).toBe(true)
    expect(clickMotion.some(leavesRest)).toBe(true)

    // A second click while the first motion runs adds its own on top...
    click(button)
    await nextTick()
    expect(navAnimations(button, 'activate')).toHaveLength(2 * clickMotion.length)
    // ...but rapid clicking does not pile motion up without bound.
    click(button)
    await nextTick()
    expect(navAnimations(button, 'activate')).toHaveLength(2 * clickMotion.length)
})

it('brings an unfinished entry to rest quickly when the pointer leaves at once', async () => {
    const button = await mountNavButton(DiscoverNavIcon)
    pointerEnter(button)
    await nextTick()
    pointerLeave(button)
    await nextTick()
    const entry = navAnimations(button, 'enter')
    expect(entry.length).toBeGreaterThan(0)
    await Promise.all(entry.map((animation) => animation.ready))
    for (const animation of entry) {
        const timing = animation.effect!.getComputedTiming()
        const remainingMs =
            (Number(timing.endTime) - Number(timing.localTime)) / animation.playbackRate
        expect(remainingMs).toBeLessThanOrEqual(520)
    }
})

it.each([
    ['link', '/target'],
    ['action', () => {}],
] as const)('keeps a disabled %s destination still', async (_kind, to) => {
    const button = await mountNavButton(SettingsNavIcon, { to, disabled: true })
    pointerEnter(button)
    press(button)
    click(button)
    await nextTick()
    await nextTick()
    expect(navAnimations(button)).toHaveLength(0)
})

it('stays still when reduced motion is requested', async () => {
    const matchMedia = window.matchMedia.bind(window)
    const spy = vi
        .spyOn(window, 'matchMedia')
        .mockImplementation((query: string) =>
            query.includes('prefers-reduced-motion')
                ? ({ matches: true, media: query } as MediaQueryList)
                : matchMedia(query),
        )
    cleanup.push(() => spy.mockRestore())

    const button = await mountNavButton(HomeNavIcon)
    pointerEnter(button)
    press(button)
    click(button)
    await nextTick()
    expect(navAnimations(button)).toHaveLength(0)
})

it('stops at once when reduced motion is switched on mid-hover', async () => {
    const listeners: ((event: MediaQueryListEvent) => void)[] = []
    const reduceQuery = {
        matches: false,
        media: '(prefers-reduced-motion: reduce)',
        addEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => {
            listeners.push(listener)
        },
        removeEventListener: () => {},
    }
    const matchMedia = window.matchMedia.bind(window)
    const spy = vi
        .spyOn(window, 'matchMedia')
        .mockImplementation((query: string) =>
            query.includes('prefers-reduced-motion')
                ? (reduceQuery as unknown as MediaQueryList)
                : matchMedia(query),
        )
    cleanup.push(() => spy.mockRestore())

    const button = await mountNavButton(SettingsNavIcon)
    pointerEnter(button)
    await nextTick()
    expect(navAnimations(button, 'idle').length).toBeGreaterThan(0)

    reduceQuery.matches = true
    listeners.forEach((listener) => listener({ matches: true } as MediaQueryListEvent))
    expect(navAnimations(button)).toHaveLength(0)
})
