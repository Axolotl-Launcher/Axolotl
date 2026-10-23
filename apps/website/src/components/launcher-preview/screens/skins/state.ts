import {
    ARMOR_SLOTS,
    type ArmorPreviewConfig,
    cloneArmorPreviewConfig,
    createDefaultArmorPreviewConfig,
} from '@modrinth/ui/src/composables/skin-rendering/armor-preview-types.ts'
import { computed, type Ref, ref } from 'vue'

import defaultSkinSnapshot from '../../data/default-skins.json'
import { type PreviewState, usePreviewDomain } from '../../lib/state'
import type { SkinEntry } from '../../lib/state/accounts'

export type SkinModel = 'CLASSIC' | 'SLIM' | 'UNKNOWN'
export type SkinSource = 'default' | 'custom' | 'custom_external'

/**
 * The app's `Skin` (apps/app-frontend/src/helpers/skins.ts), with the preview's
 * account entry adapted into its shape.
 */
export interface Skin {
    texture_key: string
    name?: string
    section?: string
    variant: SkinModel
    cape_id?: string
    texture: string
    source: SkinSource
    is_equipped: boolean
}

export interface Cape {
    id: string
    name: string
    texture: string
    is_equipped: boolean
}

export interface DefaultSkinSection {
    section: string
    title: string
    skins: Skin[]
}

/** The app lists 'Default skins' first and keeps the other sections in list order. */
const DEFAULT_SKIN_SECTION_SORT_ORDER = ['Default skins']

/**
 * The skin list of `get_available_skins` for the selected offline account: the
 * account's own skins, then the app's bundled defaults (`data/default-skins.json`,
 * from app-lib's DEFAULT_SKINS, in order). A default skin is equipped when no
 * custom skin is and it is the account's current skin.
 */
export function buildSkinList(entries: SkinEntry[]): Skin[] {
    const custom = entries.filter((entry) => entry.source === 'custom').map(toSkin)
    const equipped = entries.find((entry) => entry.is_equipped)
    let foundEquipped = custom.some((skin) => skin.is_equipped)
    const defaults = defaultSkinSnapshot.skins.map((entry): Skin => {
        const isEquipped =
            !foundEquipped &&
            !!equipped &&
            entry.texture_key === equipped.texture_key &&
            entry.variant === equipped.variant
        foundEquipped ||= isEquipped
        return {
            texture_key: entry.texture_key,
            name: entry.name,
            section: entry.section,
            variant: entry.variant as SkinModel,
            texture: entry.texture,
            source: 'default',
            is_equipped: isEquipped,
        }
    })
    return [...custom, ...defaults]
}

export function toSkin(entry: SkinEntry): Skin {
    return {
        texture_key: entry.texture_key,
        name: entry.name,
        section: entry.section ?? undefined,
        variant: entry.variant,
        cape_id: entry.cape_id ?? undefined,
        texture: entry.texture,
        source: entry.source,
        is_equipped: entry.is_equipped,
    }
}

export function skinsMatch(a?: Skin | null, b?: Skin | null) {
    return (
        a?.source === b?.source &&
        a?.texture_key === b?.texture_key &&
        a?.variant === b?.variant &&
        (a?.cape_id ?? null) === (b?.cape_id ?? null)
    )
}

export function filterSavedSkins(list: Skin[]) {
    return list.filter((skin) => skin.source !== 'default')
}

export function filterDefaultSkins(list: Skin[]) {
    return list.filter((skin) => skin.source === 'default')
}

export function sectionSortIndex(section: string) {
    const index = DEFAULT_SKIN_SECTION_SORT_ORDER.indexOf(section)
    return index === -1 ? DEFAULT_SKIN_SECTION_SORT_ORDER.length : index
}

