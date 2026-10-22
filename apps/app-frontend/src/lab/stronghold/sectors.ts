import { type Chunk, isGenerationChunk } from './core.ts'
import type { Measurement } from './measurements.ts'

export type SectorInput = {
    observations: Measurement[]
    halfAngle: number
    radius: number
    version: 'pre119' | 'post119'
}

export function sectorWeight(eye: Measurement, x: number, z: number, halfAngle: number) {
    const dx = x - eye.x,
        dz = z - eye.z
    if ((dx === 0 && dz === 0) || !(halfAngle > 0 && halfAngle <= 90)) return 0
    const yaw = (Math.atan2(-dx, dz) * 180) / Math.PI
    const delta = Math.abs(((((yaw - eye.angle + 540) % 360) + 360) % 360) - 180)
    return Math.max(0, 1 - delta / halfAngle)
}

export function contributions(input: SectorInput, chunk: { x: number; z: number }) {
    const offset = input.version === 'pre119' ? 8 : 0
    return input.observations
        .map((eye, index) => ({
            index: index + 1,
            weight: sectorWeight(
                eye,
                chunk.x * 16 + offset,
                chunk.z * 16 + offset,
                input.halfAngle,
            ),
        }))
        .filter((item) => item.weight > 0)
}

export function calculateSectors(input: SectorInput) {
    if (
        !Number.isFinite(input.radius) ||
        input.radius < 1000 ||
        input.radius > 26000 ||
        !Number.isFinite(input.halfAngle) ||
        input.halfAngle <= 0 ||
        input.halfAngle > 90
    )
        throw new Error('Invalid sector settings')
    const distribution: Chunk[] = []
    const extent = Math.ceil(input.radius / 16)
    const offset = input.version === 'pre119' ? 8 : 0
    for (let x = -extent; x <= extent; x++) {
        for (let z = -extent; z <= extent; z++) {
            const wx = x * 16 + offset,
                wz = z * 16 + offset
            if (wx * wx + wz * wz > input.radius * input.radius || !isGenerationChunk(x, z))
                continue
            let weight = 0
            for (const eye of input.observations)
                weight += sectorWeight(eye, wx, wz, input.halfAngle)
            if (weight > 0) distribution.push({ x, z, weight })
        }
    }
    distribution.sort((a, b) => b.weight - a.weight || a.x - b.x || a.z - b.z)
    return { predictions: distribution.slice(0, 100), distribution }
}
