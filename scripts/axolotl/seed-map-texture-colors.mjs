export function srgbToLinear(value) {
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
}

/** Alpha-weighted linear RGB, including the visible duration of animated frames. */
export function averageTextureColor(image, animation) {
    const sums = [0, 0, 0]
    let weight = 0
    let frames = [{ x: 0, y: 0, width: image.width, height: image.height, time: 1 }]
    if (animation) {
        const width = animation.width ?? Math.min(image.width, image.height)
        const height = animation.height ?? Math.min(image.width, image.height)
        if (width <= 0 || height <= 0 || image.width % width || image.height % height)
            throw new Error('Invalid animated texture dimensions')
        const columns = image.width / width
        const count = (columns * image.height) / height
        frames = (animation.frames ?? Array.from({ length: count }, (_, index) => index)).map(
            (frame) => {
                const index = typeof frame === 'number' ? frame : frame.index
                const time =
                    (typeof frame === 'number' ? undefined : frame.time) ?? animation.frametime ?? 1
                if (!Number.isInteger(index) || index < 0 || index >= count || time <= 0)
                    throw new Error('Invalid animated texture frame')
                return {
                    x: (index % columns) * width,
                    y: Math.floor(index / columns) * height,
                    width,
                    height,
                    time,
                }
            },
        )
    }
    for (const frame of frames) {
        for (let y = frame.y; y < frame.y + frame.height; y++) {
            for (let x = frame.x; x < frame.x + frame.width; x++) {
                const offset = (y * image.width + x) * 4
                const alpha = (image.data[offset + 3] / 255) * frame.time
                weight += alpha
                for (let channel = 0; channel < 3; channel++)
                    sums[channel] += srgbToLinear(image.data[offset + channel] / 255) * alpha
            }
        }
    }
    return weight ? sums.map((sum) => Number((sum / weight).toFixed(6))) : null
}
