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
        // The click ripple is part of every icon but invisible at rest.
        const ripple = svg?.querySelector('.nav-icon-ripple')
        expect(ripple && getComputedStyle(ripple).opacity).toBe('0')

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

function isRipple(animation: Animation): boolean {
    return !!targetOf(animation)?.classList.contains('nav-icon-ripple')
}

/** The smallest uniform scale a root clip reaches, from its keyframes. */
function smallestScale(animation: Animation): number {
    if (!(animation.effect instanceof KeyframeEffect)) return 1
    return Math.min(
        ...animation.effect
            .getKeyframes()
            .map((keyframe) => new DOMMatrixReadOnly(String(keyframe.transform)).a),
    )
}

it('squashes the icon on press and answers every click with motion and a ripple', async () => {
    const button = await mountNavButton(DiscoverNavIcon)
    const svg = button.querySelector('svg')

    press(button)
    await nextTick()
    expect(navAnimations(button, 'press').map(targetOf)).toEqual([svg])

    click(button)
    await nextTick()
    const partClicks = () => navAnimations(button, 'activate').filter((a) => !isRipple(a))
    const ripples = () => navAnimations(button, 'activate').filter(isRipple)
    const firstClick = partClicks()
    expect(firstClick.length).toBeGreaterThan(0)
    expect(firstClick.every((animation) => targetOf(animation) !== svg)).toBe(true)
    expect(firstClick.some(leavesRest)).toBe(true)
    const [firstRipple] = ripples()
    expect(ripples()).toHaveLength(1)

    // A second click while the first motion runs adds its own on top and
    // sends a fresh ripple...
    click(button)
    await nextTick()
    expect(partClicks()).toHaveLength(2 * firstClick.length)
    expect(ripples()).toHaveLength(1)
    expect(ripples()[0]).not.toBe(firstRipple)
    // ...but rapid clicking does not pile motion up without bound.
    click(button)
    await nextTick()
    expect(partClicks()).toHaveLength(2 * firstClick.length)
})

it.each(ICONS)(
    'clicking the %s icon is heavier than hovering and moves differently',
    async (_name, icon) => {
        const button = await mountNavButton(icon)
        const svg = button.querySelector('svg')

        pointerEnter(button)
        await nextTick()
        const entry = navAnimations(button, 'enter')
        const pop = entry.find((animation) => targetOf(animation) === svg)
        const hoverMotion = new Map(
            entry
                .filter((animation) => targetOf(animation) !== svg)
                .map((animation) => [
                    targetOf(animation),
                    JSON.stringify((animation.effect as KeyframeEffect).getKeyframes()),
                ]),
        )

        press(button)
        click(button)
        await nextTick()
        const squash = navAnimations(button, 'press').find(
            (animation) => targetOf(animation) === svg,
        )
        const clickMotion = navAnimations(button, 'activate').filter((a) => !isRipple(a))

        // Pressing crushes the icon harder than hovering ever shrinks it.
        expect(pop && squash).toBeTruthy()
        expect(smallestScale(squash!)).toBeLessThan(smallestScale(pop!) - 0.1)
        // The click has its own motion: never the hover motion of the same part again.
        expect(clickMotion.some(leavesRest)).toBe(true)
        for (const animation of clickMotion) {
            const keyframes = JSON.stringify((animation.effect as KeyframeEffect).getKeyframes())
            expect(keyframes).not.toBe(hoverMotion.get(targetOf(animation)))
        }
    },
)

/** The turn (degrees) and sideways shift (px) that one keyframe's transform adds. */
function sidewaysPose(transform: unknown): { angle: number; shift: number } {
    const text = typeof transform === 'string' ? transform : ''
    const sum = (pattern: RegExp) =>
        [...text.matchAll(pattern)].reduce((total, [, value]) => total + Number(value), 0)
    return {
        angle: sum(/rotate\((-?[\d.]+)deg\)/g),
        shift: sum(/translateX\((-?[\d.]+)(?:px)?\)/g) + sum(/translate\((-?[\d.]+)(?:px)?[,)]/g),
    }
}

/** How often a run of values changes direction. */
function reversals(values: number[]): number {
    let count = 0
    let direction = 0
    for (let index = 1; index < values.length; index++) {
        const step = Math.sign(values[index] - values[index - 1])
        if (step === 0) continue
        if (direction !== 0 && step !== direction) count++
        direction = step
    }
    return count
}

/** Which way a run of values first moves: 1, -1, or 0 if it never moves. */
function heading(values: number[]): number {
    const moved = values.find((value) => value !== values[0])
    return moved === undefined ? 0 : Math.sign(moved - values[0])
}

it.each(ICONS)('the %s icon never sways from side to side', async (_name, icon) => {
    const button = await mountNavButton(icon)
    pointerEnter(button)
    press(button)
    click(button)
    await nextTick()
    const animations = navAnimations(button)
    expect(animations.length).toBeGreaterThan(0)

    const turns = new Map<Element | null, Set<number>>()
    const slides = new Map<Element | null, Set<number>>()
    for (const animation of animations) {
        const poses = (animation.effect as KeyframeEffect)
            .getKeyframes()
            .map((keyframe) => sidewaysPose(keyframe.transform))
        const angles = poses.map((pose) => pose.angle)
        const shifts = poses.map((pose) => pose.shift)
        const label = `${animation.id} on ${targetOf(animation)?.getAttribute('class')}`
        // A loop only ever turns or slides one way; a single motion may go out
        // and come back once, but never on past the rest pose to the other side.
        const allowed = loopsForever(animation) ? 0 : 1
        expect(reversals(angles), label).toBeLessThanOrEqual(allowed)
        expect(reversals(shifts), label).toBeLessThanOrEqual(allowed)
        for (const [ways, values] of [
            [turns, angles],
            [slides, shifts],
        ] as const) {
            const way = heading(values)
            if (!way) continue
            const target = targetOf(animation)
            ways.set(target, (ways.get(target) ?? new Set<number>()).add(way))
        }
    }
    // Every motion of a part goes the same way, so hovering and clicking
    // cannot add up to a swing either.
    for (const ways of [...turns.values(), ...slides.values()]) expect(ways.size).toBe(1)
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
