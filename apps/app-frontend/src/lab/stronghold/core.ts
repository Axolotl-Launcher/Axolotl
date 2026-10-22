// SPDX-License-Identifier: GPL-3.0-only
// Derived from Ninjabrain1/Ninjabrain-Bot, commit 44cc82c89ece79fcfad2b9ffb1c8ede5ec8dae16.
// Ordinary eye throws only: no divine, boat, blind or all-advancements modes.

export type Eye = { x: number; z: number; angle: number; sigma: number }
export type Input = { id?: string; version: 'pre119' | 'post119'; advanced: boolean; eyes: Eye[] }
export type Chunk = { x: number; z: number; weight: number }
type Ring = {
    ring: number
    n: number
    inner: number
    outer: number
    postInner: number
    postOuter: number
}

const snapping = 7
const maxChunk = Math.trunc(32 * (4 + 7 * 6 + 0.5 * 2.5) + 2 * snapping + 1)
const rings: Ring[] = []
let count = 1,
    total = 0
for (let k = 0; k < 8; k++) {
    count += Math.trunc((2 * count) / (k + 1))
    count = Math.min(count, 128 - total)
    total += count
    const inner = 32 * (4 + k * 6 - 0.5 * 2.5)
    const outer = 32 * (4 + k * 6 + 0.5 * 2.5)
    rings.push({
        ring: k,
        n: count,
        inner,
        outer,
        postInner: inner - (snapping + 1) * Math.sqrt(2),
        postOuter: outer + (snapping + 1) * Math.sqrt(2),
    })
}

let densityTable: Float64Array | undefined
let cumulativeTable: Float64Array
function initDensity() {
    if (densityTable) return
    densityTable = new Float64Array(maxChunk + 5)
    for (const r of rings) {
        for (let i = Math.trunc(r.inner); i <= Math.trunc(r.outer); i++) {
            let rho = r.n / (2 * Math.PI * (r.outer - r.inner) * i)
            if (i === Math.trunc(r.inner) || i === Math.trunc(r.outer)) rho *= 0.5
            densityTable[i] = rho
        }
    }
    const filter = new Float64Array(Math.ceil(snapping * Math.sqrt(2)) + 1)
    const offsets = new Map<number, number>()
    for (let i = -26; i <= 30; i++) {
        const v = -(i >> 2)
        offsets.set(v, (offsets.get(v) ?? 0) + 1)
    }
    let sum = 0
    for (let k = -snapping; k <= snapping; k++) {
        for (let l = -snapping; l <= snapping; l++) {
            const w = offsets.get(k)! * offsets.get(l)!
            for (let i = 0; i < 200; i++) {
                const phi = (2 * Math.PI * i) / 200
                // Preserve the explicit Java float cast before rounding to an integer.
                const f = Math.fround(Math.sqrt(k * k + l * l) * Math.sin(phi))
                const dr = Math.abs(Math.round(f))
                filter[dr] += w
                sum += dr === 0 ? w : 2 * w
            }
        }
    }
    for (let i = 0; i < filter.length; i++) filter[i] /= sum
    const old = densityTable
    densityTable = new Float64Array(maxChunk + 5)
    for (let i = 0; i < densityTable.length; i++) {
        for (let j = -filter.length + 1; j < filter.length; j++) {
            if (i + j >= 0 && i + j < old.length)
                densityTable[i] += old[i + j] * filter[Math.abs(j)]
        }
    }
    cumulativeTable = new Float64Array(maxChunk + 5)
    let accumulated = 0
    for (let i = 0; i < cumulativeTable.length; i++) {
        accumulated += densityTable[i] * i * 2 * Math.PI
        cumulativeTable[i] = accumulated
    }
}

function density(x: number, z: number) {
    initDensity()
    const k = Math.sqrt(x * x + z * z),
        i0 = Math.trunc(k),
        i1 = i0 + 1,
        t = k - i0
    return i1 < densityTable!.length ? (1 - t) * densityTable![i0] + t * densityTable![i1] : 0
}

