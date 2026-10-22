import assert from 'node:assert/strict'
import { test } from 'node:test'

import { trackHeroGlow } from './hero-glow.ts'

function createFixture() {
    const viewport = new EventTarget()
    const properties = new Map<string, string>()
    const bounds = { left: 100, top: 20, width: 800, height: 400 }
    const hero = {
        getBoundingClientRect: () => bounds,
        style: { setProperty: (name: string, value: string) => properties.set(name, value) },
    } as unknown as HTMLElement
    const stop = trackHeroGlow(hero, viewport as unknown as Window)
    const move = (clientX: number, clientY: number) => {
        const event = new Event('pointermove', { cancelable: true })
        Object.assign(event, { clientX, clientY })
        viewport.dispatchEvent(event)
        return event
    }
    const position = () => [properties.get('--pointer-x'), properties.get('--pointer-y')]

    return { viewport, properties, bounds, stop, move, position }
}

test('glow follows viewport movement across the hero/header boundary without resetting', () => {
    const { move, position, stop } = createFixture()
    try {
        move(300, 220)
        assert.deepEqual(position(), ['25%', '50%'])

        // These viewport events also arrive when the pointer is over the sibling header.
        move(700, 40)
        assert.deepEqual(position(), ['75%', '5%'])
        move(740, 60)
        assert.deepEqual(position(), ['80%', '10%'])

        move(500, 180)
        assert.deepEqual(position(), ['50%', '40%'])
    } finally {
        stop()
    }
})

test('coordinates use current hero bounds and stay clamped after scrolling', () => {
    const { bounds, move, position, stop } = createFixture()
    try {
        move(-100, -100)
        assert.deepEqual(position(), ['0%', '0%'])
        move(1000, 1000)
        assert.deepEqual(position(), ['100%', '100%'])

        bounds.top = -180
        move(300, 20)
        assert.deepEqual(position(), ['25%', '50%'])
    } finally {
        stop()
    }
})

test('zero-sized heroes do not receive invalid percentage coordinates', () => {
    const { bounds, properties, move, stop } = createFixture()
    try {
        bounds.width = 0
        move(300, 220)
        assert.equal(properties.size, 0)
        bounds.width = 800
        bounds.height = 0
        move(300, 220)
        assert.equal(properties.size, 0)
    } finally {
        stop()
    }
})

test('leaving the viewport retains the last position and cleanup stops tracking', () => {
    const { viewport, move, position, stop } = createFixture()
    move(300, 220)
    viewport.dispatchEvent(new Event('pointerleave'))
    assert.deepEqual(position(), ['25%', '50%'])
    stop()
    stop()
    move(700, 40)
    assert.deepEqual(position(), ['25%', '50%'])
})

test('tracking does not cancel pointer events or navigation clicks', () => {
    const { viewport, move, stop } = createFixture()
    try {
        assert.equal(move(700, 40).defaultPrevented, false)
        const click = new Event('click', { cancelable: true })
        let clicked = false
        viewport.addEventListener('click', () => {
            clicked = true
        })
        assert.equal(viewport.dispatchEvent(click), true)
        assert.equal(clicked, true)
        assert.equal(click.defaultPrevented, false)
    } finally {
        stop()
    }
})
