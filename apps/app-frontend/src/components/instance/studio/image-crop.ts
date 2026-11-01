import {
    type ImageRect,
    type PreviewRect,
    type PreviewSize,
    toImageRect,
} from './image-preview-geometry'

/**
 * Formats the webview can encode again after a crop.
 *
 * Cropping re-encodes the pixels, so the result has to be written back with the
 * extension the file already has. Animated GIF, vector SVG and AVIF/ICO/BMP are
 * left out: re-encoding would either drop the animation or produce bytes that do
 * not match the file's format.
 */
const CROPPABLE_MEDIA_TYPES = new Set(['image/png', 'image/jpeg', 'image/webp'])

export function canCropImage(mimeType: string): boolean {
    return CROPPABLE_MEDIA_TYPES.has(mimeType)
}

/**
 * Draws the selected region of the preview onto a canvas at its natural pixel
 * size and encodes it back to the file's own format.
 */
export async function cropImageBytes(
    url: string,
    selection: PreviewRect,
    scale: number,
    natural: PreviewSize,
    mimeType: string,
): Promise<Uint8Array> {
    const rect: ImageRect = toImageRect(selection, scale, natural)
    if (rect.width <= 0 || rect.height <= 0) {
        throw new Error('The crop selection is empty')
    }

    const image = await decodeImage(url)
    const canvas = document.createElement('canvas')
    canvas.width = rect.width
    canvas.height = rect.height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('The webview could not create a drawing surface')
    context.drawImage(image, rect.x, rect.y, rect.width, rect.height, 0, 0, rect.width, rect.height)

    const blob = await new Promise<Blob | null>((resolve) =>
        canvas.toBlob(resolve, mimeType, mimeType === 'image/jpeg' ? 0.92 : undefined),
    )
    if (!blob) throw new Error('The webview could not encode the cropped image')
    // A webview without an encoder for the requested format silently falls back
    // to PNG; writing those bytes under a `.webp` name would corrupt the file.
    if (blob.type !== mimeType) {
        throw new Error(`The webview cannot encode ${mimeType}`)
    }
    return new Uint8Array(await blob.arrayBuffer())
}

function decodeImage(url: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
        const image = new Image()
        image.onload = () => resolve(image)
        image.onerror = () => reject(new Error('The image could not be decoded'))
        image.src = url
    })
}
