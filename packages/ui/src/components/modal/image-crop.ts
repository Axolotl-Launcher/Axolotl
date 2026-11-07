export interface CropSize {
    width: number
    height: number
}

export interface CropRect extends CropSize {
    left: number
    top: number
}

export interface CropView {
    zoom: number
    centerX: number
    centerY: number
}

export interface ImageCropAspectRatioOption {
    id: string
    label: string
    ratio: number
}

export interface ImageCropShowOptions {
    aspectRatioOptions?: ImageCropAspectRatioOption[]
    defaultAspectRatioId?: string
}

export interface ImageCropResult {
    blob: Blob
    bytes: Uint8Array
    width: number
    height: number
    mimeType: 'image/png' | 'image/webp'
    extension: 'png' | 'webp'
    /** The user kept the source resolution/quality instead of auto-compressing. */
    preserveOriginalQuality?: boolean
}

export interface NormalizedCropRect {
    left: number
    top: number
    width: number
    height: number
}

export interface CropSelection {
    sourceWidth: number
    sourceHeight: number
    crop: { left: number; top: number; width: number; height: number }
    normalizedCrop: NormalizedCropRect
    aspectRatio: number
    outputWidth: number
    outputHeight: number
    /** The user explicitly chose to proceed past the source size/pixel guard. */
    oversizedSource: boolean
    /** Keep the source resolution/quality instead of auto-compressing. */
    preserveOriginalQuality: boolean
}

export interface ImageCropSelectionOptions extends ImageCropShowOptions {
    /** Overrides the component's maxOutputDimension for this session. */
    maxOutputDimension?: number
    /** Show the source animation in the result preview instead of a still. */
    animated?: boolean
}

export type ImageCropOutcome = ImageCropResult | CropSelection

export interface ImageCropModalExposed {
    show(source: Blob, options?: ImageCropShowOptions): Promise<ImageCropResult | null>
    showCrop(source: Blob, options?: ImageCropSelectionOptions): Promise<CropSelection | null>
    hide(): void
}

export const MIN_CROP_ZOOM = 1
export const MAX_CROP_ZOOM = 8
export const DEFAULT_CROP_ASPECT_RATIO = 16 / 9
export const DEFAULT_CROP_FRAME_PADDING = 24

export function normalizeAspectRatio(value: number, fallback = DEFAULT_CROP_ASPECT_RATIO): number {
    return Number.isFinite(value) && value > 0 ? value : fallback
}

export function fitCropFrame(
    containerWidth: number,
    containerHeight: number,
    aspectRatio: number,
    padding = DEFAULT_CROP_FRAME_PADDING,
): CropRect {
    const width = Math.max(1, Number.isFinite(containerWidth) ? containerWidth : 1)
    const height = Math.max(1, Number.isFinite(containerHeight) ? containerHeight : 1)
    const maxPadding = Math.max(0, Math.min((width - 1) / 2, (height - 1) / 2))
    const safePadding = Math.max(0, Math.min(padding, maxPadding))
    const availableWidth = Math.max(1, width - safePadding * 2)
    const availableHeight = Math.max(1, height - safePadding * 2)
    const ratio = normalizeAspectRatio(aspectRatio)
    const frameWidth = Math.min(availableWidth, availableHeight * ratio)
    const frameHeight = frameWidth / ratio

    return {
        left: (width - frameWidth) / 2,
        top: (height - frameHeight) / 2,
        width: frameWidth,
        height: frameHeight,
    }
}

export function coverScale(image: CropSize, frame: CropSize): number {
    if (
        !Number.isFinite(image.width) ||
        !Number.isFinite(image.height) ||
        !Number.isFinite(frame.width) ||
        !Number.isFinite(frame.height) ||
        image.width <= 0 ||
        image.height <= 0 ||
        frame.width <= 0 ||
        frame.height <= 0
    ) {
        return 1
    }

    return Math.max(frame.width / image.width, frame.height / image.height)
}

export function resetView(image: CropSize): CropView {
    return {
        zoom: MIN_CROP_ZOOM,
        centerX: Math.max(1, image.width) / 2,
        centerY: Math.max(1, image.height) / 2,
    }
}

export function clampCropView(view: CropView, image: CropSize, frame: CropSize): CropView {
    const zoom = clamp(view.zoom, MIN_CROP_ZOOM, MAX_CROP_ZOOM)
    const scale = coverScale(image, frame) * zoom
    const sourceWidth = frame.width / scale
    const sourceHeight = frame.height / scale

    return {
        zoom,
        centerX: clampCenter(view.centerX, sourceWidth, image.width),
        centerY: clampCenter(view.centerY, sourceHeight, image.height),
    }
}

export function sourceRect(view: CropView, image: CropSize, frame: CropSize): CropRect {
    const clamped = clampCropView(view, image, frame)
    const scale = coverScale(image, frame) * clamped.zoom
    const width = Math.min(image.width, frame.width / scale)
    const height = Math.min(image.height, frame.height / scale)

    return {
        left: clamp(clamped.centerX - width / 2, 0, Math.max(0, image.width - width)),
        top: clamp(clamped.centerY - height / 2, 0, Math.max(0, image.height - height)),
        width,
        height,
    }
}

export function panView(
    view: CropView,
    dx: number,
    dy: number,
    image: CropSize,
    frame: CropSize,
): CropView {
    const clamped = clampCropView(view, image, frame)
    const scale = coverScale(image, frame) * clamped.zoom

    return clampCropView(
        {
            zoom: clamped.zoom,
            centerX: clamped.centerX - dx / scale,
            centerY: clamped.centerY - dy / scale,
        },
        image,
        frame,
    )
}

export function zoomViewAt(
    view: CropView,
    nextZoom: number,
    focalOffsetX: number,
    focalOffsetY: number,
    image: CropSize,
    frame: CropSize,
): CropView {
    const clamped = clampCropView(view, image, frame)
    const zoom = clamp(nextZoom, MIN_CROP_ZOOM, MAX_CROP_ZOOM)
    const oldScale = coverScale(image, frame) * clamped.zoom
    const nextScale = coverScale(image, frame) * zoom
    const anchorX = clamped.centerX + focalOffsetX / oldScale
    const anchorY = clamped.centerY + focalOffsetY / oldScale

    return clampCropView(
        {
            zoom,
            centerX: anchorX - focalOffsetX / nextScale,
            centerY: anchorY - focalOffsetY / nextScale,
        },
        image,
        frame,
    )
}

export function outputSizeForSource(source: CropSize, maxOutputDimension: number): CropSize {
    const width = Math.max(1, source.width)
    const height = Math.max(1, source.height)
    const edge = Math.max(width, height)
    const limit =
        Number.isFinite(maxOutputDimension) && maxOutputDimension > 0 ? maxOutputDimension : edge
    const ratio = Math.min(1, limit / edge)

    return {
        width: Math.max(1, Math.round(width * ratio)),
        height: Math.max(1, Math.round(height * ratio)),
    }
}

export function extensionForMimeType(mimeType: string): 'png' | 'webp' | null {
    const normalized = mimeType.trim().toLowerCase()
    if (normalized === 'image/png') return 'png'
    if (normalized === 'image/webp') return 'webp'
    return null
}

function clampCenter(value: number, sourceSize: number, imageSize: number): number {
    if (sourceSize >= imageSize) return imageSize / 2
    return clamp(value, sourceSize / 2, imageSize - sourceSize / 2)
}

function clamp(value: number, min: number, max: number): number {
    if (!Number.isFinite(value)) return (min + max) / 2
    return Math.min(Math.max(value, min), max)
}
