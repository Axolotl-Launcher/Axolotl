import { describe, expect, it } from 'vitest'

import { cardKeyframes, LAYER, POSES, restingPose } from './card-motion'

describe('cardKeyframes', () => {
    it('starts where the card is, even mid-flight, and ends at rest in its slot', () => {
        const midway = {
            transform: 'matrix(0.95, 0.1, -0.1, 0.95, -60, -70)',
            opacity: 0.9,
            layer: 1,
        }
        const frames = cardKeyframes(midway, 'left', 'sink', -1, true)
        expect(frames[0]).toMatchObject({ transform: midway.transform, opacity: 0.9, zIndex: 1 })
        expect(frames[frames.length - 1]).toMatchObject({
            transform: POSES.left,
            opacity: 0.7,
            zIndex: LAYER.left,
        })
    })

    it('keeps the stacking order until halfway, then takes the slot’s', () => {
        const frames = cardKeyframes(restingPose('right'), 'front', 'rise', 1)
        const layers = frames
            .filter((frame) => frame.zIndex !== undefined)
            .map((frame) => [frame.offset, frame.zIndex])
        expect(layers).toEqual([
            [0, LAYER.right],
            [0.5, LAYER.right],
            [0.51, LAYER.front],
            [1, LAYER.front],
        ])
    })

    it('swings a card coming forward up and toward the side it came from', () => {
        const left = cardKeyframes(restingPose('left'), 'front', 'rise', -1)[1]
        const right = cardKeyframes(restingPose('right'), 'front', 'rise', 1)[1]
        expect(left.transform).toBe('translate(-80px, -84px) rotate(-5deg) scale(0.96)')
        expect(right.transform).toBe('translate(80px, -84px) rotate(5deg) scale(0.96)')
    })

    it('sends a card caught mid-flight straight on, still trading places halfway', () => {
        const midway = { transform: 'matrix(0.95, 0.1, -0.1, 0.95, 60, -70)', opacity: 1, layer: 1 }
        const frames = cardKeyframes(midway, 'right', 'sink', 1, true)
        expect(frames.filter((frame) => frame.transform)).toHaveLength(2)
        expect(frames.find((frame) => frame.offset === 0.51)).toMatchObject({
            zIndex: LAYER.right,
        })
    })

    it('moves a card between places behind in a straight line', () => {
        const frames = cardKeyframes(restingPose('hidden'), 'left', 'shift', -1)
        expect(frames.filter((frame) => frame.transform)).toHaveLength(2)
    })

    it('fades a new card in at its slot', () => {
        const [from, to] = cardKeyframes(restingPose('front'), 'right', 'enter', 1)
        expect(from).toMatchObject({ opacity: 0 })
        expect(to).toMatchObject({ transform: POSES.right, opacity: 0.7 })
    })
})