export function defaultSectionTitleKey(section: string) {
    switch (section) {
        case 'Modrinth':
            return 'modrinthSection'
        case 'MINECON Earth 2017':
            return 'mineconEarth2017Section'
        case 'Builders & Biomes':
            return 'buildersAndBiomesSection'
        case 'Striding Hero':
            return 'stridingHeroSection'
        case 'The Garden Awakens':
            return 'theGardenAwakensSection'
        case 'Chase the Skies':
            return 'chaseTheSkiesSection'
        case 'The Copper Age':
            return 'theCopperAgeSection'
        case 'Mounts of Mayhem':
            return 'mountsOfMayhemSection'
        case 'Tiny Takeover':
            return 'tinyTakeoverSection'
        case 'Chaos Cubed':
            return 'chaosCubedSection'
        case 'Default skins':
            return 'defaultSkinsSection'
        default:
            return null
    }
}

/**
 * The app's arm-pixel rule (`determineModelType`): the 2×12 column at (54, 20)
 * of the arm's outer face is transparent in a slim texture and opaque in a wide
 * one. PNGs that have no such column at all (legacy 64×32) are wide.
 */
export async function determineModelType(texture: string): Promise<'SLIM' | 'CLASSIC'> {
    const image = await loadImage(texture).catch(() => null)
    if (!image) return 'CLASSIC'

    const canvas = document.createElement('canvas')
    canvas.width = image.width
    canvas.height = image.height
    const context = canvas.getContext('2d')
    if (!context) return 'CLASSIC'
    context.drawImage(image, 0, 0)
    canvas.remove?.()

    const armX = 54
    const armY = 20
    const armWidth = 2
    const armHeight = 12

    if (image.width < armX + armWidth || image.height < armY + armHeight) {
        // A legacy 64×32 sheet stops short of the modern arm columns.
        return 'CLASSIC'
    }

    const imageData = context.getImageData(armX, armY, armWidth, armHeight).data
    for (let alphaIndex = 3; alphaIndex < imageData.length; alphaIndex += 4) {
        if (imageData[alphaIndex] !== 0) return 'CLASSIC'
    }

    return 'SLIM'
}

export function loadImage(src: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
        const image = new Image()
        image.crossOrigin = 'anonymous'
        image.onload = () => resolve(image)
        image.onerror = () => reject(new Error('Failed to load the image.'))
        image.src = src
    })
}

/** Reads a picked or dropped PNG into a data URL, refusing anything that is not a PNG. */
export function readSkinFile(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
        if (file.type !== 'image/png' && !file.name.toLowerCase().endsWith('.png')) {
            reject(new Error('Skin textures must be PNG files.'))
            return
        }
        const reader = new FileReader()
        reader.onload = () => resolve(String(reader.result))
        reader.onerror = () => reject(new Error('Failed to read the dropped file.'))
        reader.readAsDataURL(file)
    })
}

/**
 * The app's `normalize_skin_texture` (a Rust command): every texture the
 * launcher stores is a 64×64 sheet. A legacy 64×32 texture is upscaled into the
 * modern layout, with the arm and leg sheets copied into the second layer.
 * Returns a PNG data URL.
 */
export async function normalizeSkinTexture(
    texture: string,
    preserveInnerAlpha = false,
): Promise<string> {
    const image = await loadImage(texture)
    if (image.width === 64 && image.height === 64) {
        if (preserveInnerAlpha) return texture
        return flattenAlpha(image)
    }

    const canvas = document.createElement('canvas')
    canvas.width = 64
    canvas.height = 64
    const context = canvas.getContext('2d')
    if (!context) return texture
    context.imageSmoothingEnabled = false
    context.drawImage(image, 0, 0)

    if (image.width === 64 && image.height === 32) {
        // Right leg, right arm and their second layers move to the modern slots.
        context.drawImage(image, 0, 16, 16, 16, 16, 48, 16, 16)
        context.drawImage(image, 40, 16, 16, 16, 40, 48, 16, 16)
        // Copy the base sheet into the overlay layer so `preserveInnerAlpha`
        // works the way the launcher's textures do.
        context.drawImage(image, 0, 0, 64, 32, 0, 0, 64, 32)
    }

    return canvas.toDataURL('image/png')
}