/** Reference bands enclosing the base model's biome-adjusted generation support, in blocks. */
export function generationBands() {
    const padding = Math.ceil(snapping * Math.sqrt(2)) + 1 + Math.SQRT2
    return rings.map((ring) => ({
        inner: (Math.floor(ring.inner) - padding) * 16,
        outer: (Math.floor(ring.outer) + padding) * 16,
    }))
}

/** Same four-corner prior as candidate enumeration; independent of the player's observations. */
export function isGenerationChunk(x: number, z: number) {
    return (
        density(x - 0.5, z - 0.5) +
            density(x - 0.5, z + 0.5) +
            density(x + 0.5, z - 0.5) +
            density(x + 0.5, z + 0.5) >
        0
    )
}
function cumulative(r: number) {
    if (r < 0) return 0
    initDensity()
    const i0 = Math.trunc(r),
        i1 = i0 + 1,
        t = r - i0
    return i1 < cumulativeTable.length
        ? (1 - t) * cumulativeTable[i0] + t * cumulativeTable[i1]
        : cumulativeTable[cumulativeTable.length - 1]
}

function maxDistance(x: number, z: number) {
    const r = Math.sqrt(x * x + z * z) / 16
    let d = Infinity
    for (const ring of rings) {
        const a = ring.inner * ring.inner + r * r - 2 * r * ring.inner * Math.cos(Math.PI / ring.n)
        const b = ring.outer * ring.outer + r * r - 2 * r * ring.outer * Math.cos(Math.PI / ring.n)
        d = Math.min(d, Math.sqrt(a > b ? a : b))
    }
    return (d + Math.sqrt(2) * (snapping + 0.5)) * 16
}

function startMajor(
    om: number,
    on: number,
    ux: number,
    uz: number,
    vx: number,
    vz: number,
    mx: boolean,
    positive: boolean,
) {
    if (om * om + on * on <= maxChunk * maxChunk) return om
    const ox = mx ? om : on,
        oz = mx ? on : om
    function orthogonal(ax: number, az: number, x: number, z: number) {
        const p = x * ax + z * az
        return z * (x * p - ax) - x * (z * p - az)
    }
    function project(ax: number, az: number, x: number, z: number) {
        const p = ax * x + az * z
        return mx ? x * p : z * p
    }
    function intersection(x: number, z: number) {
        const dot = ox * x + oz * z
        const a = dot * dot + maxChunk * maxChunk - ox * ox - oz * oz
        if (a < 0) return 0
        const b = -dot - Math.sqrt(a)
        return mx ? ox + b * x : oz + b * z
    }
    if (orthogonal(-ox, -oz, ux, uz) > 0 && orthogonal(-ox, -oz, vx, vz) < 0) {
        const magnitude = Math.sqrt(ox * ox + oz * oz)
        const ix = (ox / magnitude) * maxChunk,
            iz = (oz / magnitude) * maxChunk
        const m1 = om + project(ix - ox, iz - oz, ux, uz)
        const m2 = om + project(ix - ox, iz - oz, vx, vz)
        return positive !== m1 > m2 ? m1 : m2
    }
    const u = intersection(ux, uz),
        v = intersection(vx, vz)
    if (u !== 0 || v !== 0) {
        if (u !== 0 && v !== 0) return positive !== u > v ? u : v
        return u !== 0 ? u : v
    }
    return om
}

