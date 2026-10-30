import { beforeEach, expect, it, vi } from 'vitest'

/**
 * The command arguments are compared character for character with the Rust
 * command signature (`apps/app/src/api/lab.rs`): a camelCase/snake_case mismatch
 * deserializes to `None` on the Rust side rather than failing, so the mistake
 * would only show up as a scan that silently ignored a setting.
 */

const invoke = vi.fn(async () => ({ seed: '', regions: 1, hits: [], elapsedMs: 0, lowestY: null }))

vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }))

beforeEach(() => {
    invoke.mockClear()
})

it('sends every scan argument under the name the command expects', async () => {
    const { scanLowYSwampHut } = await import('./client')

    await scanLowYSwampHut({
        seed: '12345',
        regions: 8,
        threads: 4,
        version: '1.21',
        preset: 'LARGE_BIOMES',
        maxY: -50,
        phase2MaxY: -54,
    })

    expect(invoke).toHaveBeenCalledOnce()
    const [command, payload] = invoke.mock.calls[0] as [string, Record<string, unknown>]
    expect(command).toBe('plugin:lab|low_y_swamp_hut_scan')
    expect(payload).toEqual({
        seed: '12345',
        regions: 8,
        threads: 4,
        version: '1.21',
        preset: 'LARGE_BIOMES',
        maxY: -50,
        phase2MaxY: -54,
    })
})

it('omits the optional parameters instead of sending placeholders', async () => {
    const { scanLowYSwampHut } = await import('./client')

    await scanLowYSwampHut({ seed: '7', regions: 2, threads: 0 })

    const [, payload] = invoke.mock.calls[0] as [string, Record<string, unknown>]
    expect(payload.version).toBeUndefined()
    expect(payload.preset).toBeUndefined()
    expect(payload.maxY).toBeUndefined()
    expect(payload.phase2MaxY).toBeUndefined()
})

it('probes availability through its own command', async () => {
    const { lowYSwampHutAvailable } = await import('./client')
    invoke.mockResolvedValueOnce(true)

    await expect(lowYSwampHutAvailable()).resolves.toBe(true)
    expect((invoke.mock.calls[0] as [string])[0]).toBe('plugin:lab|low_y_swamp_hut_available')
})
