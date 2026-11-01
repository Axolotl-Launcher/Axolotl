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

function signature(bytes: Uint8Array): number[] {
    return [...bytes.slice(0, 4)]
}

it('only offers editing to the formats it can write back', () => {
    expect(canEditImage('image/png')).toBe(true)
    expect(canEditImage('image/jpeg')).toBe(true)
    expect(canEditImage('image/webp')).toBe(true)
    // Re-encoding an animated GIF would drop the animation, and an SVG or an icon
    // would lose the vector data the file is made of.
    expect(canEditImage('image/gif')).toBe(false)
    expect(canEditImage('image/svg+xml')).toBe(false)
    expect(canEditImage('image/x-icon')).toBe(false)
    expect(canEditImage('')).toBe(false)
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