function prior(eye: Eye, offset: number): Chunk[] {
    const tolerance = (Math.min(1, 30 * eye.sigma) / 180) * Math.PI
    const phi = (eye.angle / 180) * Math.PI
    const dx = -Math.sin(phi),
        dz = Math.cos(phi)
    const ux = -Math.sin(phi - tolerance),
        uz = Math.cos(phi - tolerance)
    const vx = -Math.sin(phi + tolerance),
        vz = Math.cos(phi + tolerance)
    const mx = Math.cos(phi) * Math.cos(phi) < 0.5
    const positive = mx ? -Math.sin(phi) > 0 : Math.cos(phi) > 0
    const om = ((mx ? eye.x : eye.z) - offset) / 16
    const on = ((mx ? eye.z : eye.x) - offset) / 16
    const start = startMajor(om, on, ux, uz, vx, vz, mx, positive)
    const uk = mx ? uz / ux : ux / uz,
        vk = mx ? vz / vx : vx / vz
    const right = positive ? vk - uk > 0 : uk - vk > 0
    let i = Math.trunc(positive ? Math.ceil(start) : Math.floor(start))
    const chunks: Chunk[] = []
    while ((mx ? (i - start) / dx : (i - start) / dz) < 5000 / 16) {
        const minorU = on + uk * (i - om),
            minorV = on + vk * (i - om)
        let j = Math.trunc(right ? Math.ceil(minorU) : Math.floor(minorU))
        j = Math.max(-maxChunk, Math.min(maxChunk, j))
        // Preserve upstream conditional-expression precedence for differential testing.
        while (right ? j < minorV : j > minorV && j <= maxChunk && j >= -maxChunk) {
            const x = mx ? i : j,
                z = mx ? j : i
            let weight = 0
            for (let k = 0; k < 2; k++) {
                for (let l = 0; l < 2; l++) weight += density(x - 0.5 + k, z - 0.5 + l)
            }
            chunks.push({ x, z, weight: weight / 4 })
            j += right ? 1 : -1
        }
        i += positive ? 1 : -1
    }
    return chunks
}

function positionVariance(distance2: number, x: number, z: number) {
    const corner = (a: number) =>
        Math.abs(a - Math.floor(a) - 0.3) < 1e-6 || Math.abs(a - Math.floor(a) - 0.7) < 1e-6
    if (corner(x) && corner(z)) return 0
    const maxError = (0.005 * Math.sqrt(2) * 180) / Math.PI
    return (maxError * maxError) / distance2 / 6
}

function normalize(chunks: Chunk[]) {
    let sum = 0
    for (const c of chunks) sum += c.weight
    for (const c of chunks) c.weight /= sum
}

// Java Double.compare treats NaN as greater than all finite values.
function compare(a: number, b: number): number {
    if (Number.isNaN(a)) return Number.isNaN(b) ? 0 : 1
    if (Number.isNaN(b)) return -1
    if (a === b)
        return Object.is(a, -0) && !Object.is(b, -0)
            ? -1
            : Object.is(b, -0) && !Object.is(a, -0)
              ? 1
              : 0
    return a < b ? -1 : 1
}

function integral(
    ring: Ring,
    l: number,
    phiPrime: number,
    dphi: number,
    phiP: number,
    rp: number,
    di: number,
    same: boolean,
) {
    const mu = phiPrime + (l * 2 * Math.PI) / ring.n
    let pdfint = 0,
        value = 0
    for (let k = -7; k <= 7; k++) {
        const dp = k * dphi
        const pdf = same
            ? Math.pow(1 + (dp * ring.inner) / (15 * Math.sqrt(2)), 4.5) *
              Math.pow(1 - (dp * ring.inner) / (15 * Math.sqrt(2)), 4.5)
            : 1
        pdfint += pdf * dphi
        const phi = mu + k * dphi,
            gamma = phiP - phi
        const sinBeta = (rp / di) * Math.sin(gamma)
        if (sinBeta < 1 && sinBeta > -1) {
            const beta = Math.asin(sinBeta)
            const a0 = beta - gamma,
                a1 = Math.PI - gamma - beta
            let r0 = (di * Math.sin(a0)) / Math.sin(gamma),
                r1 = (di * Math.sin(a1)) / Math.sin(gamma)
            if (r1 > ring.postOuter) r1 = ring.postOuter
            if (r0 < ring.postInner) r0 = ring.postInner
            if (r0 > ring.postOuter) r0 = ring.postOuter
            if (r1 < ring.postInner) r1 = ring.postInner
            value += (pdf * (cumulative(r1) - cumulative(r0)) * dphi) / ring.n
        }
    }
    value /= pdfint
    return value > 1 ? 1 : value
}

