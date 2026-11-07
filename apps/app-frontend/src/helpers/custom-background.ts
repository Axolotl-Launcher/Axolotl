import { canEditImage } from '@/components/instance/studio/image-encoding'

/** Formats offered by the background picker and recognized by the launcher. */
export const CUSTOM_BACKGROUND_EXTENSIONS = [
    'png',
    'jpeg',
    'jpg',
    'webp',
    'bmp',
    'gif',
    'apng',
    'avif',
    'mp4',
] as const

/** Stored backgrounds are loaded and decoded by the launcher on every start. */
export const MAX_CUSTOM_BACKGROUND_BYTES = 64 * 1024 * 1024

/** Upper bound when the user explicitly keeps the source resolution. */
export const MAX_PRESERVED_BACKGROUND_BYTES = 512 * 1024 * 1024

export type CustomBackgroundExtension = (typeof CUSTOM_BACKGROUND_EXTENSIONS)[number]

export type CustomBackgroundKind =
    'static' | 'animated-image' | 'animated-avif' | 'video' | 'unsupported'

const MIME_BY_EXTENSION: Record<CustomBackgroundExtension, string> = {
    png: 'image/png',
    jpeg: 'image/jpeg',
    jpg: 'image/jpeg',
    webp: 'image/webp',
    bmp: 'image/bmp',
    gif: 'image/gif',
    apng: 'image/apng',
    avif: 'image/avif',
    mp4: 'video/mp4',
}

const KNOWN_EXTENSIONS = new Set<string>(CUSTOM_BACKGROUND_EXTENSIONS)

export function extensionFromPath(path: string): CustomBackgroundExtension | null {
    const extension = path.split('.').pop()?.toLowerCase() ?? ''
    return KNOWN_EXTENSIONS.has(extension) ? (extension as CustomBackgroundExtension) : null
}

/**
 * Sniff the container instead of trusting the file name. The picker also
 * offers "All files", so a supported image/video may arrive with any (or no)
 * extension; this is the program-side validation pass.
 */
export function detectCustomBackgroundExtension(
    bytes: Uint8Array | null,
    fallback: CustomBackgroundExtension | null,
): CustomBackgroundExtension | null {
    if (!bytes || bytes.length === 0) return fallback

    if (startsWith(bytes, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) {
        return 'png'
    }
    if (startsWith(bytes, [0xff, 0xd8, 0xff])) return 'jpeg'
    if (startsWithAscii(bytes, 'GIF87a') || startsWithAscii(bytes, 'GIF89a')) return 'gif'
    if (startsWithAscii(bytes, 'BM')) return 'bmp'
    if (isRiffWebp(bytes)) return 'webp'
    if (isIsoBmff(bytes)) {
        return containsAscii(bytes, 'avif', 64) || containsAscii(bytes, 'avis', 64) ? 'avif' : 'mp4'
    }

    return fallback
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
    if (extension === 'gif' || extension === 'apng') return 'animated-image'
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

function startsWith(bytes: Uint8Array, signature: number[]): boolean {
    return signature.every((value, index) => bytes[index] === value)
}

function startsWithAscii(bytes: Uint8Array, signature: string): boolean {
    return ascii(bytes, 0, signature.length) === signature
}

function isRiffWebp(bytes: Uint8Array): boolean {
    return bytes.length >= 12 && ascii(bytes, 0, 4) === 'RIFF' && ascii(bytes, 8, 4) === 'WEBP'
}

function isIsoBmff(bytes: Uint8Array): boolean {
    return bytes.length >= 12 && ascii(bytes, 4, 4) === 'ftyp'
}

function containsAscii(bytes: Uint8Array, value: string, limit: number): boolean {
    const end = Math.min(bytes.length, limit)
    for (let offset = 0; offset + value.length <= end; offset += 1) {
        if (ascii(bytes, offset, value.length) === value) return true
    }
    return false
}

function ascii(bytes: Uint8Array, offset: number, length: number): string {
    let text = ''
    for (let index = 0; index < length; index += 1) {
        text += String.fromCharCode(bytes[offset + index] ?? 0)
    }
    return text
}
