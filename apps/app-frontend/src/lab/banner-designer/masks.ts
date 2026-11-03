import maskData from './assets/banner-pattern-masks.json' with { type: 'json' }
import type { BannerPatternId } from './patterns.ts'

/**
 * The banner texture is a 42 × 41 wrap-around of the whole banner: column 0 and
 * column 21 are the thin left and right sides, rows 1..40 of columns 1..20 are
 * the front face, and columns 22..41 mirror it for the back. Only the front face
 * is stored here, as the alpha channel of each pattern mask; including the side
 * columns would shift every pattern by a pixel.
 */
export const BANNER_FACE_WIDTH: number = maskData.face.width
export const BANNER_FACE_HEIGHT: number = maskData.face.height

const ENCODED_MASKS: Record<string, string> = maskData.patterns
const decodedMasks = new Map<string, Uint8Array>()

export function decodeMaskAlpha(encoded: string): Uint8Array {
    const binary = atob(encoded)
    const alpha = new Uint8Array(binary.length)
    for (let index = 0; index < binary.length; index++) {
        alpha[index] = binary.charCodeAt(index)
    }
    return alpha
}

/**
 * Alpha coverage of a pattern mask, or `null` for `base`, which paints the
 * whole face instead of using a texture.
 */
export function patternMask(id: BannerPatternId): Uint8Array | null {
    const cached = decodedMasks.get(id)
    if (cached) return cached
    const encoded = ENCODED_MASKS[id]
    if (!encoded) return null
    const decoded = decodeMaskAlpha(encoded)
    if (decoded.length !== BANNER_FACE_WIDTH * BANNER_FACE_HEIGHT) {
        throw new Error(`Unexpected banner mask size for ${id}`)
    }
    decodedMasks.set(id, decoded)
    return decoded
}
