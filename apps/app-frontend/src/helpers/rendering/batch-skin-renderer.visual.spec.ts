import { afterEach, expect, it, vi } from 'vitest'

import type { Skin } from '../skins'
import {
    cleanupUnusedPreviews,
    generateHeadRender,
    getHeadRenderKey,
    headBlobUrlMap,
    pruneSkinPreviewMemory,
    retainPlayerHeadTextures,
    setActiveSkinPreviews,
    skinBlobUrlMap,
} from './batch-skin-renderer'

vi.mock('../skins', () => ({ get_normalized_skin_texture: vi.fn() }))
vi.mock('../storage/head-storage', () => ({
    headStorage: {
        cleanupInvalidKeys: vi.fn(),
        retrieve: async () => 'blob:loaded-head',
    },
}))
vi.mock('../storage/skin-preview-storage', () => ({
    skinPreviewStorage: { cleanupInvalidKeys: vi.fn() },
}))

afterEach(() => {
    setActiveSkinPreviews([])
    vi.restoreAllMocks()
})

it('revokes removed skin previews and heads while retaining avatars owned by the account card', async () => {
    const revoke = vi.spyOn(URL, 'revokeObjectURL')
    const releaseAccount = retainPlayerHeadTextures(['account'])
    skinBlobUrlMap.set('old+CLASSIC+no-cape', { forwards: 'blob:old-skin' })
    skinBlobUrlMap.set('current+CLASSIC+no-cape', { forwards: 'blob:current-skin' })
    headBlobUrlMap.set(getHeadRenderKey('old'), 'blob:old-head')
    headBlobUrlMap.set(getHeadRenderKey('current'), 'blob:current-head')
    headBlobUrlMap.set(getHeadRenderKey('account'), 'blob:account-head')
    await cleanupUnusedPreviews([{ texture_key: 'current', variant: 'CLASSIC' } as Skin])
    expect(revoke).toHaveBeenCalledWith('blob:old-skin')
    expect(revoke).toHaveBeenCalledWith('blob:old-head')
    expect(revoke).not.toHaveBeenCalledWith('blob:account-head')
    expect(skinBlobUrlMap.size).toBe(1)

    setActiveSkinPreviews([])
    expect(skinBlobUrlMap.size).toBe(0)
    expect(headBlobUrlMap.size).toBe(1)
    releaseAccount()
    await Promise.resolve()
    expect(headBlobUrlMap.size).toBe(0)
    expect(revoke).toHaveBeenCalledWith('blob:account-head')
})

it('a new avatar owner can retain the same texture before the old owner releases it', async () => {
    const oldOwner = retainPlayerHeadTextures(['shared'])
    headBlobUrlMap.set(getHeadRenderKey('shared'), 'blob:shared')
    oldOwner()
    const newOwner = retainPlayerHeadTextures(['shared'])
    await Promise.resolve()
    pruneSkinPreviewMemory()
    expect(headBlobUrlMap.get(getHeadRenderKey('shared'))).toBe('blob:shared')
    newOwner()
    await Promise.resolve()
    expect(headBlobUrlMap.size).toBe(0)
})

it('retains asynchronously loaded account heads until their owner releases them', async () => {
    const release = retainPlayerHeadTextures(['loading'])
    const url = await generateHeadRender({ texture_key: 'loading' } as Skin)
    await Promise.resolve()
    expect(headBlobUrlMap.get(getHeadRenderKey('loading'))).toBe(url)
    release()
    await Promise.resolve()
    expect(headBlobUrlMap.has(getHeadRenderKey('loading'))).toBe(false)
})
