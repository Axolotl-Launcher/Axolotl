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
 * The curve a card from rest follows: a point gliding from `from` to the slot, bent by two
 * control points, over `duration` ms from `start`. On a curve the card never stops to turn.
 */
type CardPath = {
    from: CardPose
    controls: [CardPose, CardPose]
    start: number
    duration: number
    /** Distance along the curve at each sample point, measured on first use. */
    lengths?: number[]
}

/** A card in motion: where it is, how fast each number changes per second, and its curve. */
export type CardMotion = { pose: CardPose; velocity: CardPose; path?: CardPath }

// The card chases its point on a spring, so it trails it smoothly and keeps its speed when
// a pick sends it elsewhere. A little short of critically damped: quick, no wobble.
const STIFFNESS = 290
const DAMPING = 31
const STEP_S = 1 / 240
/** Slower than this (px/s), a card counts as at rest when a new move begins. */
const MOVING_SPEED = 40

const PATH_MS: Partial<Record<CardMove, number>> = { rise: 460, sink: 460, draw: 540, stow: 540 }

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
 * Where a move from rest bends its curve, so the card goes around the others: up the side
 * and over when coming forward, down and around when going back, and a wide loop out of and
 * into the deck. The depth stays with the start until late, so crossing cards trade places
 * once they are apart.
 */
function controlsFor(
    move: CardMove,
    side: number,
    from: CardPose,
): [CardPose, CardPose] | undefined {
    const at = (x: number, y: number, r: number, s: number, o: number) => ({
        x,
        y,
        r,
        s,
        o,
        d: from.d,
    })
    switch (move) {
        case 'rise':
            return [at(side * 115, -140, side * 8, 0.94, 1), at(side * 25, -120, side * 3, 0.98, 1)]
        case 'draw':
            return [at(side * 100, -190, side * 6, 0.9, 1), at(side * 200, -10, side * 9, 0.96, 1)]
        case 'sink':
            return [at(side * 5, 85, side * 2, 0.9, 0.95), at(side * 95, 95, side * 8, 0.86, 0.8)]
        case 'stow':
            return [
                at(side * -130, 50, side * -6, 0.88, 0.8),
                at(side * -140, -130, side * -4, 0.82, 0.5),
            ]
        default:
            return undefined
    }
}

/**
 * Starts a move from wherever the card is. A card already moving keeps its speed and just
 * steers for its new slot, so a pick made mid-flight bends its path or turns it back; a card
 * at rest follows a curve around the others.
 */
export function startMove(
    motion: CardMotion,
    move: CardMove,
    side: number,
    now: number,
): CardMotion {
    const controls =
        speedOf(motion) < MOVING_SPEED ? controlsFor(move, side, motion.pose) : undefined
    const duration = PATH_MS[move]
    return {
        pose: { ...motion.pose },
        velocity: { ...motion.velocity },
        path:
            controls && duration
                ? { from: { ...motion.pose }, controls, start: now, duration }
                : undefined,
    }
}

const SAMPLES = 48

function curveAt(path: CardPath, target: CardPose, u: number): CardPose {
    const point = still()
    for (const key of KEYS) {
        const v = 1 - u
        point[key] =
            v * v * v * path.from[key] +
            3 * v * v * u * path.controls[0][key] +
            3 * v * u * u * path.controls[1][key] +
            u * u * u * target[key]
    }
    return point
}

/** How far along the curve, on screen, each of its evenly spaced sample points lies. */
function lengthsOf(path: CardPath, target: CardPose) {
    const lengths = [0]
    let previous = path.from
    for (let i = 1; i <= SAMPLES; i++) {
        const point = curveAt(path, target, i / SAMPLES)
        lengths.push(lengths[i - 1]! + Math.hypot(point.x - previous.x, point.y - previous.y))
        previous = point
    }
    return lengths
}

/**
 * The point on a path at time `t`. It is placed by distance along the curve, so it does not
 * bunch up where the curve bends: it sets off at once and only ever slows, into the slot.
 */
function pointOn(path: CardPath, target: CardPose, t: number): CardPose {
    const progress = Math.min(Math.max((t - path.start) / path.duration, 0), 1)
    const eased = 1 - (1 - progress) ** 3
    path.lengths ??= lengthsOf(path, target)
    const lengths = path.lengths
    const wanted = eased * lengths[SAMPLES]!
    let i = 0
    while (i < SAMPLES - 1 && lengths[i + 1]! < wanted) i++
    const span = lengths[i + 1]! - lengths[i]!
    const u = span > 0 ? (i + (wanted - lengths[i]!) / span) / SAMPLES : eased
    return curveAt(path, target, Math.min(u, 1))
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
        const goal = motion.path ? pointOn(motion.path, target, t) : target
        for (const key of KEYS) {
            velocity[key] += (STIFFNESS * (goal[key] - pose[key]) - DAMPING * velocity[key]) * h
            pose[key] += velocity[key] * h
        }
    }
    const pathDone = !motion.path || now >= motion.path.start + motion.path.duration
    return pathDone ? { pose, velocity } : { ...motion, pose, velocity }
}

/** Whether a card has come to rest at `target`; it is then snapped exactly onto it. */
export function settle(motion: CardMotion, target: CardPose): CardMotion | null {
    if (motion.path) return null
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
