import { canEditImage } from '@/components/instance/studio/image-encoding'

/** Formats offered by the background picker and the drag/drop filter. */
export const CUSTOM_BACKGROUND_EXTENSIONS = ['png', 'jpeg', 'jpg', 'webp', 'bmp', 'gif'] as const

/**
 * Formats the conversion backend recognizes but the UI does not offer until
 * the FFmpeg settings extension exists. Keeping them here lets the API and
 * error handling stay stable across that follow-up.
 */
export const CUSTOM_BACKGROUND_FUTURE_EXTENSIONS = ['avif', 'mp4'] as const

/** Stored backgrounds are loaded and decoded by the launcher on every start. */
export const MAX_CUSTOM_BACKGROUND_BYTES = 64 * 1024 * 1024

export type CustomBackgroundExtension =
    | (typeof CUSTOM_BACKGROUND_EXTENSIONS)[number]
    | (typeof CUSTOM_BACKGROUND_FUTURE_EXTENSIONS)[number]

export type CustomBackgroundKind =
    'static' | 'animated-image' | 'animated-avif' | 'video' | 'unsupported'

const MIME_BY_EXTENSION: Record<CustomBackgroundExtension, string> = {
    png: 'image/png',
    jpeg: 'image/jpeg',
    jpg: 'image/jpeg',
    webp: 'image/webp',
    bmp: 'image/bmp',
    gif: 'image/gif',
    avif: 'image/avif',
    mp4: 'video/mp4',
}

const KNOWN_EXTENSIONS = new Set<string>([
    ...CUSTOM_BACKGROUND_EXTENSIONS,
    ...CUSTOM_BACKGROUND_FUTURE_EXTENSIONS,
])

export function extensionFromPath(path: string): CustomBackgroundExtension | null {
    const extension = path.split('.').pop()?.toLowerCase() ?? ''
    return KNOWN_EXTENSIONS.has(extension) ? (extension as CustomBackgroundExtension) : null
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

export function classifyCustomBackground(
    extension: CustomBackgroundExtension,
    bytes: Uint8Array | null,
): CustomBackgroundKind {
    if (extension === 'gif') return 'animated-image'
    if (extension === 'avif') return 'animated-avif'
    if (extension === 'mp4') return 'video'

    if (extension === 'png' || extension === 'webp') {
        const mimeType = customBackgroundMimeType(extension)
        return canEditImage(mimeType, bytes) ? 'static' : 'animated-image'
    }

    return extension === 'jpeg' || extension === 'jpg' || extension === 'bmp'
        ? 'static'
        : 'unsupported'
}

export function isCroppableCustomBackground(mimeType: string, bytes: Uint8Array | null): boolean {
    if (mimeType === 'image/png' || mimeType === 'image/webp') {
        return canEditImage(mimeType, bytes)
    }

    return mimeType === 'image/jpeg' || mimeType === 'image/bmp'
}
