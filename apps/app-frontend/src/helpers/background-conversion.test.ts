import { expect, it, vi } from 'vitest'

import { convertAnimatedBackground, isBackgroundConversionError } from './background-conversion'

const invoke = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

it('sends the stable conversion contract to the Rust command', async () => {
    invoke.mockResolvedValue({
        path: 'C:/data/backgrounds/launcher-background-1-abcd.webp',
        width: 1920,
        height: 1080,
        frameCount: 24,
        durationMs: 1200,
        byteLength: 1234,
    })

    const options = {
        inputPath: 'C:/pictures/loop.gif',
        outputPath: 'C:/data/backgrounds/launcher-background-1-abcd.webp',
        crop: { left: 0.1, top: 0.2, width: 0.8, height: 0.6 },
        outputWidth: 1920,
        outputHeight: 1080,
        quality: 75,
        backend: 'auto' as const,
    }
    const result = await convertAnimatedBackground(options)

    expect(invoke).toHaveBeenCalledWith('plugin:files|background_convert_animated', {
        options,
    })
    expect(result.frameCount).toBe(24)
})

it('recognizes structured conversion errors', () => {
    expect(
        isBackgroundConversionError({
            code: 'backend_unavailable',
            message: 'AVIF needs FFmpeg',
        }),
    ).toBe(true)
    expect(isBackgroundConversionError(new Error('plain error'))).toBe(false)
    expect(isBackgroundConversionError({ code: 'decode_failed' })).toBe(false)
})
