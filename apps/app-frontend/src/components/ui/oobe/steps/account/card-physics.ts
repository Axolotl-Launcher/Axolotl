import type { CardMove, CardSlot } from './card-layout'

/**
 * A card's place and look as numbers: offset from the front card's place, turn in degrees,
 * size, opacity, and depth, which orders the stack (front 3, hidden 2, sides 1).
 */
export type CardPose = { x: number; y: number; r: number; s: number; o: number; d: number }

const KEYS = ['x', 'y', 'r', 's', 'o', 'd'] as const

export const SLOT_POSES: Record<CardSlot, CardPose> = {
    front: { x: 0, y: 0, r: 0, s: 1, o: 1, d: 3 },
    // Fanned out far enough that most of each head shows beside the front card.
    left: { x: -100, y: 8, r: -10, s: 0.88, o: 0.7, d: 1 },
    right: { x: 100, y: 8, r: 10, s: 0.88, o: 0.7, d: 1 },
    // Only its top edge clears the front card; its head stays covered. It sits above the
    // side cards, which never reach the strip of it that shows, so it can come out over them.
    hidden: { x: 0, y: -46, r: 0, s: 0.8, o: 0.4, d: 2 },
}

/** The side a card swings toward; the hidden card is drawn out to the right. */
export const SIDE: Record<CardSlot, number> = { front: 0, left: -1, right: 1, hidden: 1 }

/**
 * A card in motion: where it is, how fast each number is changing per second, and the
 * point it heads for first, until `viaUntil`, before turning toward its slot.
 */
export type CardMotion = {
    pose: CardPose
    velocity: CardPose
    via?: CardPose
    viaUntil?: number
}

// A spring per number, a little short of critically damped: quick off the mark, no wobble.
const STIFFNESS = 110
const DAMPING = 19
/** How long a card from rest heads for its swing point before turning for its slot. */
export const VIA_MS = 170
const STEP_S = 1 / 240
/** Slower than this (px/s), a card counts as at rest when a new move begins. */
const MOVING_SPEED = 40

/** Read for every frame; tests replace it to step the cards by hand. */
export const cardClock = { now: () => performance.now() }

const still = (): CardPose => ({ x: 0, y: 0, r: 0, s: 0, o: 0, d: 0 })

export function restingMotion(slot: CardSlot): CardMotion {
    return { pose: { ...SLOT_POSES[slot] }, velocity: still() }
}

/** A new card, a little below its slot and see-through, about to settle in. */
export function enteringMotion(slot: CardSlot): CardMotion {
    const pose = SLOT_POSES[slot]
    return { pose: { ...pose, y: pose.y + 16, o: 0 }, velocity: still() }
}

export function speedOf(motion: CardMotion) {
    return Math.hypot(motion.velocity.x, motion.velocity.y)
}

/**
 * The point a card from rest heads for first, so it goes around the others: up and out
 * when coming forward, down and aside when going back. It keeps its depth until it turns.
 */
function viaFor(move: CardMove, side: number, pose: CardPose): CardPose | undefined {
    const at = (x: number, y: number, r: number, s: number, o: number) => ({
        x,
        y,
        r,
        s,
        o,
        d: pose.d,
    })
    switch (move) {
        case 'rise':
            return at(side * 70, -170, side * 6, 0.96, 1)
        case 'draw':
            return at(side * 120, -175, side * 8, 0.9, 1)
        case 'sink':
            return at(side * 60, 90, side * 5, 0.88, 0.9)
        case 'stow':
            return at(side * -50, 70, side * -4, 0.88, 0.8)
        default:
            return undefined
    }
}

/**
 * Starts a move from wherever the card is. A card already moving keeps its speed and just
 * steers for its new slot, so a pick made mid-flight bends its path or turns it back; a card
 * at rest swings around the others on its way.
 */
export function startMove(
    motion: CardMotion,
    move: CardMove,
    side: number,
    now: number,
): CardMotion {
    const fromRest = speedOf(motion) < MOVING_SPEED
    const via = fromRest ? viaFor(move, side, motion.pose) : undefined
    return {
        pose: { ...motion.pose },
        velocity: { ...motion.velocity },
        via,
        viaUntil: via ? now + VIA_MS : undefined,
    }
}

/** Advances a card by `dt` seconds toward `target`, ending at `now`. */
export function stepMotion(
    motion: CardMotion,
    target: CardPose,
    dt: number,
    now: number,
): CardMotion {
    const pose = { ...motion.pose }
    const velocity = { ...motion.velocity }
    const steps = Math.min(Math.ceil(dt / STEP_S), 480)
    const h = steps > 0 ? dt / steps : 0
    for (let i = 1; i <= steps; i++) {
        const t = now - (dt - i * h) * 1000
        const goal = motion.via && motion.viaUntil && t < motion.viaUntil ? motion.via : target
        for (const key of KEYS) {
            velocity[key] += (STIFFNESS * (goal[key] - pose[key]) - DAMPING * velocity[key]) * h
            pose[key] += velocity[key] * h
        }
    }
    const viaDone = !motion.viaUntil || now >= motion.viaUntil
    return viaDone ? { pose, velocity } : { ...motion, pose, velocity }
}

/** Whether a card has come to rest at `target`; it is then snapped exactly onto it. */
export function settle(motion: CardMotion, target: CardPose): CardMotion | null {
    if (motion.via) return null
    // Within a tenth of a pixel and barely moving: the snap cannot be seen.
    const pixels = (key: string) => key === 'x' || key === 'y' || key === 'r'
    const close = KEYS.every(
        (key) => Math.abs(motion.pose[key] - target[key]) < (pixels(key) ? 0.1 : 0.002),
    )
    const slow = KEYS.every((key) => Math.abs(motion.velocity[key]) < (pixels(key) ? 1 : 0.01))
    return close && slow ? { pose: { ...target }, velocity: still() } : null
}

export function cardTransform(pose: CardPose) {
    return `translate(${pose.x}px, ${pose.y}px) rotate(${pose.r}deg) scale(${pose.s})`
}

/** Stacking order follows depth, so two crossing cards trade places where their depths cross. */
export function layerOf(pose: CardPose) {
    return Math.round(pose.d * 100)
}

/** How much of the front card's outline and button a card shows: all of it only in front. */
export function frontAmount(pose: CardPose) {
    return Math.min(Math.max(pose.d - 2, 0), 1)
}
