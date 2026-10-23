import { shallowRef } from 'vue'

import { loadImage, type Skin } from './state'

export interface RenderResult {
    forwards: string
}

const rendered = shallowRef(new Map<string, RenderResult>())
const pending = new Map<string, Promise<RenderResult | undefined>>()

export function previewKey(skin: Skin) {
    return `${skin.texture_key}+${skin.variant}+${skin.cape_id ?? 'no-cape'}`
}

/**
 * The skins the preview ships (the app's default skins and the account's custom
 * skin) come with thumbnails rendered by the app's batch renderer settings
 * (`BatchSkinRenderer` in skin-preview-renderer.ts). Only a skin the visitor
 * uploads is drawn here in 2D, since the site cannot run that renderer itself.
 */
const SHIPPED_TEXTURE = /^\/launcher-preview\/skins\/(?:default\/)?([\w-]+)\.png$/

function shippedPreview(skin: Skin): RenderResult | undefined {
    const match = SHIPPED_TEXTURE.exec(skin.texture)
    return match ? { forwards: `/launcher-preview/skins/previews/${match[1]}.webp` } : undefined
}

export function getBakedSkinTextures(skin: Skin): RenderResult | undefined {
    return shippedPreview(skin) ?? rendered.value.get(previewKey(skin))
}

function face(
    sheet: CanvasRenderingContext2D,
    sx: number,
    sy: number,
    sw: number,
    sh: number,
    ox: number,
    oy: number,
    target: CanvasRenderingContext2D,
    scale: number,
) {
    target.drawImage(sheet.canvas, sx, sy, sw, sh, ox * scale, oy * scale, sw * scale, sh * scale)
}

/**
 * Lays the 64×64 sheet out as the player seen from the front, on a 16×32 grid.
 * The result is upscaled and returned as a PNG data URL.
 */
function renderFront(sheet: CanvasRenderingContext2D, slim: boolean) {
    const canvas = document.createElement('canvas')
    const scale = 16
    canvas.width = 16 * scale
    canvas.height = 32 * scale
    const context = canvas.getContext('2d')
    if (!context) return canvas.toDataURL('image/png')
    context.imageSmoothingEnabled = false

    const armWidth = slim ? 3 : 4

    // Legs (right at x=4..8, left at x=8..12), then the body, the arms and
    // finally the head, which overlaps the body.
    face(sheet, 0, 20, 4, 12, 4, 20, context, scale)
    face(sheet, 0, 36, 4, 12, 4, 20, context, scale)
    face(sheet, 16, 20, 4, 12, 8, 20, context, scale)
    face(sheet, 0, 52, 4, 12, 8, 20, context, scale)

    face(sheet, 20, 20, 8, 12, 4, 8, context, scale)
    face(sheet, 20, 36, 8, 12, 4, 8, context, scale)

    // Right arm on the viewer's left, left arm on the viewer's right.
    face(sheet, 44, 20, armWidth, 12, 4 - armWidth, 8, context, scale)
    face(sheet, 44, 36, armWidth, 12, 4 - armWidth, 8, context, scale)
    face(sheet, 36, 52, armWidth, 12, 12, 8, context, scale)
    face(sheet, 52, 52, armWidth, 12, 12, 8, context, scale)

    // Head: the front face plus the hat layer above it.
    face(sheet, 8, 8, 8, 8, 0, 0, context, scale)
    face(sheet, 40, 8, 8, 8, 0, 0, context, scale)

    return canvas.toDataURL('image/png')
}

export async function bakeSkinPreview(skin: Skin): Promise<RenderResult | undefined> {
    const shipped = shippedPreview(skin)
    if (shipped) return shipped
    const key = previewKey(skin)
    const cached = rendered.value.get(key)
    if (cached) return cached

    const existing = pending.get(key)
    if (existing) return existing

    const job = (async () => {
        try {
            const image = await loadImage(skin.texture)
            const sheetCanvas = document.createElement('canvas')
            sheetCanvas.width = 64
            sheetCanvas.height = 64
            const sheet = sheetCanvas.getContext('2d')
            if (!sheet) return undefined
            sheet.imageSmoothingEnabled = false
            sheet.drawImage(image, 0, 0)

            const result: RenderResult = {
                forwards: renderFront(sheet, skin.variant === 'SLIM'),
            }
            const next = new Map(rendered.value)
            next.set(key, result)
            rendered.value = next
            return result
        } catch {
            return undefined
        } finally {
            pending.delete(key)
        }
    })()

    pending.set(key, job)
    return job
}

/** Bakes every listed skin that has no render yet; resolves when all are done. */
export async function bakeSkinPreviews(skins: Skin[]) {
    await Promise.all(skins.map((skin) => bakeSkinPreview(skin)))
}
