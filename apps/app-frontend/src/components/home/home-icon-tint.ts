import { type MaybeRefOrGetter, ref, toValue, watch } from 'vue'

/**
 * The colour an instance icon is "about", for the launch card's glow.
 *
 * Only clearly coloured pixels vote: greys, beige, near-black and near-white
 * are skipped, so a low-saturation icon (a beige square with black letters)
 * has no tint at all and the card falls back to the accent colour instead of
 * a muddy brown. The result is normalised to a bright, mid-lightness colour,
 * so whatever the icon, the glow reads the same in both themes.
 */
export function pickIconTint(pixels: ArrayLike<number>): string | null {
    let red = 0
    let green = 0
    let blue = 0
    let weight = 0
    let opaque = 0
    let coloured = 0

    for (let index = 0; index + 3 < pixels.length; index += 4) {
        if (pixels[index + 3] < 128) continue
        opaque++
        const r = pixels[index] / 255
        const g = pixels[index + 1] / 255
        const b = pixels[index + 2] / 255
        const max = Math.max(r, g, b)
        const min = Math.min(r, g, b)
        const chroma = max - min
        const lightness = (max + min) / 2
        if (chroma < 0.22 || lightness < 0.12 || lightness > 0.92) continue
        coloured++
        const vote = chroma * chroma
        red += r * vote
        green += g * vote
        blue += b * vote
        weight += vote
    }

    // A few coloured pixels in a grey icon are an accent, not its colour.
    if (opaque === 0 || coloured / opaque < 0.12 || weight === 0) return null

    const [hue, saturation, lightness] = rgbToHsl(red / weight, green / weight, blue / weight)
    const s = Math.min(0.85, Math.max(0.5, saturation))
    const l = Math.min(0.62, Math.max(0.5, lightness))
    return `hsl(${Math.round(hue)} ${Math.round(s * 100)}% ${Math.round(l * 100)}%)`
}

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
    const max = Math.max(r, g, b)
    const min = Math.min(r, g, b)
    const lightness = (max + min) / 2
    const chroma = max - min
    if (chroma === 0) return [0, 0, lightness]
    const saturation = chroma / (1 - Math.abs(2 * lightness - 1))
    let hue: number
    if (max === r) hue = ((g - b) / chroma) % 6
    else if (max === g) hue = (b - r) / chroma + 2
    else hue = (r - g) / chroma + 4
    return [(hue * 60 + 360) % 360, saturation, lightness]
}

const SAMPLE_SIZE = 24
const tints = new Map<string, Promise<string | null>>()

/** Reads an icon at a small size and picks its tint; null when it cannot be read. */
export function loadIconTint(url: string): Promise<string | null> {
    let tint = tints.get(url)
    if (!tint) {
        tint = new Promise<string | null>((resolve) => {
            const image = new Image()
            // A cross-origin icon is only readable with CORS; without it the
            // canvas is tainted and the card keeps the accent colour.
            if (new URL(url, window.location.href).origin !== window.location.origin) {
                image.crossOrigin = 'anonymous'
            }
            image.onload = () => {
                try {
                    const canvas = document.createElement('canvas')
                    canvas.width = SAMPLE_SIZE
                    canvas.height = SAMPLE_SIZE
                    const context = canvas.getContext('2d', { willReadFrequently: true })
                    if (!context) return resolve(null)
                    context.drawImage(image, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE)
                    resolve(pickIconTint(context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE).data))
                } catch {
                    resolve(null)
                }
            }
            image.onerror = () => resolve(null)
            image.src = url
        })
        tints.set(url, tint)
    }
    return tint
}

/** The tint of the icon at `url`, null while loading or when the icon has none. */
export function useIconTint(url: MaybeRefOrGetter<string | null | undefined>) {
    const tint = ref<string | null>(null)
    watch(
        () => toValue(url),
        (current) => {
            tint.value = null
            if (!current || typeof Image === 'undefined') return
            void loadIconTint(current).then((value) => {
                if (toValue(url) === current) tint.value = value
            })
        },
        { immediate: true },
    )
    return tint
}
