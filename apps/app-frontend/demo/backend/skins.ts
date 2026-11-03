import herobrine from '@/assets/skins/herobrine.png'
import type { Cape, Skin, SkinModel } from '@/helpers/skins'

import defaultSkins from '../data/default-skins.json'
import type { Handlers } from './types'

const textures = import.meta.glob<string>('../assets/skins/*.png', {
    eager: true,
    query: '?url',
    import: 'default',
})

/** The account named Steve wears the default Steve skin and has saved the app's Herobrine. */
const STEVE_CLASSIC = '31f477eb1a7beee631c2ca64d06f8f68fa93a3386d04452ab27f43acdf1b60cb'

let skins: Skin[] = [
    {
        texture_key: 'local-herobrine',
        name: 'Herobrine',
        variant: 'CLASSIC',
        texture: herobrine,
        source: 'custom',
        is_equipped: false,
    },
    ...defaultSkins.map((skin): Skin => ({
        texture_key: skin.texture_key,
        name: skin.name,
        section: skin.section,
        variant: skin.variant as SkinModel,
        texture: textures[`../assets/skins/${skin.file}`],
        source: 'default',
        is_equipped: skin.texture_key === STEVE_CLASSIC,
    })),
]

let uploads = 0

function equip(textureKey: string | null) {
    skins = skins.map((skin) => ({ ...skin, is_equipped: skin.texture_key === textureKey }))
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
    'plugin:minecraft-skins|get_available_capes': (): Cape[] => [],
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
