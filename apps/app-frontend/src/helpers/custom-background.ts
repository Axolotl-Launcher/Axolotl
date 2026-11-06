import { canEditImage } from '@/components/instance/studio/image-encoding'

/** Static formats the launcher can turn into an encoded background. */
export const CUSTOM_BACKGROUND_EXTENSIONS = ['png', 'jpeg', 'jpg', 'webp', 'bmp'] as const

/** Stored backgrounds are loaded and decoded by the launcher on every start. */
export const MAX_CUSTOM_BACKGROUND_BYTES = 64 * 1024 * 1024

export type CustomBackgroundExtension = (typeof CUSTOM_BACKGROUND_EXTENSIONS)[number]

const MIME_BY_EXTENSION: Record<CustomBackgroundExtension, string> = {
    png: 'image/png',
    jpeg: 'image/jpeg',
    jpg: 'image/jpeg',
    webp: 'image/webp',
    bmp: 'image/bmp',
}

/**
 * The launcher background is stored as a static encoded image. Animated GIF,
 * AVIF, APNG and animated WebP stay out until the future video/animation crop
 * branch can preserve their frames.
 */
export function extensionFromPath(path: string): CustomBackgroundExtension | null {
    const extension = path.split('.').pop()?.toLowerCase() ?? ''
    return CUSTOM_BACKGROUND_EXTENSIONS.includes(extension as CustomBackgroundExtension)
        ? (extension as CustomBackgroundExtension)
        : null
}

export function isCustomBackgroundImagePath(path: string): boolean {
    return extensionFromPath(path) !== null
}

export function isCustomBackgroundWithinByteLimit(byteLength: number): boolean {
    return byteLength <= MAX_CUSTOM_BACKGROUND_BYTES
}

export function customBackgroundMimeType(extension: CustomBackgroundExtension): string {
    return MIME_BY_EXTENSION[extension]
}

export function isCroppableCustomBackground(mimeType: string, bytes: Uint8Array | null): boolean {
    if (mimeType === 'image/png' || mimeType === 'image/webp') {
        return canEditImage(mimeType, bytes)
    }

    return mimeType === 'image/jpeg' || mimeType === 'image/bmp'
}
