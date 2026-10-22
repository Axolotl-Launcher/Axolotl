import assert from 'node:assert/strict'
import test from 'node:test'

import { appendMeasurements, MeasurementInputError, parseMeasurements } from './measurements.ts'

const first = '/execute in minecraft:overworld run tp @s 199.50 71.00 -63.50 60.23 -31.39'

test('reads F3+C positions and yaw without mixing Y, Z and pitch', () => {
    const measurement = parseMeasurements(first)[0]
    assert.equal(measurement.x, 199.5)
    assert.equal(measurement.z, -63.5)
    assert.ok(Math.abs(measurement.angle - 60.23) < 1e-10)
    assert.equal(
        parseMeasurements(first.replace('60.23', '420.23'))[0].angle,
        parseMeasurements(first)[0].angle,
    )
})

test('rejects incomplete, foreign-dimension and nonnumeric commands', () => {
    for (const text of [
        '',
        first.replace(' -31.39', ''),
        first.replace('minecraft:overworld', 'minecraft:the_nether'),
        first.replace('199.50', 'NaN'),
        first.replace('199.50', '~1'),
    ]) {
        assert.throws(() => parseMeasurements(text), MeasurementInputError)
    }
})

test('adding a batch is atomic and duplicate observations cannot increase certainty', () => {
    const previous = parseMeasurements(first)
    assert.throws(
        () => appendMeasurements(previous, [{ x: 200, z: 100, angle: 0 }, previous[0]]),
        MeasurementInputError,
    )
    assert.equal(previous.length, 1)
    assert.throws(
        () => appendMeasurements(previous, [{ x: Infinity, z: 0, angle: 0 }]),
        MeasurementInputError,
    )
})
