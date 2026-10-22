import assert from 'node:assert/strict'
import test from 'node:test'

import { createWindowAppearanceController, type WindowAppearance } from './window-appearance.ts'

const transparent: WindowAppearance = {
    os: 'Windows',
    transparent: true,
    blur: true,
    decorated: false,
    maximized: false,
}

test('applies an update that arrives in the microtask where the previous request settles', async () => {
    let release!: (value: { cssBorder: boolean; warnings: string[] }) => void
    const response = new Promise<{ cssBorder: boolean; warnings: string[] }>((resolve) => {
        release = resolve
    })
    const applied: boolean[] = []
    const controller = createWindowAppearanceController(
        async () => {},
        (state) => {
            applied.push(state.transparent)
            return applied.length === 1
                ? response
                : Promise.resolve({ cssBorder: false, warnings: [] })
        },
        () => {},
        () => {},
    )
    const first = controller.update(transparent)
    await Promise.resolve()
    release({ cssBorder: true, warnings: [] })
    let last!: Promise<void>
    queueMicrotask(() => {
        last = controller.update({ ...transparent, transparent: false })
    })
    await first
    await last
    assert.deepEqual(applied, [true, false])
})

test('serializes updates, drops intermediate states, and never publishes a stale frame result', async () => {
    let release!: () => void
    const blocked = new Promise<void>((resolve) => {
        release = resolve
    })
    const applied: WindowAppearance[] = []
    const borders: boolean[] = []
    const controller = createWindowAppearanceController(
        async () => {},
        async (state) => {
            applied.push(state)
            if (applied.length === 1) await blocked
            return { cssBorder: true, warnings: [] }
        },
        (enabled) => borders.push(enabled),
        () => {},
    )
    const first = controller.update(transparent)
    const second = controller.update({ ...transparent, blur: false })
    const last = controller.update({ ...transparent, transparent: false })
    release()
    await Promise.all([first, second, last])
    assert.deepEqual(
        applied.map((state) => [state.transparent, state.blur]),
        [
            [true, true],
            [false, true],
        ],
    )
    assert.ok(borders.every((value) => !value))
    await controller.update(transparent)
    assert.equal(borders.at(-1), true)
})

test('native decorations and maximization suppress CSS, then restoration reapplies it', async () => {
    let border = false
    const controller = createWindowAppearanceController(
        async () => {},
        async () => ({ cssBorder: true, warnings: [] }),
        (enabled) => {
            border = enabled
        },
        () => {},
    )
    await controller.update(transparent)
    assert.equal(border, true)
    await controller.update({ ...transparent, decorated: true })
    assert.equal(border, false)
    await controller.update({ ...transparent, maximized: true })
    assert.equal(border, false)
    await controller.update(transparent)
    assert.equal(border, true)
})

test('a frame failure leaves CSS disabled and later updates still run', async () => {
    let fail = true
    let border = true
    const errors: unknown[] = []
    const controller = createWindowAppearanceController(
        async () => {},
        async () => {
            if (fail) throw new Error('native frame failed')
            return { cssBorder: false, warnings: ['border suppression failed'] }
        },
        (enabled) => {
            border = enabled
        },
        (error) => errors.push(error),
    )
    await controller.update(transparent)
    assert.equal(border, false)
    fail = false
    await controller.update(transparent)
    assert.equal(border, false)
    assert.equal(errors.length, 2)
})

test('unmount discards pending work and ignores the completing native operation', async () => {
    let release!: () => void
    const blocked = new Promise<void>((resolve) => {
        release = resolve
    })
    let markStarted!: () => void
    const started = new Promise<void>((resolve) => {
        markStarted = resolve
    })
    let count = 0
    let border = false
    const controller = createWindowAppearanceController(
        async () => {},
        async () => {
            count++
            markStarted()
            await blocked
            return { cssBorder: true, warnings: [] }
        },
        (enabled) => {
            border = enabled
        },
        () => {},
    )
    const first = controller.update(transparent)
    await started
    void controller.update({ ...transparent, blur: false })
    controller.dispose()
    release()
    await first
    await controller.update(transparent)
    assert.equal(count, 1)
    assert.equal(border, false)
})

test('applies the frame after effects settle and still suppresses native borders after an effect failure', async () => {
    const operations: string[] = []
    const warnings: unknown[] = []
    const controller = createWindowAppearanceController(
        async () => {
            operations.push('effects')
            throw new Error('unsupported blur')
        },
        async () => {
            operations.push('frame')
            return { cssBorder: true, warnings: [] }
        },
        () => {},
        (error) => warnings.push(error),
    )
    await controller.update(transparent)
    assert.deepEqual(operations, ['effects', 'frame'])
    assert.match(String(warnings[0]), /unsupported blur/)
})
