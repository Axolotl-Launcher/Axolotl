/**
 * Formats the webview can encode again after an edit.
 *
 * The editor exports PNG, so the result has to be written back with the
 * extension the file already has. Animated GIF, vector SVG and AVIF/ICO/BMP are
 * left out: re-encoding would either drop the animation or produce bytes that do
 * not match the file's format.
 */
const EDITABLE_MEDIA_TYPES = new Set(['image/png', 'image/jpeg', 'image/webp'])

export function canEditImage(mimeType: string): boolean {
    return EDITABLE_MEDIA_TYPES.has(mimeType)
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
