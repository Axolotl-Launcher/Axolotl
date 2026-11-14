import type { Chunk } from './core'
import type { Measurement } from './measurements'

export type View = { x: number; z: number; span: number }

// Keep doubles: the posterior may contain very small but meaningful weights.
export function packHeatmap(chunks: Chunk[]) {
    const positive = chunks.filter((c) => Number.isFinite(c.weight) && c.weight > 0)
    const packed = new Float64Array(positive.length * 3)
    positive.forEach((c, i) => packed.set([c.x, c.z, c.weight], i * 3))
    return packed
}

export function direction(angle: number) {
    const radians = (angle * Math.PI) / 180
    return { x: -Math.sin(radians), z: Math.cos(radians) }
}

export function fitView(eyes: Measurement[], heatmap: Float64Array, aspect: number): View {
    const points = eyes.map(({ x, z }) => ({ x, z }))
    const threshold = (heatmap[2] || 0) * 0.9
    for (let i = 0; i < heatmap.length && heatmap[i + 2] >= threshold; i += 3) {
        points.push({ x: heatmap[i] * 16 + 8, z: heatmap[i + 1] * 16 + 8 })
    }
    if (!points.length) return { x: 0, z: 0, span: 4096 }
    let minX = Infinity,
        maxX = -Infinity,
        minZ = Infinity,
        maxZ = -Infinity
    for (const p of points) {
        minX = Math.min(minX, p.x)
        maxX = Math.max(maxX, p.x)
        minZ = Math.min(minZ, p.z)
        maxZ = Math.max(maxZ, p.z)
    }
    return {
        x: (minX + maxX) / 2,
        z: (minZ + maxZ) / 2,
        span: Math.max(128, (maxX - minX) * 1.25, (maxZ - minZ) * aspect * 1.25),
    }
}

export function worldAt(view: View, width: number, height: number, x: number, y: number) {
    return {
        x: view.x + ((x - width / 2) * view.span) / width,
        z: view.z + ((y - height / 2) * view.span) / width,
    }
}

export function zoomAt(view: View, factor: number, anchor: { x: number; z: number }): View {
    const span = Math.min(120_000_000, Math.max(32, view.span * factor))
    const ratio = span / view.span
    return {
        x: anchor.x + (view.x - anchor.x) * ratio,
        z: anchor.z + (view.z - anchor.z) * ratio,
        span,
    }
}

/** Axis ticks always coincide with chunk boundaries, including at overview scales. */
export function chunkGridStep(span: number) {
    return 16 * 2 ** Math.max(0, Math.ceil(Math.log2(span / (7 * 16))))
}
