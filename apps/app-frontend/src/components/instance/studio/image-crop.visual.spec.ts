import { expect, it } from 'vitest'

import { canCropImage, cropImageBytes } from './image-crop'

function solidImage(width: number, height: number, color: string): string {
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('the test needs a 2D context')
    context.fillStyle = color
    context.fillRect(0, 0, width, height)
    return canvas.toDataURL('image/png')
}

async function decodedSize(bytes: Uint8Array): Promise<{ width: number; height: number }> {
    const bitmap = await createImageBitmap(new Blob([bytes as BlobPart], { type: 'image/png' }))
    return { width: bitmap.width, height: bitmap.height }
}

it('writes the selected region at its natural pixel size', async () => {
    const url = solidImage(4, 4, '#ff0000')

    const bytes = await cropImageBytes(
        url,
        { left: 0, top: 0, width: 2, height: 2 },
        1,
        { width: 4, height: 4 },
        'image/png',
    )

    expect(await decodedSize(bytes)).toEqual({ width: 2, height: 2 })
})

it('maps a selection made on a zoomed preview back onto the source pixels', async () => {
    const url = solidImage(16, 16, '#00ff00')

    const bytes = await cropImageBytes(
        url,
        { left: 60, top: 0, width: 120, height: 120 },
        30,
        { width: 16, height: 16 },
        'image/png',
    )

    expect(await decodedSize(bytes)).toEqual({ width: 4, height: 4 })
})

it('keeps the pixels of the region instead of the whole image', async () => {
    const canvas = document.createElement('canvas')
    canvas.width = 2
    canvas.height = 1
    const context = canvas.getContext('2d')!
    context.fillStyle = '#ff0000'
    context.fillRect(0, 0, 1, 1)
    context.fillStyle = '#0000ff'
    context.fillRect(1, 0, 1, 1)
    const url = canvas.toDataURL('image/png')

    const bytes = await cropImageBytes(
        url,
        { left: 1, top: 0, width: 1, height: 1 },
        1,
        { width: 2, height: 1 },
        'image/png',
    )

    const bitmap = await createImageBitmap(new Blob([bytes as BlobPart], { type: 'image/png' }))
    const check = document.createElement('canvas')
    check.width = 1
    check.height = 1
    const checkContext = check.getContext('2d')!
    checkContext.drawImage(bitmap, 0, 0)
    const [red, , blue] = checkContext.getImageData(0, 0, 1, 1).data
    expect(red).toBe(0)
    expect(blue).toBe(255)
})

it('refuses a format the webview would silently re-encode', async () => {
    const url = solidImage(4, 4, '#ffffff')

    await expect(
        cropImageBytes(
            url,
            { left: 0, top: 0, width: 2, height: 2 },
            1,
            { width: 4, height: 4 },
            'image/gif',
        ),
    ).rejects.toThrow('cannot encode')
})

it('only offers cropping for formats it can write back', () => {
    expect(canCropImage('image/png')).toBe(true)
    expect(canCropImage('image/jpeg')).toBe(true)
    expect(canCropImage('image/gif')).toBe(false)
    expect(canCropImage('image/svg+xml')).toBe(false)
})
