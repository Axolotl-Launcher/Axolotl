import type { Skin } from '@/helpers/skins'

/**
 * A player head cut from a skin texture, face plus hat layer, as an SVG data
 * URL. It needs no canvas, so it works before any skin render has finished.
 */
export function createSkinHeadDataUrl(textureUrl: string) {
    const escapedTextureUrl = textureUrl
        .replaceAll('&', '&amp;')
        .replaceAll('"', '&quot;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 8 8" shape-rendering="crispEdges"><image href="${escapedTextureUrl}" x="-8" y="-8" width="64" height="64" style="image-rendering:pixelated"/><image href="${escapedTextureUrl}" x="-40" y="-8" width="64" height="64" style="image-rendering:pixelated"/></svg>`

    return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
}

/** An account as far as its skin goes: offline accounts have none. */
export type SkinnedAccount = {
    account_type: string
    profile: {
        id: string
        skins?: Array<{
            state: string
            url: string
            variant?: Skin['variant']
            textureKey?: string
        }>
    }
}

/**
 * The skin a signed-in account wears, in the form the head renderer takes. A remote
 * texture cannot be drawn through `createSkinHeadDataUrl`, since an SVG shown as an
 * image loads nothing external, so online heads go through `getPlayerHeadUrl`.
 */
export function getAccountSkin(account: SkinnedAccount | undefined): Skin | undefined {
    if (!account || account.account_type === 'offline') return undefined
    const skin =
        account.profile.skins?.find((skin) => skin.state === 'ACTIVE') ?? account.profile.skins?.[0]
    if (!skin?.url) return undefined

    return {
        texture_key: skin.textureKey ?? `${account.profile.id}:${skin.url}`,
        variant: skin.variant ?? 'UNKNOWN',
        texture: skin.url,
        source: 'custom_external',
        is_equipped: true,
    }
}