function closestCondition(c: Chunk, eye: Eye, offset: number) {
    const dx = c.x + (offset - eye.x) / 16,
        dz = c.z + (offset - eye.z) / 16
    const rp = Math.sqrt(eye.x * eye.x + eye.z * eye.z) / 16,
        di = Math.sqrt(dx * dx + dz * dz)
    const phiPrime = -Math.atan2(c.x, c.z),
        phiP = -Math.atan2(eye.x, eye.z)
    const max = maxDistance(eye.x, eye.z) / 16
    const radius = Math.sqrt(c.x * c.x + c.z * c.z)
    const current = rings.find((r) => radius >= r.postInner && radius <= r.postOuter)
    if (!current) return 0
    let probability = 1
    for (const r of rings) {
        if (rp + max < r.inner || rp - max > r.outer) continue
        const same = current.ring === r.ring
        const dphi = same
            ? ((2 / 15) * 15 * Math.sqrt(2)) / current.inner
            : ((2 / 15) * Math.PI) / r.n
        for (let l = 0; l < r.n; l++) {
            if (same && l === 0) continue
            probability *= 1 - integral(r, l, phiPrime, dphi, phiP, rp, di, same)
        }
    }
    c.weight *= probability
    return probability
}

export function calculateRaw(input: Input) {
    if (!input.eyes.length) return { chunks: [] as Chunk[], success: false, best: null }
    const first = input.eyes[0],
        offset = input.version === 'pre119' ? 8 : 0
    const max = maxDistance(first.x, first.z) / 16
    if (max * first.sigma > 1000) return { chunks: [] as Chunk[], success: false, best: null }
    const chunks = prior(first, offset)
    for (const c of chunks) {
        const dx = c.x - first.x / 16,
            dz = c.z - first.z / 16
        if (dx * dx + dz * dz > max * max) c.weight = 0
    }
    for (const eye of input.eyes) {
        for (const c of chunks) {
            const dx = c.x * 16 + offset - eye.x,
                dz = c.z * 16 + offset - eye.z
            const gamma = (-180 / Math.PI) * Math.atan2(dx, dz)
            let delta = Math.abs((gamma - eye.angle) % 360)
            delta = Math.min(delta, 360 - delta)
            const variance =
                eye.sigma * eye.sigma + positionVariance(dx * dx + dz * dz, eye.x, eye.z)
            c.weight *= Math.exp((-delta * delta) / (2 * variance))
        }
        normalize(chunks)
    }
    if (input.advanced) {
        chunks.sort((a, b) => -compare(a.weight, b.weight))
        let total = 0,
            samples = 0
        for (let i = 0; i < chunks.length; i++) {
            const c = chunks[i]
            if (i < 100 || c.weight > 0.001) {
                total += closestCondition(c, first, offset)
                samples++
            } else c.weight *= total / samples
        }
        normalize(chunks)
    }
    let best: Chunk | null = null
    for (const c of chunks) if (best === null || compare(c.weight, best.weight) > 0) best = c
    const success = best !== null && Number.isFinite(best.weight) && best.weight > 0.0005
    chunks.sort((a, b) => -compare(a.weight, b.weight))
    return { chunks, best, success }
}

export function calculate(input: Input, includeDistribution = false) {
    if (!['pre119', 'post119'].includes(input.version) || typeof input.advanced !== 'boolean')
        throw new Error('Invalid settings')
    if (input.eyes.length > 100) throw new Error('Too many measurements')
    for (const eye of input.eyes) {
        if (
            ![eye.x, eye.z, eye.angle, eye.sigma].every(Number.isFinite) ||
            eye.sigma <= 0 ||
            Math.abs(eye.x) > 30_000_000 ||
            Math.abs(eye.z) > 30_000_000 ||
            eye.sigma > 10
        )
            throw new Error('Invalid measurement')
    }
    const raw = calculateRaw(input)
    return {
        success: raw.success,
        predictions: raw.success ? raw.chunks.slice(0, 5) : [],
        status: raw.success ? 'prediction' : 'no-reliable-result',
        ...(includeDistribution ? { distribution: raw.chunks } : {}),
    }
}
