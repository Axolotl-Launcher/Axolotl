import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'

import {
    minecraftEnchantmentLevel,
    resolveMinecraftAttributeName,
    resolveRecipeItemName,
} from '../recipe-generator/item-names.ts'
import type { SeedMapStructureDetails, SeedMapStructurePiece } from './advanced.ts'
import { SEED_MAP_BIOME_NAMES, seedMapBiomeMessage } from './biomes.ts'
import type { SeedMapFeatureKind } from './features.ts'
import { structureLabelMessages as messages } from './structure-label-messages.ts'

type NameKey = keyof typeof messages
type Format = (message: MessageDescriptor, values?: Record<string, string | number>) => string
type Variant = NonNullable<SeedMapStructureDetails['variant']>

const pieceNames: Record<string, NameKey> = {
    TeDP: 'desert-pyramid',
    TeJP: 'jungle-temple',
    TeSH: 'swamp-hut',
    BTP: 'buried-treasure',
    RUPO: 'ruined-portal',
    SHS: 'stronghold-straight',
    SHPH: 'stronghold-prison',
    SHLT: 'stronghold-left',
    SHRT: 'stronghold-right',
    SHRC: 'stronghold-crossing',
    SHSSD: 'stronghold-stairs-straight',
    SHSD: 'stronghold-stairs',
    SH5C: 'stronghold-five',
    SHCC: 'stronghold-chest',
    SHLi: 'stronghold-library',
    SHPR: 'stronghold-portal',
    SHFC: 'stronghold-filler',
    NeStart: 'fortress-start',
    NeBS: 'fortress-bridge',
    NeBCr: 'fortress-crossing',
    NeRC: 'fortress-fortified',
    NeSR: 'fortress-stairs',
    NeMT: 'fortress-spawner',
    NeCE: 'fortress-entrance',
    NeSC: 'fortress-corridor',
    NeSCSC: 'fortress-corridor-crossing',
    NeSCRT: 'fortress-right',
    NeSCLT: 'fortress-left',
    NeCCS: 'fortress-corridor-stairs',
    NeCTB: 'fortress-t',
    NeCSR: 'fortress-wart',
    NeBEF: 'fortress-end',
    'igloo/top': 'igloo-top',
    'igloo/middle': 'igloo-middle',
    'igloo/bottom': 'igloo-bottom',
    'pillager_outpost/watchtower': 'watchtower',
    'bastion/units/walls/wall_base': 'bastion-wall',
    'bastion/hoglin_stable/ramparts/ramparts_3': 'bastion-rampart',
    'bastion/treasure/ramparts/mid_wall_main': 'bastion-mid-wall',
    'bastion/bridge/starting_pieces/entrance': 'bastion-entrance',
}
const endPieces: Record<string, NameKey> = {
    base_floor: 'end-base-floor',
    base_roof: 'end-base-roof',
    bridge_end: 'end-bridge-end',
    bridge_gentle_stairs: 'end-gentle',
    bridge_piece: 'end-bridge',
    bridge_steep_stairs: 'end-steep',
    fat_tower_base: 'end-fat-base',
    fat_tower_middle: 'end-fat-middle',
    fat_tower_top: 'end-fat-top',
    second_floor_1: 'end-second-1',
    second_floor_2: 'end-second-2',
    second_roof: 'end-second-roof',
    ship: 'end-ship',
    third_floor_1: 'end-third-1',
    third_floor_2: 'end-third-2',
    third_roof: 'end-third-roof',
    tower_base: 'end-tower-base',
    tower_floor: 'end-tower-floor',
    tower_piece: 'end-tower-piece',
    tower_top: 'end-tower-top',
}
const shipNames: Record<string, NameKey> = {
    with_mast: 'ship-mast',
    upsidedown_full: 'ship-up-full',
    upsidedown_fronthalf: 'ship-up-front',
    upsidedown_backhalf: 'ship-up-back',
    sideways_full: 'ship-side-full',
    sideways_fronthalf: 'ship-side-front',
    sideways_backhalf: 'ship-side-back',
    rightsideup_full: 'ship-full',
    rightsideup_fronthalf: 'ship-front',
    rightsideup_backhalf: 'ship-back',
}
const shipTemplates = [
    'with_mast',
    'upsidedown_full',
    'upsidedown_fronthalf',
    'upsidedown_backhalf',
    'sideways_full',
    'sideways_fronthalf',
    'sideways_backhalf',
    'rightsideup_full',
    'rightsideup_fronthalf',
    'rightsideup_backhalf',
    'with_mast_degraded',
    'upsidedown_full_degraded',
    'upsidedown_fronthalf_degraded',
    'upsidedown_backhalf_degraded',
    'sideways_full_degraded',
    'sideways_fronthalf_degraded',
    'sideways_backhalf_degraded',
    'rightsideup_full_degraded',
    'rightsideup_fronthalf_degraded',
    'rightsideup_backhalf_degraded',
]
const tables: Record<string, NameKey> = {
    desert_pyramid: 'loot-desert',
    jungle_temple: 'loot-jungle',
    jungle_temple_dispenser: 'loot-dispenser',
    igloo_chest: 'loot-igloo',
    pillager_outpost: 'loot-outpost',
    shipwreck_supply: 'loot-ship-supply',
    shipwreck_map: 'loot-ship-map',
    shipwreck_treasure: 'loot-ship-treasure',
    nether_bridge: 'loot-fortress',
    bastion_other: 'loot-bastion',
    bastion_bridge: 'loot-bastion-bridge',
    bastion_treasure: 'loot-bastion-treasure',
    bastion_hoglin_stable: 'loot-bastion-stable',
    end_city_treasure: 'loot-end',
    stronghold_library: 'loot-library',
    stronghold_corridor: 'loot-corridor',
    stronghold_crossing: 'loot-crossing',
    buried_treasure: 'loot-buried',
    ruined_portal: 'loot-portal',
    abandoned_camp_barrel: 'loot-camp-barrel',
    abandoned_camp_common_chest: 'loot-camp-common',
    abandoned_camp_special_chest: 'loot-camp-special',
    abandoned_camp_treasure_chest: 'loot-camp-treasure',
    abandoned_camp_secret_chest: 'loot-camp-secret',
}
const rotatingKinds = new Set<SeedMapFeatureKind>([
    'village',
    'bastion',
    'ancient-city',
    'ruined-portal',
    'igloo',
    'shipwreck',
    'outpost',
    'desert-pyramid',
    'jungle-temple',
    'swamp-hut',
    'trial-chambers',
    'abandoned-camp',
    'nether-fossil',
])

