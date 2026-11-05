import type { Cape, Skin, SkinModel } from '@/helpers/skins'

import recorded from '../data/account.json'
import defaultSkins from '../data/default-skins.json'
import type { Handlers } from './types'

const textures = import.meta.glob<string>('../assets/skins/*.png', {
    eager: true,
    query: '?url',
    import: 'default',
})

const STEVE_CLASSIC = '31f477eb1a7beee631c2ca64d06f8f68fa93a3386d04452ab27f43acdf1b60cb'

export const recordedCape: Cape = {
    id: recorded.cape.texture_key,
    name: recorded.name,
    texture: textures[`../assets/skins/${recorded.cape.file}`],
    is_equipped: true,
}

export const recordedSkin: Skin = {
    texture_key: recorded.skin.texture_key,
    name: recorded.name,
    variant: recorded.skin.variant as SkinModel,
    texture: textures[`../assets/skins/${recorded.skin.file}`],
    cape_id: recordedCape.id,
    source: 'custom_external',
    is_equipped: true,
}

let skins: Skin[] = [
    recordedSkin,
    ...defaultSkins.map((skin): Skin => ({
        texture_key: skin.texture_key,
        name: skin.name,
        section: skin.section,
        variant: skin.variant as SkinModel,
        texture: textures[`../assets/skins/${skin.file}`],
        source: 'default',
        is_equipped: false,
    })),
]

let capes: Cape[] = [recordedCape]

export function useAccountAppearance(skin?: Skin, cape?: Cape) {
    skins = skins.filter((entry) => entry.source === 'default')
    if (skin) skins.unshift({ ...skin })
    capes = cape ? [{ ...cape }] : []
    equip(skin?.texture_key ?? STEVE_CLASSIC)
}

let uploads = 0

function equip(textureKey: string | null) {
    skins = skins.map((skin) => ({ ...skin, is_equipped: skin.texture_key === textureKey }))
    const equipped = skins.find((skin) => skin.is_equipped)
    capes = capes.map((cape) => ({ ...cape, is_equipped: cape.id === equipped?.cape_id }))
}

/** A skin the visitor chose: kept in this page only, as an object URL. */
function addCustomSkin(textureBlob: unknown, variant: unknown, cape: unknown): Skin {
    const skin: Skin = {
        texture_key: `local-upload-${++uploads}`,
        variant: variant as SkinModel,
        cape_id: (cape as Cape | undefined)?.id,
        texture: URL.createObjectURL(
            new Blob([new Uint8Array(textureBlob as ArrayLike<number>)], { type: 'image/png' }),
        ),
        source: 'custom',
        is_equipped: false,
    }
    skins = [skin, ...skins]
    return skin
}

/** Every texture the demo offers is already a 64×64 skin, so normalizing returns its bytes. */
async function textureBytes(texture: unknown) {
    if (typeof texture !== 'string') return Array.from(texture as ArrayLike<number>)
    const response = await fetch(texture)
    return Array.from(new Uint8Array(await response.arrayBuffer()))
}

export const skinHandlers: Handlers = {
    'plugin:minecraft-skins|get_available_skins': () => skins,
    'plugin:minecraft-skins|get_available_capes': () => capes,
    'plugin:minecraft-skins|equip_skin': ({ skin }) => equip((skin as Skin).texture_key),
    'plugin:minecraft-skins|unequip_skin': () => equip(STEVE_CLASSIC),
    'plugin:minecraft-skins|add_and_equip_custom_skin': ({ textureBlob, variant, cape }) => {
        const skin = addCustomSkin(textureBlob, variant, cape)
        equip(skin.texture_key)
        return { ...skin, is_equipped: true }
    },
    'plugin:minecraft-skins|save_custom_skin': ({ skin, textureBlob, variant, cape }) => {
        skins = skins.filter((entry) => entry.texture_key !== (skin as Skin).texture_key)
        return addCustomSkin(textureBlob, variant, cape)
    },
    'plugin:minecraft-skins|remove_custom_skin': ({ skin }) => {
        skins = skins.filter((entry) => entry.texture_key !== (skin as Skin).texture_key)
    },
    'plugin:minecraft-skins|set_custom_skin_order': ({ textureKeys }) => {
        const order = textureKeys as string[]
        const rank = (skin: Skin) =>
            skin.source === 'default' ? Infinity : order.indexOf(skin.texture_key)
        skins = [...skins].sort((a, b) => rank(a) - rank(b))
    },
    'plugin:minecraft-skins|normalize_skin_texture': ({ texture }) => textureBytes(texture),
    'plugin:minecraft-skins|flush_pending_skin_change': () => {},
    'plugin:minecraft-skins|flush_pending_skin_change_for_profile': () => {},
}
