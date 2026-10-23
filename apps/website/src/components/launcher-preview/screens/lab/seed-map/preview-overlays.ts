import type { Quaternion } from '../../../lib/three.ts'
import { Vector3 } from '../../../lib/three.ts'

export type PreviewAxis = { axis: 'X' | 'Y' | 'Z'; x: number; y: number; depth: number }

export function seedMapPreviewAxes(camera: Quaternion): PreviewAxis[] {
    const inverse = camera.clone().invert()
    return (
        [
            ['X', new Vector3(1, 0, 0)],
            ['Y', new Vector3(0, 1, 0)],
            ['Z', new Vector3(0, 0, 1)],
        ] as const
    )
        .map(([axis, vector]) => {
            vector.applyQuaternion(inverse)
            return { axis, x: vector.x * 28, y: -vector.y * 28, depth: vector.z }
        })
        .sort((a, b) => a.depth - b.depth)
}