function flattenAlpha(image: HTMLImageElement) {
    const canvas = document.createElement('canvas')
    canvas.width = 64
    canvas.height = 64
    const context = canvas.getContext('2d')
    if (!context) return image.src
    context.drawImage(image, 0, 0)
    return canvas.toDataURL('image/png')
}

export interface SkinsDomain {
    skins: Ref<Skin[]>
    originalSelectedSkin: Ref<Skin | null>
    selectedSkin: Ref<Skin | null>
    armorPreviewConfig: Ref<ArmorPreviewConfig>
    savedArmorPreview: Ref<ArmorPreviewConfig>
    capes: Ref<Cape[]>
    hasPendingSkinChange: import('vue').ComputedRef<boolean>
    hasSavedArmorPreviewChange: import('vue').ComputedRef<boolean>
    setSelectedSkin: (skin: Skin) => void
    addCustomSkin: (skin: Omit<Skin, 'texture_key' | 'source'>) => Skin
    updateCustomSkin: (skin: Skin, previous: Skin) => Skin
    deleteCustomSkin: (skin: Skin) => void
    reorderCustomSkins: (ordered: Skin[]) => void
    equipSkin: (skin: Skin) => void
    saveArmorPreview: () => void
    resetArmorPreviewToSaved: () => void
    restoreDefaultArmorPreview: () => void
}

/**
 * Skin state of the preview user. It lives in the shared domain map so that
 * leaving the page and coming back keeps the saved skins, the pending selection
 * and the armour draft, exactly as the desktop app's store does.
 */
export function useSkinsState(): SkinsDomain {
    return usePreviewDomain('skins', createSkinsDomain)
}

