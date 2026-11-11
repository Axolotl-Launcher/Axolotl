import '@/assets/stylesheets/window-frame.css'

import { expect, it } from 'vitest'

import { applyTheme } from './visual-harness'

it('draws one token-based contour only when CSS owns a restored window frame', () => {
    applyTheme('dark')
    const frame = document.createElement('div')
    frame.className = 'app-grid-layout has-transparent-background'
    document.body.append(frame)
    try {
        expect(getComputedStyle(frame, '::after').content).toBe('none')
        frame.classList.add('has-css-window-border')
        const style = getComputedStyle(frame, '::after')
        expect(style.content).toBe('""')
        expect(style.pointerEvents).toBe('none')
        expect(style.boxShadow.split(',').filter((part) => part.includes('inset'))).toHaveLength(1)
        expect(style.boxShadow).toMatch(/0px 0px 0px 1px/)
        for (const state of ['is-maximized', 'has-native-decorations']) {
            frame.classList.add(state)
            expect(getComputedStyle(frame, '::after').content).toBe('none')
            frame.classList.remove(state)
            expect(getComputedStyle(frame, '::after').content).toBe('""')
        }
        frame.classList.remove('has-css-window-border')
        expect(getComputedStyle(frame, '::after').content).toBe('none')
    } finally {
        frame.remove()
    }
})

it('keeps filling the content corner notch while the window frame owns ::after', () => {
    applyTheme('dark')
    const frame = document.createElement('div')
    frame.className = 'app-grid-layout has-transparent-background has-css-window-border'
    // The app publishes both tokens through `global.scss`, which this harness
    // does not load; the notch's colour mix needs them to resolve.
    frame.style.setProperty('--surface-3-opaque', '#262b34')
    frame.style.setProperty('--opacity-ratio-chrome', '72%')
    document.body.append(frame)
    try {
        const notch = getComputedStyle(frame, '::before')
        expect(notch.content).toBe('""')
        expect(notch.position).toBe('absolute')
        expect(notch.boxShadow).not.toBe('none')

        const contour = getComputedStyle(frame, '::after')
        expect(contour.content).toBe('""')
        expect(contour.position).toBe('fixed')
        expect(contour.boxShadow).toMatch(/inset/)
    } finally {
        frame.remove()
    }
})
