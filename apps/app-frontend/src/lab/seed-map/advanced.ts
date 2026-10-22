import { invoke } from '@tauri-apps/api/core'

import type { SeedMapEdition } from './backend'
import type { SeedMapDimension } from './biomes'

export type SeedMapPosition = { x: number; y: number; z: number }
export type SeedMapStructureQuery = {
    seed: string
    edition: SeedMapEdition
    version: string
    dimension: SeedMapDimension
    x: number
    z: number
    featureMask: number
    includeBlocks?: boolean
}

export type SeedMapStructurePiece = {
    id: number
    name: string
    rotation: number
    position: SeedMapPosition
    positionKnown: boolean
    min?: SeedMapPosition
    max?: SeedMapPosition
    portalEyes?: number
    endPortalRoom?: boolean
    templateId?: string
    mirror?: boolean
}

export type SeedMapContainer = {
    id: number
    piece: number
    x: number
    z: number
    table?: string | null
    estimated?: boolean
    fixedItems?: { id: string; count: number }[]
}
export type SeedMapStructureDetails = {
    supported: boolean
    capabilities?: { variant: boolean; containers: boolean }
    truncated: boolean
    variant?: {
        rotation: number
        mirror: number
        start: number
        biome: number
        abandoned: boolean
        giant: boolean
        underground: boolean
        airPocket: boolean
        basement: boolean
        cracked: boolean
        size: number
        offset: SeedMapPosition
        size3d: SeedMapPosition
    }
    pieces: SeedMapStructurePiece[]
    blocks?: {
        position: [number, number, number]
        state: { name: string; properties: Record<string, string> }
        rotation: number
        mirror: boolean
        pieceId: number
        overwriteAir: boolean
    }[]
    containers: SeedMapContainer[]
    limits: string[]
}

export type SeedMapLoot = {
    supported: boolean
    truncated: boolean
    estimated?: boolean
    items?: {
        id: string
        count: number
        enchantments: { id: string; level: number }[]
        effect?: string
        duration?: number
    }[]
    unresolvedTables?: string[]
    limits: string[]
}

export function getSeedMapStructureDetails(query: SeedMapStructureQuery) {
    return invoke<SeedMapStructureDetails>('plugin:seed-map|seed_map_structure_details', { query })
}

export function getSeedMapContainerLoot(query: SeedMapStructureQuery, container: number) {
    return invoke<SeedMapLoot>('plugin:seed-map|seed_map_container_loot', { query, container })
}
