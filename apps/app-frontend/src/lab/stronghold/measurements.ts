export type Measurement = { x: number; z: number; angle: number }
export type MeasurementError = 'format' | 'dimension' | 'range' | 'duplicate' | 'limit'

export class MeasurementInputError extends Error {
    readonly reason: MeasurementError
    constructor(reason: MeasurementError) {
        super(reason)
        this.reason = reason
    }
}

export function validateMeasurement(value: Measurement): Measurement {
    if (
        ![value.x, value.z, value.angle].every(Number.isFinite) ||
        Math.abs(value.x) > 30_000_000 ||
        Math.abs(value.z) > 30_000_000
    ) {
        throw new MeasurementInputError('range')
    }
    return { ...value, angle: ((((value.angle + 180) % 360) + 360) % 360) - 180 }
}

/** Accept only complete F3+C commands; never extract coordinates from arbitrary text. */
export function parseMeasurements(text: string): Measurement[] {
    const lines = text
        .trim()
        .split(/\r?\n/)
        .filter((line) => line.trim())
    if (!lines.length) throw new MeasurementInputError('format')
    return lines.map((line) => {
        const parts = line.trim().split(/\s+/)
        if (
            parts.length !== 11 ||
            parts[0] !== '/execute' ||
            parts[1] !== 'in' ||
            parts[3] !== 'run' ||
            parts[4] !== 'tp' ||
            parts[5] !== '@s'
        ) {
            throw new MeasurementInputError('format')
        }
        if (parts[2] !== 'minecraft:overworld') throw new MeasurementInputError('dimension')
        const numbers = parts
            .slice(6)
            .map((value) => (/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/.test(value) ? Number(value) : NaN))
        if (!numbers.every(Number.isFinite)) throw new MeasurementInputError('format')
        return validateMeasurement({ x: numbers[0], z: numbers[2], angle: numbers[3] })
    })
}

/** A paste batch either succeeds completely or leaves the previous observations intact. */
export function appendMeasurements(existing: Measurement[], added: Measurement[]): Measurement[] {
    if (existing.length + added.length > 100) throw new MeasurementInputError('limit')
    const result = [...existing]
    for (const item of added) {
        const value = validateMeasurement(item)
        if (
            result.some(
                (other) =>
                    other.x === value.x &&
                    other.z === value.z &&
                    Math.abs(other.angle - value.angle) < 1e-10,
            )
        ) {
            throw new MeasurementInputError('duplicate')
        }
        result.push(value)
    }
    return result
}
