import { expect, it } from 'vitest'

import { canEditImage, reencodeImageBytes } from './image-encoding'

function pngBytes(width: number, height: number): Uint8Array {
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('the test needs a 2D context')
    context.fillStyle = '#ff0000'
    context.fillRect(0, 0, width, height)
    return Uint8Array.from(atob(canvas.toDataURL('image/png').split(',')[1]), (character) =>
        character.charCodeAt(0),
    )
}

/**
 * Inserts a chunk after the PNG header, which is where an animation declaration
 * has to sit. The CRC is left as zero: the walker reads chunk types and lengths,
 * never the checksum.
 */
function pngWithChunk(type: string, data: number[]): Uint8Array {
    const png = pngBytes(2, 2)
    const headerLength = ((png[8] << 24) | (png[9] << 16) | (png[10] << 8) | png[11]) >>> 0
    const insertAt = 8 + 12 + headerLength

    const chunk = new Uint8Array(12 + data.length)
    chunk[0] = (data.length >>> 24) & 0xff
    chunk[1] = (data.length >>> 16) & 0xff
    chunk[2] = (data.length >>> 8) & 0xff
    chunk[3] = data.length & 0xff
    for (let index = 0; index < 4; index += 1) chunk[4 + index] = type.charCodeAt(index)
    chunk.set(data, 8)

    const result = new Uint8Array(png.byteLength + chunk.byteLength)
    result.set(png.subarray(0, insertAt), 0)
    result.set(chunk, insertAt)
    result.set(png.subarray(insertAt), insertAt + chunk.byteLength)
    return result
}

function webpBytes(chunks: Array<[type: string, data: number[]]>): Uint8Array {
    const parts = chunks.map(([type, data]) => {
        const chunk = new Uint8Array(8 + data.length + (data.length % 2))
        for (let index = 0; index < 4; index += 1) chunk[index] = type.charCodeAt(index)
        chunk[4] = data.length & 0xff
        chunk[5] = (data.length >>> 8) & 0xff
        chunk[6] = (data.length >>> 16) & 0xff
        chunk[7] = (data.length >>> 24) & 0xff
        chunk.set(data, 8)
        return chunk
    })
    const payload = parts.reduce((total, part) => total + part.byteLength, 0)
    const result = new Uint8Array(12 + payload)
    result.set([0x52, 0x49, 0x46, 0x46], 0)
    const riffSize = 4 + payload
    result[4] = riffSize & 0xff
    result[5] = (riffSize >>> 8) & 0xff
    result[6] = (riffSize >>> 16) & 0xff
    result[7] = (riffSize >>> 24) & 0xff
    result.set([0x57, 0x45, 0x42, 0x50], 8)
    let offset = 12
    for (const part of parts) {
        result.set(part, offset)
        offset += part.byteLength
    }
    return result
}

function signature(bytes: Uint8Array): number[] {
    return [...bytes.slice(0, 4)]
}

it('only offers editing to the formats it can write back', () => {
    const still = pngBytes(2, 2)

    expect(canEditImage('image/png', still)).toBe(true)
    expect(canEditImage('image/jpeg', still)).toBe(true)
    expect(canEditImage('image/webp', still)).toBe(true)
    // Re-encoding an animated GIF would drop the animation, and an SVG or an icon
    // would lose the vector data the file is made of.
    expect(canEditImage('image/gif', still)).toBe(false)
    expect(canEditImage('image/svg+xml', still)).toBe(false)
    expect(canEditImage('image/x-icon', still)).toBe(false)
    expect(canEditImage('', still)).toBe(false)
    // Without the bytes there is no way to tell whether the file is animated.
    expect(canEditImage('image/png', null)).toBe(false)
})

it('refuses an animated PNG even though its media type is editable', () => {
    // `acTL` is what an APNG adds to declare its frame count.
    const animated = pngWithChunk('acTL', [0, 0, 0, 2, 0, 0, 0, 0])

    expect(canEditImage('image/png', animated)).toBe(false)
})

it('refuses an animated WebP even though its media type is editable', () => {
    const animated = webpBytes([
        ['VP8X', [0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0]],
        ['ANIM', [0, 0, 0, 0, 0, 0]],
    ])

    expect(canEditImage('image/webp', animated)).toBe(false)
})

it('accepts a WebP container that carries no animation', () => {
    const still = webpBytes([['VP8 ', [0x00, 0x00, 0x00, 0x00]]])

    expect(canEditImage('image/webp', still)).toBe(true)
})

it('passes a PNG export through untouched', async () => {
    const bytes = pngBytes(2, 2)

    expect(await reencodeImageBytes(bytes, 'image/png')).toBe(bytes)
})

it('re-encodes the export into the format the file already has', async () => {
    const encoded = await reencodeImageBytes(pngBytes(4, 3), 'image/jpeg')

    // A JPEG starts with FFD8; a PNG signature here would mean the bytes were
    // written under a `.jpg` name and the file would no longer decode.
    expect(signature(encoded)).toEqual([0xff, 0xd8, 0xff, 0xe0])
    const bitmap = await createImageBitmap(new Blob([encoded as BlobPart], { type: 'image/jpeg' }))
    expect({ width: bitmap.width, height: bitmap.height }).toEqual({ width: 4, height: 3 })
})

it('refuses a format the webview cannot encode', async () => {
    await expect(reencodeImageBytes(pngBytes(2, 2), 'image/avif-not-real')).rejects.toThrow()
})
