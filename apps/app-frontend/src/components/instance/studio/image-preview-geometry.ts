export interface PreviewSize {
    width: number
    height: number
}

/** Matches the range the zoom controls allow, so a wheel gesture cannot drift
 * below a few pixels or past a scale that would stall the webview's compositor. */
export const MIN_PREVIEW_SCALE = 0.05
export const MAX_PREVIEW_SCALE = 32

export function clampPreviewScale(scale: number): number {
    if (!Number.isFinite(scale)) return 1
    return Math.min(MAX_PREVIEW_SCALE, Math.max(MIN_PREVIEW_SCALE, scale))
}

/**
 * Scale that shows the whole image inside the stage.
 *
 * A small texture is scaled up rather than left at its pixel size: Minecraft
 * textures are 16×16 or 32×32, and a preview that renders them at 16 px tells
 * the user nothing. The upper bound keeps a 1×1 image from filling the pane.
 */
export function fitPreviewScale(natural: PreviewSize, stage: PreviewSize): number {
    if (natural.width <= 0 || natural.height <= 0) return 1
    if (stage.width <= 0 || stage.height <= 0) return 1
    return clampPreviewScale(Math.min(stage.width / natural.width, stage.height / natural.height))
}

export interface PreviewRect {
    left: number
    top: number
    width: number
    height: number
}

export interface ImageRect {
    x: number
    y: number
    width: number
    height: number
}

/**
 * Converts a selection drawn over the displayed image into pixel coordinates.
 *
 * The selection is measured in CSS pixels, so it has to be divided by the scale
 * the image is drawn at. Rounding to whole pixels keeps the crop from sampling a
 * fraction of a texel, and clamping keeps a drag that ends outside the image
 * from asking the canvas for pixels that do not exist.
 */
export function toImageRect(
    selection: PreviewRect,
    scale: number,
    natural: PreviewSize,
): ImageRect {
    const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1
    if (natural.width <= 0 || natural.height <= 0) {
        return { x: 0, y: 0, width: 0, height: 0 }
    }

    const x = clamp(Math.round(selection.left / safeScale), 0, natural.width - 1)
    const y = clamp(Math.round(selection.top / safeScale), 0, natural.height - 1)
    return {
        x,
        y,
        width: clamp(Math.round(selection.width / safeScale), 1, natural.width - x),
        height: clamp(Math.round(selection.height / safeScale), 1, natural.height - y),
    }
}

function clamp(value: number, minimum: number, maximum: number): number {
    return Math.min(maximum, Math.max(minimum, value))
}
