import { afterEach, expect, it, vi } from 'vitest'

import type { Cape, Skin } from '../skins'
import { skinPreviewStorage } from '../storage/skin-preview-storage'
import { getHeadRenderKey, headBlobUrlMap, skinBlobUrlMap } from './batch-skin-renderer'
import { cancelSkinPreviews, generateSkinPreviews } from './skin-preview-renderer'

vi.mock('../skins', () => ({
    determineModelType: async () => 'CLASSIC',
    get_normalized_skin_texture: async (skin: Skin) => {
        if (skin.texture_key === 'broken-skin') throw new Error('Skin unavailable')
        return skin.texture
    },
}))
vi.mock('../storage/head-storage', () => ({
    headStorage: {
        batchRetrieve: async () => ({}),
        cleanupInvalidKeys: vi.fn(),
        retrieve: async () => null,
        store: vi.fn(),
    },
}))
vi.mock('../storage/skin-preview-storage', () => ({
    skinPreviewStorage: {
        batchRetrieve: async () => ({}),
        cleanupInvalidKeys: vi.fn(),
        store: vi.fn(),
    },
}))

afterEach(() => {
    cancelSkinPreviews()
    vi.clearAllMocks()
})

function texture(width: number, height: number) {
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const ctx = canvas.getContext('2d')!
    ctx.fillStyle = '#38adff'
    ctx.fillRect(0, 0, width, height)
    return canvas.toDataURL('image/png')
}

async function hasVisiblePixels(url: string) {
    const bitmap = await createImageBitmap(await (await fetch(url)).blob())
    try {
        const canvas = document.createElement('canvas')
        canvas.width = bitmap.width
        canvas.height = bitmap.height
        const ctx = canvas.getContext('2d')!
        ctx.drawImage(bitmap, 0, 0)
        return ctx
            .getImageData(0, 0, canvas.width, canvas.height)
            .data.some((value, index) => index % 4 === 3 && value > 0)
    } finally {
        bitmap.close()
    }
}

it('keeps the player and later previews when a cape fails, then caches the recovered preview', async () => {
    const capedSkin: Skin = {
        texture_key: 'caped-skin',
        variant: 'CLASSIC',
        cape_id: 'cape',
        source: 'custom',
        texture: texture(64, 64),
        is_equipped: true,
    }
    const otherSkin: Skin = { ...capedSkin, texture_key: 'other-skin', cape_id: undefined }
    const brokenSkin: Skin = { ...otherSkin, texture_key: 'broken-skin' }
    const cape: Cape = {
        id: 'cape',
        name: 'Cape',
        texture: 'data:image/png;base64,aW52YWxpZA==',
        is_equipped: true,
    }
    const capedKey = 'caped-skin+CLASSIC+cape'
    const otherKey = 'other-skin+CLASSIC+no-cape'

    await generateSkinPreviews([capedSkin, brokenSkin, otherSkin], [cape])

    const fallback = skinBlobUrlMap.get(capedKey)?.forwards
    expect(fallback).toMatch(/^blob:/)
    expect(await hasVisiblePixels(fallback!)).toBe(true)
    expect(headBlobUrlMap.has(getHeadRenderKey(capedSkin.texture_key))).toBe(true)
    expect(skinBlobUrlMap.get(otherKey)?.forwards).toMatch(/^blob:/)
    expect(skinPreviewStorage.store).not.toHaveBeenCalledWith(capedKey, expect.anything())
    expect(skinPreviewStorage.store).toHaveBeenCalledWith(otherKey, expect.anything())

    await generateSkinPreviews([capedSkin, otherSkin], [{ ...cape, texture: texture(64, 32) }])

    expect(skinBlobUrlMap.get(capedKey)?.forwards).not.toBe(fallback)
    expect(skinPreviewStorage.store).toHaveBeenCalledWith(capedKey, expect.anything())
})
