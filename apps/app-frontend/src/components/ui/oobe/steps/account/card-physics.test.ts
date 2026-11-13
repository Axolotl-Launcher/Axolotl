import { describe, expect, it } from 'vitest'

import {
    type CardMotion,
    type CardPose,
    layerOf,
    restingMotion,
    settle,
    SLOT_POSES,
    startMove,
    stepMotion,
} from './card-physics'

const FRAME_MS = 16

/** Runs a card for `ms`, one frame at a time from `start`, and returns every frame's pose. */
function run(motion: CardMotion, target: CardPose, ms: number, start = 0) {
    const frames: { t: number; motion: CardMotion }[] = []
    let current = motion
    for (let t = start + FRAME_MS; t <= start + ms; t += FRAME_MS) {
        current = stepMotion(current, target, FRAME_MS / 1000, t)
        frames.push({ t, motion: current })
    }
    return frames
}

const distance = (a: CardPose, b: CardPose) => Math.hypot(a.x - b.x, a.y - b.y)

describe('card motion', () => {
    it('comes to rest exactly on its slot', () => {
        const frames = run(startMove(restingMotion('right'), 'rise', 1, 0), SLOT_POSES.front, 1600)
        const last = frames[frames.length - 1]!.motion
        expect(settle(last, SLOT_POSES.front)?.pose).toEqual(SLOT_POSES.front)
    })

    it('swings up around the front card and never stalls on the way', () => {
        const frames = run(startMove(restingMotion('right'), 'rise', 1, 0), SLOT_POSES.front, 1200)
        expect(Math.min(...frames.map((frame) => frame.motion.pose.y))).toBeLessThan(-40)
        // A card still far from its slot keeps moving: no pause at the top of the swing.
        for (let i = 1; i < frames.length; i++) {
            const pose = frames[i]!.motion.pose
            if (distance(pose, SLOT_POSES.front) < 15) break
            expect(distance(pose, frames[i - 1]!.motion.pose)).toBeGreaterThan(1)
        }
    })

    it('keeps its speed when a new pick redirects it mid-flight', () => {
        const flying = run(startMove(restingMotion('right'), 'rise', 1, 0), SLOT_POSES.front, 144)
        const before = flying[flying.length - 2]!.motion.pose
        const at = flying[flying.length - 1]!.motion
        // Taken back: it is sent back to the right, without a swing of its own.
        const back = startMove(at, 'sink', 1, 144)
        expect(back.via).toBeUndefined()
        const after = stepMotion(back, SLOT_POSES.right, FRAME_MS / 1000, 160).pose
        const stepBefore = distance(at.pose, before)
        const stepAfter = distance(after, at.pose)
        expect(stepAfter).toBeGreaterThan(stepBefore * 0.6)
        expect(stepAfter).toBeLessThan(stepBefore * 1.4)
    })

    it('lets two swapping cards trade stacking order only once they have swung apart', () => {
        const rising = run(startMove(restingMotion('right'), 'rise', 1, 0), SLOT_POSES.front, 900)
        const sinking = run(startMove(restingMotion('front'), 'sink', 1, 0), SLOT_POSES.right, 900)
        const crossing = rising.findIndex(
            (frame, i) => layerOf(frame.motion.pose) > layerOf(sinking[i]!.motion.pose),
        )
        expect(crossing).toBeGreaterThan(0)
        const risingThen = rising[crossing]!.motion.pose
        const sinkingThen = sinking[crossing]!.motion.pose
        expect(risingThen.y).toBeLessThan(sinkingThen.y - 40)
        expect(layerOf(rising[rising.length - 1]!.motion.pose)).toBe(layerOf(SLOT_POSES.front))
    })
})