/** Preserve upstream identifiers separately from translated display names. */
export function createSeedMapLabels(format: Format, locale: string) {
    const text = (key: NameKey, values?: Record<string, string | number>) =>
        format(messages[key], values)
    function biome(id: number): string | undefined {
        const name = SEED_MAP_BIOME_NAMES[id]
        const message = name && seedMapBiomeMessage(name)
        return message ? format(message) : undefined
    }
    function biomeName(name: string, id: number) {
        const label = biome(id)
        return label ? text('biome-name', { biome: label, name }) : name
    }
    function pieceId(name: string, variant?: Variant): string | undefined {
        if (!name.includes('%')) return name === 'unknown' ? undefined : name
        if (!/^abandoned_camp\/tent\/%s\/tent_%s_\d+$/.test(name) || !variant) return undefined
        const biomeName = SEED_MAP_BIOME_NAMES[variant.biome]
        if (!biomeName) return undefined
        const slug = biomeName.toLowerCase().replaceAll(' ', '_')
        return name.replaceAll('%s', slug)
    }
    function piece(kind: SeedMapFeatureKind, name: string, variant?: Variant): string {
        const id = name.replace(/^minecraft:/, '')
        if (Object.hasOwn(pieceNames, id)) return text(pieceNames[id])
        const end = id.replace(/^end_city\//, '')
        if (kind === 'end-city' && Object.hasOwn(endPieces, end)) return text(endPieces[end])
        const portal = /^ruined_portal\/(giant_)?portal_(\d+)$/.exec(id)
        if (portal)
            return text(portal[1] ? 'giant-portal' : 'portal', { number: Number(portal[2]) })
        const fossil = /^nether_fossils\/fossil_(\d+)$/.exec(id)
        if (fossil) return text('fossil', { number: Number(fossil[1]) })
        const ship = /^shipwreck\/(.+?)(_degraded)?$/.exec(id)
        if (ship && Object.hasOwn(shipNames, ship[1])) {
            const label = text(shipNames[ship[1]])
            return ship[2] ? text('degraded', { name: label }) : label
        }
        const tent = /^abandoned_camp\/tent\/([^/]+)\/tent_[^/]+_(\d+)$/.exec(id)
        if (tent) return biomeName(text('tent', { number: Number(tent[2]) }), variant?.biome ?? -1)
        const camp =
            /^abandoned_camp\/camp\/([^/]+)\/campsite_.+?_(?:(barrel|chest|special)_)?(\d+)$/.exec(
                id,
            )
        if (camp) {
            const label = text(
                camp[2] ? `campsite-${camp[2] as 'barrel' | 'chest' | 'special'}` : 'campsite',
                { number: Number(camp[3]) },
            )
            const biomeId = Object.entries(SEED_MAP_BIOME_NAMES).find(
                ([, value]) => value.toLowerCase().replaceAll(' ', '_') === camp[1],
            )?.[0]
            return biomeName(label, biomeId === undefined ? -1 : Number(biomeId))
        }
        return text('unknown-piece')
    }
    function variant(kind: SeedMapFeatureKind, value: Variant, pieces: SeedMapStructurePiece[]) {
        const start = value.start
        if (!Number.isInteger(start) || start < 0 || start === 255) return undefined
        let id: string | undefined
        let name: string | undefined
        if (kind === 'bastion' && start < 4) {
            const keys = ['units', 'stable', 'treasure', 'bastion-bridge'] as const
            name = text(keys[start])
            id = ['units', 'hoglin_stable', 'treasure', 'bridge'][start]
        } else if (kind === 'village') {
            const villages: Record<number, [string, number, number]> = {
                1: ['plains', 0, 3],
                177: ['plains', 0, 3],
                2: ['desert', 1, 3],
                35: ['savanna', 1, 4],
                5: ['taiga', 1, 2],
                12: ['snowy', 1, 3],
            }
            const definition = villages[value.biome]
            if (definition && start >= definition[1] && start <= definition[2]) {
                id =
                    definition[0] === 'plains' && start === 0
                        ? 'plains_fountain_01'
                        : `${definition[0]}_meeting_point_${start}`
                name = biomeName(
                    text(start === 0 ? 'fountain' : 'meeting', { number: start }),
                    value.biome,
                )
            }
        } else if (kind === 'ruined-portal' && start >= 1 && start <= (value.giant ? 3 : 10)) {
            id = `${value.giant ? 'giant_' : ''}ruined_portal_${start}`
            name = text(value.giant ? 'giant-portal' : 'portal', { number: start })
        } else if (kind === 'ancient-city' && start >= 1 && start <= 3) {
            id = `city_center_${start}`
            name = text('city-center', { number: start })
        } else if (kind === 'trial-chambers' && start < 2) {
            id = `corridor/end_${start + 1}`
            name = text('trial-end', { number: start + 1 })
        } else if (kind === 'nether-fossil' && start < 14) {
            id = `fossil_${start + 1}`
            name = text('fossil', { number: start + 1 })
        } else if (kind === 'abandoned-camp' && start >= 1 && start <= 10) {
            id = pieceId(`abandoned_camp/tent/%s/tent_%s_${start}`, value)
            name = biomeName(text('tent', { number: start }), value.biome)
        } else if (kind === 'shipwreck') {
            const actual = pieces.find((p) => p.name.startsWith('shipwreck/'))
            const ocean = new Set([0, 10, 24, 44, 45, 46, 47, 48, 49, 50]).has(value.biome)
            const index = ocean ? start : [0, 4, 5, 6, 7, 8, 9, 10, 17, 18, 19][start]
            id =
                actual?.name ??
                (shipTemplates[index] ? `shipwreck/${shipTemplates[index]}` : undefined)
            if (id) name = piece(kind, id)
        }
        return name ? { name, id } : undefined
    }
    function rotation(degrees: number): string {
        return Number.isFinite(degrees) && degrees % 90 === 0
            ? text('rotation', { degrees: ((degrees % 360) + 360) % 360 })
            : text('unknown-rotation')
    }
    function table(id: string): string {
        const path = id.replace(/^minecraft:/, '')
        const match = /^(?:chests|barrels)\/([^/]+)$/.exec(path)
        return match && Object.hasOwn(tables, match[1])
            ? text(tables[match[1]])
            : text('unknown-table')
    }
    return {
        piece,
        pieceId,
        variant,
        rotation,
        table,
        hasRotation: (kind: SeedMapFeatureKind) => rotatingKinds.has(kind),
        hasMirror: (kind: SeedMapFeatureKind) => kind === 'ruined-portal' || kind === 'igloo',
        mirror: (value: number) =>
            text(value === 0 ? 'no-mirror' : value === 1 ? 'mirrored' : 'unknown-mirror'),
        item: (id: string) => resolveRecipeItemName(id, locale, '') || text('unknown-item'),
        enchantment: (id: string) =>
            resolveMinecraftAttributeName('enchantment', id, locale) ?? text('unknown-enchantment'),
        effect: (id: string) =>
            resolveMinecraftAttributeName('effect', id, locale) ?? text('unknown-effect'),
        level: minecraftEnchantmentLevel,
    }
}

export function seedMapEffectDuration(ticks: number): string | undefined {
    if (!Number.isFinite(ticks) || ticks <= 0) return undefined
    const seconds = Math.floor(ticks / 20)
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
}
