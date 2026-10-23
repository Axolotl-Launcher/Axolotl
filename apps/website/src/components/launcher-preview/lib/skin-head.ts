import { ref } from 'vue'

const heads = ref(new Map<string, string>())
const pending = new Set<string>()

/**
 * Port of the app's `generatePlayerHeadBlob` (apps/app-frontend/src/helpers/
 * rendering/batch-skin-renderer.ts): the 8×8 face scaled to 64×64, framed by
 * the hat layer at 72×72 when the skin has one.
 */
function renderHead(texture: string) {
    if (heads.value.has(texture) || pending.has(texture)) return
    pending.add(texture)
    const image = new Image()
    image.onload = () => {
        pending.delete(texture)
        if (image.width !== 64 || image.height !== 64) return

        const outerLayer = document.createElement('canvas')
        outerLayer.width = 8
        outerLayer.height = 8
        const outerContext = outerLayer.getContext('2d')
        if (!outerContext) return
        outerContext.drawImage(image, 40, 8, 8, 8, 0, 0, 8, 8)
        const hasOuterLayer = outerContext
            .getImageData(0, 0, 8, 8)
            .data.some((channel, index) => index % 4 === 3 && channel > 0)

        const size = hasOuterLayer ? 72 : 64
        const offset = hasOuterLayer ? 4 : 0
        const output = document.createElement('canvas')
        output.width = size
        output.height = size
        const context = output.getContext('2d')
        if (!context) return
        context.imageSmoothingEnabled = false
        context.drawImage(image, 8, 8, 8, 8, offset, offset, 64, 64)
        if (hasOuterLayer) context.drawImage(outerLayer, 0, 0, 8, 8, 0, 0, size, size)

        heads.value = new Map(heads.value).set(texture, output.toDataURL('image/png'))
    }
    image.onerror = () => pending.delete(texture)
    image.src = texture
}

/** The rendered head of a skin texture; `null` until the texture has loaded. */
export function playerHeadUrl(texture: string): string | null {
    const url = heads.value.get(texture)
    if (url) return url
    renderHead(texture)
    return null
}
