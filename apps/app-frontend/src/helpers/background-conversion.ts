import type { NormalizedCropRect } from '@modrinth/ui'
import { invoke } from '@tauri-apps/api/core'

export type BackgroundConversionBackend = 'auto' | 'native' | 'ffmpeg'

export interface ConvertAnimatedBackgroundOptions {
    inputPath: string
    outputPath: string
    crop: NormalizedCropRect
    outputWidth: number
    outputHeight: number
    quality?: number
    maxFrames?: number
    maxDurationMs?: number
    maxOutputBytes?: number
    /** The user explicitly chose auto-compression for an oversized source. */
    allowOversizedSource?: boolean
    /** Keep the source resolution/quality instead of auto-compressing. */
    preserveOriginalQuality?: boolean
    /**
     * `native` covers GIF, APNG and animated WebP. `auto` falls back to
     * `backend_unavailable` for AVIF/MP4 until the FFmpeg settings extension
     * provides a path.
     */
    backend?: BackgroundConversionBackend
    ffmpegPath?: string | null
}

export interface ConvertedAnimatedBackground {
    path: string
    width: number
    height: number
    frameCount: number
    durationMs: number
    byteLength: number
}

export interface BackgroundConversionError {
    code: string
    message: string
}

export function isBackgroundConversionError(error: unknown): error is BackgroundConversionError {
    return (
        typeof error === 'object' &&
        error !== null &&
        typeof (error as { code?: unknown }).code === 'string' &&
        typeof (error as { message?: unknown }).message === 'string'
    )
}

/**
 * The single outward conversion entry point. The FFmpeg settings extension
 * will only need to supply `backend`/`ffmpegPath`; the command contract stays
 * stable.
 */
export function convertAnimatedBackground(
    options: ConvertAnimatedBackgroundOptions,
): Promise<ConvertedAnimatedBackground> {
    return invoke('plugin:files|background_convert_animated', { options })
}