function createSkinsDomain(state: PreviewState): { value: SkinsDomain } {
    // `get_available_capes` lists the online profile's capes; an offline account has none.
    const capes = ref<Cape[]>([])

    const skins = ref<Skin[]>(buildSkinList(state.accounts.skins.value))
    const originalSelectedSkin = ref<Skin | null>(
        skins.value.find((skin) => skin.is_equipped) ?? null,
    )
    const selectedSkin = ref<Skin | null>(originalSelectedSkin.value)

    const savedArmorPreview = ref<ArmorPreviewConfig>(createDefaultArmorPreviewConfig())
    const armorPreviewConfig = ref<ArmorPreviewConfig>(
        cloneArmorPreviewConfig(savedArmorPreview.value),
    )

    const hasPendingSkinChange = computed(
        () => !skinsMatch(selectedSkin.value, originalSelectedSkin.value),
    )

    const hasSavedArmorPreviewChange = computed(() =>
        ARMOR_SLOTS.some((slot) => {
            const draft = armorPreviewConfig.value[slot]
            const saved = savedArmorPreview.value[slot]
            return (
                draft.material !== saved.material ||
                draft.trimPattern !== saved.trimPattern ||
                draft.trimMaterial !== saved.trimMaterial
            )
        }),
    )

    function setSelectedSkin(skin: Skin) {
        selectedSkin.value = skin
    }

    function addCustomSkin(skin: Omit<Skin, 'texture_key' | 'source'>): Skin {
        const textureKey = `custom-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
        const added: Skin = {
            ...skin,
            texture_key: textureKey,
            source: 'custom',
            is_equipped: false,
        }
        const firstDefaultIndex = skins.value.findIndex((entry) => entry.source === 'default')
        const next = [...skins.value]
        if (firstDefaultIndex === -1) next.push(added)
        else next.splice(firstDefaultIndex, 0, added)
        skins.value = next
        return added
    }

    function updateCustomSkin(skin: Skin, previous: Skin): Skin {
        const updated: Skin = { ...skin, source: 'custom' }
        let found = false
        skins.value = skins.value.map((entry) => {
            if (skinsMatch(entry, previous)) {
                found = true
                return { ...updated, is_equipped: entry.is_equipped }
            }
            return entry
        })
        if (!found) {
            const next = [...skins.value]
            next.splice(
                Math.max(
                    0,
                    next.findIndex((entry) => entry.source === 'default'),
                ),
                0,
                updated,
            )
            skins.value = next
        }
        if (skinsMatch(selectedSkin.value, previous)) {
            selectedSkin.value = skins.value.find((entry) => skinsMatch(entry, updated)) ?? updated
        }
        if (skinsMatch(originalSelectedSkin.value, previous)) {
            originalSelectedSkin.value =
                skins.value.find((entry) => skinsMatch(entry, updated)) ?? updated
        }
        return updated
    }

    function deleteCustomSkin(skin: Skin) {
        const next = skins.value.filter((entry) => !skinsMatch(entry, skin))
        skins.value = next
        if (selectedSkin.value && skinsMatch(selectedSkin.value, skin)) {
            selectedSkin.value =
                next.find((entry) => skinsMatch(entry, originalSelectedSkin.value)) ??
                next.find((entry) => entry.is_equipped) ??
                null
        }
        if (originalSelectedSkin.value && skinsMatch(originalSelectedSkin.value, skin)) {
            originalSelectedSkin.value = next.find((entry) => entry.is_equipped) ?? null
        }
    }

    function reorderCustomSkins(ordered: Skin[]) {
        const orderedKeys = new Set(ordered.map((skin) => skin.texture_key))
        const rest = skins.value.filter((skin) => !orderedKeys.has(skin.texture_key))
        const defaults = rest.filter((skin) => skin.source === 'default')
        const others = rest.filter((skin) => skin.source !== 'default')
        skins.value = [...ordered, ...others, ...defaults]
    }

    function equipSkin(skin: Skin) {
        skins.value = skins.value.map((entry) => ({
            ...entry,
            is_equipped: skinsMatch(entry, skin),
        }))
        const equipped = skins.value.find((entry) => skinsMatch(entry, skin)) ?? skin
        originalSelectedSkin.value = equipped
        selectedSkin.value = equipped

        // Keep the shell's account state (and its preview icon) in step.
        const accountEntry = state.accounts.skins.value.find(
            (entry) => entry.texture_key === skin.texture_key,
        )
        if (accountEntry) {
            const index = state.accounts.skins.value.indexOf(accountEntry)
            state.accounts.skins.value = state.accounts.skins.value.map((entry, entryIndex) => ({
                ...entry,
                is_equipped: entryIndex === index,
            }))
        } else {
            state.accounts.skins.value = [
                ...state.accounts.skins.value,
                {
                    texture_key: skin.texture_key,
                    name: skin.name ?? 'Custom skin',
                    section: null,
                    variant: skin.variant === 'SLIM' ? 'SLIM' : 'CLASSIC',
                    texture: skin.texture,
                    cape_id: skin.cape_id ?? null,
                    source: 'custom',
                    is_equipped: true,
                },
            ].map((entry) => ({ ...entry, is_equipped: entry.texture_key === skin.texture_key }))
        }
    }

    function saveArmorPreview() {
        savedArmorPreview.value = cloneArmorPreviewConfig(armorPreviewConfig.value)
    }

    function resetArmorPreviewToSaved() {
        armorPreviewConfig.value = cloneArmorPreviewConfig(savedArmorPreview.value)
    }

    function restoreDefaultArmorPreview() {
        armorPreviewConfig.value = createDefaultArmorPreviewConfig()
    }

    return {
        value: {
            skins,
            originalSelectedSkin,
            selectedSkin,
            armorPreviewConfig,
            savedArmorPreview,
            capes,
            hasPendingSkinChange,
            hasSavedArmorPreviewChange,
            setSelectedSkin,
            addCustomSkin,
            updateCustomSkin,
            deleteCustomSkin,
            reorderCustomSkins,
            equipSkin,
            saveArmorPreview,
            resetArmorPreviewToSaved,
            restoreDefaultArmorPreview,
        },
    }
}
