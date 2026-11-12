/** Every animation a nav icon starts has an id with this prefix. */
export const NAV_ICON_ANIMATION_PREFIX = 'nav-icon-'

/**
 * One animation of one part. Every clip starts and ends on the resting pose,
 * which is what lets clips start, overlap and stop at any moment without a
 * visible jump.
 */
export interface NavIconClip {
    keyframes: Keyframe[]
    duration: number
    delay?: number
    /**
     * Clips are added on top of whatever else runs on the part (the default),
     * so an entry, a loop, a click and a quick repeat of any of them can play
     * at once. Use `replace` for properties that cannot be summed, such as
     * stroke dashes, and only in `enter`; such a clip never overlaps itself.
     */
    composite?: CompositeOperation
    /**
     * Set for clips that cannot be layered over a copy of themselves, such as a
     * bubble that grows from nothing: a second copy starting at zero size would
     * make the first vanish. Such a clip never overlaps itself, and its part
     * should run no other clip.
     */
    single?: boolean
}

export interface NavIconPartMotion {
    /** Played once each time the pointer arrives. */
    enter?: NavIconClip
    /** Looped while the pointer stays; when it leaves, the current cycle finishes faster. */
    idle?: NavIconClip
    /** Played once on each click or keyboard activation. */
    activate?: NavIconClip
}

/** Part motions keyed by a CSS selector inside the icon. */
export type NavIconMotion = Record<string, NavIconPartMotion>

/** `[offset, value, easing of the segment that starts here]` */
export type NavIconStop = readonly [offset: number, value: string, easing?: string]

/** Keyframes for one property; every segment uses `easing` unless its stop names another. */
export function frames(
    property: string,
    stops: readonly NavIconStop[],
    easing = 'ease-in-out',
): Keyframe[] {
    return stops.map(([offset, value, segmentEasing]) => ({
        offset,
        [property]: value,
        easing: segmentEasing ?? easing,
    }))
}

/**
 * A loop that swings to `amplitude` on one side and back through the rest pose
 * to the other, shaped like a sine: fastest through the middle, still at the ends.
 */
export function swing(transform: (amount: number) => string, amplitude: number): Keyframe[] {
    return frames('transform', [
        [0, transform(0), 'ease-out'],
        [0.25, transform(-amplitude), 'ease-in'],
        [0.5, transform(0), 'ease-out'],
        [0.75, transform(amplitude), 'ease-in'],
        [1, transform(0)],
    ])
}

/** A loop that moves from `rest` to `peak` and settles back, still at both ends. */
export function bob(transform: (amount: number) => string, peak: number, rest = 0): Keyframe[] {
    return frames('transform', [
        [0, transform(rest)],
        [0.5, transform(peak)],
        [1, transform(rest)],
    ])
}

/**
 * Wipes a `pathLength="1"` stroke off and draws it back in from its start. With
 * a 1 + 1.2 dash period, one continuous offset sweep first retracts the dash
 * (fully hidden between -1 and -1.2) and then brings the previous dash in, so
 * nothing jumps or flashes. Dashes cannot be summed, so this replaces.
 */
export function redraw(duration: number): NavIconClip {
    const dash = { strokeDasharray: '1 1.2' }
    return {
        duration,
        composite: 'replace',
        keyframes: [
            { ...dash, offset: 0, strokeDashoffset: '0', easing: 'ease-in-out' },
            { ...dash, offset: 0.16, strokeDashoffset: '-1.1' },
            { ...dash, offset: 0.26, strokeDashoffset: '-1.1', easing: 'ease-in-out' },
            { ...dash, offset: 0.62, strokeDashoffset: '-2.2' },
            { ...dash, offset: 1, strokeDashoffset: '-2.2' },
        ],
    }
}
