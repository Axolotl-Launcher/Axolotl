/**
 * Formats the webview can encode again after an edit.
 *
 * The editor exports PNG, so the result has to be written back with the
 * extension the file already has. Animated GIF, vector SVG and AVIF/ICO/BMP are
 * left out: re-encoding would either drop the animation or produce bytes that do
 * not match the file's format.
 */
const EDITABLE_MEDIA_TYPES = new Set(['image/png', 'image/jpeg', 'image/webp'])

/**
 * Whether the editor may open this file.
 *
 * The editor exports a single still frame, so a file that is a sequence of frames
 * has to stay out of it: saving would replace the animation with whichever frame
 * happened to be on the canvas. PNG and WebP carry their animation inside the
 * container rather than in the media type, so the bytes decide, not the name.
 */
export function canEditImage(mimeType: string, bytes: Uint8Array | null): boolean {
    if (!bytes || !EDITABLE_MEDIA_TYPES.has(mimeType)) return false
    return !isAnimatedImage(bytes, mimeType)
}

/**
 * Re-encodes the editor's PNG export into the format the file already has.
 *
 * PNG is passed through untouched: the editor already produced that format, and
 * a decode/encode round trip would only risk changing the pixels.
 */
export async function reencodeImageBytes(bytes: Uint8Array, mimeType: string): Promise<Uint8Array> {
    if (!mimeType || mimeType === 'image/png') return bytes

    // The editor's export is a plain buffer; the DOM types reject a view that may
    // be backed by a shared buffer.
    const bitmap = await createImageBitmap(new Blob([bytes as BlobPart], { type: 'image/png' }))
    try {
        const canvas = document.createElement('canvas')
        canvas.width = bitmap.width
        canvas.height = bitmap.height
        const context = canvas.getContext('2d')
        if (!context) throw new Error('The webview could not create a drawing surface')
        context.drawImage(bitmap, 0, 0)

        const blob = await new Promise<Blob | null>((resolve) =>
            canvas.toBlob(resolve, mimeType, mimeType === 'image/jpeg' ? 0.92 : undefined),
        )
        if (!blob) throw new Error('The webview could not encode the edited image')
        // A webview without an encoder for the requested format silently falls
        // back to PNG; writing those bytes under a `.webp` name would corrupt the
        // file.
        if (blob.type !== mimeType) {
            throw new Error(`The webview cannot encode ${mimeType}`)
        }
        return new Uint8Array(await blob.arrayBuffer())
    } finally {
        bitmap.close()
    }
}

function isAnimatedImage(bytes: Uint8Array, mimeType: string): boolean {
    if (mimeType === 'image/png') return pngDeclaresAnimation(bytes)
    if (mimeType === 'image/webp') return webpDeclaresAnimation(bytes)
    return false
}

function readAscii(bytes: Uint8Array, offset: number, length: number): string {
    let text = ''
    for (let index = 0; index < length; index += 1) {
        text += String.fromCharCode(bytes[offset + index])
    }
    return text
}

function readUint32BigEndian(bytes: Uint8Array, offset: number): number {
    return (
        ((bytes[offset] << 24) |
            (bytes[offset + 1] << 16) |
            (bytes[offset + 2] << 8) |
            bytes[offset + 3]) >>>
        0
    )
}

function readUint32LittleEndian(bytes: Uint8Array, offset: number): number {
    return (
        ((bytes[offset + 3] << 24) |
            (bytes[offset + 2] << 16) |
            (bytes[offset + 1] << 8) |
            bytes[offset]) >>>
        0
    )
}

/**
 * An APNG announces its frame count in an `acTL` chunk that has to precede the
 * first `IDAT`; walking the chunk list stops at the pixels rather than scanning
 * the whole file for four bytes that could occur inside image data.
 */
function pngDeclaresAnimation(bytes: Uint8Array): boolean {
    if (bytes.byteLength < 8) return false
    let offset = 8
    while (offset + 8 <= bytes.byteLength) {
        const length = readUint32BigEndian(bytes, offset)
        const type = readAscii(bytes, offset + 4, 4)
        if (type === 'acTL') return true
        if (type === 'IDAT' || type === 'IEND') return false
        // Length, type, data and CRC; a malformed length would otherwise walk
        // past the end of the buffer.
        if (offset + 12 + length > bytes.byteLength) return false
        offset += 12 + length
    }
    return false
}

/**
 * An animated WebP always carries an `ANIM` chunk in its RIFF container, whether
 * the frames are stored as a single animated stream or as separate `ANMF` chunks.
 */
function webpDeclaresAnimation(bytes: Uint8Array): boolean {
    if (bytes.byteLength < 12) return false
    if (readAscii(bytes, 0, 4) !== 'RIFF' || readAscii(bytes, 8, 4) !== 'WEBP') return false
    let offset = 12
    while (offset + 8 <= bytes.byteLength) {
        const type = readAscii(bytes, offset, 4)
        const length = readUint32LittleEndian(bytes, offset + 4)
        if (type === 'ANIM') return true
        // RIFF chunks are padded to an even length.
        offset += 8 + length + (length % 2)
    }
    return false
}
