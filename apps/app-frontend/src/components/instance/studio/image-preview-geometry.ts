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
