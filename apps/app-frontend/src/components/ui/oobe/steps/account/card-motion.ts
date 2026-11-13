import type { CardMove, CardSlot } from './card-layout'

/** Where a card is and how it looks: a resting slot, or wherever a move has got it to. */
export type CardPose = { transform: string; opacity: number; layer: number }

/** Each slot's pose, relative to the front card; every pose uses the same functions so they interpolate. */
export const POSES: Record<CardSlot, string> = {
    front: 'translate(0px, 0px) rotate(0deg) scale(1)',
    // Fanned out far enough that most of each head shows beside the front card.
    left: 'translate(-100px, 8px) rotate(-10deg) scale(0.88)',
    right: 'translate(100px, 8px) rotate(10deg) scale(0.88)',
    // Only its top edge clears the front card; its head stays covered.
    hidden: 'translate(0px, -46px) rotate(0deg) scale(0.8)',
}

export const OPACITY: Record<CardSlot, number> = { front: 1, left: 0.7, right: 0.7, hidden: 0.4 }

/**
 * Stacking order. The hidden card sits above the side cards, which never reach the strip of
 * it that shows, so it can come out over them.
 */
export const LAYER: Record<CardSlot, number> = { front: 3, left: 1, right: 1, hidden: 2 }

/** The side a card swings toward; the hidden card is drawn out to the right. */
export const SIDE: Record<CardSlot, number> = { front: 0, left: -1, right: 1, hidden: 1 }

export const CARD_MOTION_MS: Record<CardMove, number> = {
    rise: 720,
    sink: 720,
    draw: 820,
    stow: 820,
    shift: 560,
    enter: 420,
}

const EASE = 'cubic-bezier(0.45, 0, 0.2, 1)'
/** A card already in motion keeps going instead of starting again from rest. */
const EASE_MOVING = 'cubic-bezier(0.2, 0, 0.2, 1)'

export function restingPose(slot: CardSlot): CardPose {
    return { transform: POSES[slot], opacity: OPACITY[slot], layer: LAYER[slot] }
}

/** The pose halfway through a move, where two crossing cards trade stacking order. */
function waypoint(move: CardMove, side: number): { transform: string; opacity: number } | null {
    switch (move) {
        // Up and out from behind the front card.
        case 'rise':
            return {
                transform: `translate(${side * 80}px, -84px) rotate(${side * 5}deg) scale(0.96)`,
                opacity: 1,
            }
        // Pulled up and out of the deck to the side.
        case 'draw':
            return {
                transform: `translate(${side * 70}px, -100px) rotate(${side * 6}deg) scale(0.88)`,
                opacity: 1,
            }
        // Down and aside, out of the way of the card coming forward.
        case 'sink':
            return {
                transform: `translate(${side * 40}px, 34px) rotate(${side * 4}deg) scale(0.9)`,
                opacity: 0.9,
            }
        case 'stow':
            return {
                transform: `translate(${side * -28}px, 26px) rotate(${side * -3}deg) scale(0.9)`,
                opacity: 0.9,
            }
        default:
            return null
    }
}

/**
 * Keyframes that take a card from `start`, wherever it is, to slot `to`. The card keeps its
 * stacking order until halfway, then takes the slot's, so a card coming forward passes in
 * front only once it is clear. `moving` says `start` is mid-flight: such a card is already
 * out of line, so it heads straight for its new place instead of swinging out again, and a
 * pick taken back plays as the cards turning back.
 */
export function cardKeyframes(
    start: CardPose,
    to: CardSlot,
    move: CardMove,
    side: number,
    moving = false,
): Keyframe[] {
    const end = restingPose(to)
    if (move === 'enter') {
        return [
            { transform: `translateY(16px) ${end.transform}`, opacity: 0, zIndex: end.layer },
            { transform: end.transform, opacity: end.opacity, zIndex: end.layer },
        ]
    }
    const easing = moving ? EASE_MOVING : EASE
    const middle = moving ? null : waypoint(move, side)
    const first: Keyframe = {
        offset: 0,
        transform: start.transform,
        opacity: start.opacity,
        zIndex: start.layer,
        easing,
    }
    const last: Keyframe = {
        offset: 1,
        transform: end.transform,
        opacity: end.opacity,
        zIndex: end.layer,
    }
    if (!middle) {
        return [
            first,
            { offset: 0.5, zIndex: start.layer },
            { offset: 0.51, zIndex: end.layer },
            last,
        ]
    }
    return [
        first,
        { offset: 0.5, ...middle, zIndex: start.layer, easing: EASE },
        { offset: 0.51, zIndex: end.layer },
        last,
    ]
}
