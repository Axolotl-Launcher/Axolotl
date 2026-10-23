import { SEED_MAP_DEFAULT_VERSION, type SeedMapEdition } from './backend.ts'
import { SEED_MAP_BIOMES, type SeedMapDimension } from './biomes.ts'
import { SEED_MAP_FEATURES, type SeedMapFeatureKind } from './features.ts'
import { SEED_MAP_ORES, type SeedMapDisplayMode, type SeedMapOreKind } from './ores.ts'

export type SeedMapMarker = {
    id: string
    name: string
    x: number
    z: number
    color: string
    dimension: SeedMapDimension
}

export type SeedMapWorkspace = {
    version: 4
    seed: string
    edition: SeedMapEdition
    gameVersion: string
    dimension: SeedMapDimension
    displayMode: SeedMapDisplayMode
    undergroundLayers: number
    undergroundY: number
    center: { x: number; z: number }
    zoom: number
    elevation: number
    showGrid: boolean
    showChunkCoordinates: boolean
    showSpawn: boolean
    terrainEstimation: boolean
    contourLines: boolean
    highlightBiomeEnabled: boolean
    highlightedBiomes: number[]
    visibleFeatures: SeedMapFeatureKind[]
    selectedOres: SeedMapOreKind[]
    oreYMin: number | null
    oreYMax: number | null
    markers: SeedMapMarker[]
    completedFeatures: string[]
    completedOres: string[]
}

type StorageLike = Pick<Storage, 'getItem' | 'setItem'>

export const SEED_MAP_STORAGE_KEY = 'axolotl.lab.seed-map.v1'

export const SEED_MAP_SCALES = [1, 4, 16, 64, 256] as const
export const SEED_MAP_MIN_ZOOM = -2
/** Fixed vertical slices exposed by the seed-map elevation selector. */
export const SEED_MAP_ELEVATIONS = [128, 16, -32] as const

export function createDefaultSeedMapWorkspace(): SeedMapWorkspace {
    return {
        version: 4,
        seed: '10292992',
        edition: 'java',
        gameVersion: SEED_MAP_DEFAULT_VERSION,
        dimension: 'overworld',
        displayMode: 'structures',
        undergroundLayers: 7,
        undergroundY: 16,
        center: { x: 0, z: 0 },
        zoom: 1,
        elevation: SEED_MAP_ELEVATIONS[0],
        showGrid: false,
        showChunkCoordinates: false,
        showSpawn: true,
        terrainEstimation: true,
        contourLines: false,
        highlightBiomeEnabled: false,
        highlightedBiomes: [],
        visibleFeatures: [
            'village',
            'ruined-portal',
            'stronghold',
            'fortress',
            'bastion',
            'end-city',
        ],
        selectedOres: ['diamond'],
        oreYMin: null,
        oreYMax: null,
        markers: [],
        completedFeatures: [],
        completedOres: [],
    }
}

export function loadSeedMapWorkspace(
    storage: StorageLike | null = getBrowserStorage(),
): SeedMapWorkspace {
    const fallback = createDefaultSeedMapWorkspace()
    if (!storage) return fallback
    try {
        const raw = storage.getItem(SEED_MAP_STORAGE_KEY)
        return raw ? sanitizeSeedMapWorkspace(JSON.parse(raw), fallback) : fallback
    } catch {
        return fallback
    }
}

export function saveSeedMapWorkspace(
    workspace: SeedMapWorkspace,
    storage: StorageLike | null = getBrowserStorage(),
): void {
    if (storage)
        storage.setItem(SEED_MAP_STORAGE_KEY, JSON.stringify(sanitizeSeedMapWorkspace(workspace)))
}

export function sanitizeSeedMapWorkspace(
    value: unknown,
    fallback = createDefaultSeedMapWorkspace(),
): SeedMapWorkspace {
    if (!value || typeof value !== 'object') return fallback
    const source = value as Partial<Omit<SeedMapWorkspace, 'version'>> & {
        version?: unknown
        highlightedBiome?: unknown
    }
    const center =
        source.center && typeof source.center === 'object' ? source.center : fallback.center
    const zoom = finiteNumber(source.zoom, fallback.zoom)
    const availableBiomeIds = new Set(SEED_MAP_BIOMES.map((biome) => biome.id))
    const highlightedBiomes = (
        Array.isArray(source.highlightedBiomes)
            ? source.highlightedBiomes
            : typeof source.highlightedBiome === 'number'
              ? [source.highlightedBiome]
              : []
    )
        .filter(
            (biome, index, values): biome is number =>
                typeof biome === 'number' &&
                Number.isInteger(biome) &&
                availableBiomeIds.has(biome) &&
                values.indexOf(biome) === index,
        )
        .slice(0, SEED_MAP_BIOMES.length)
    const selectedOres = sanitizeOreKinds(source.selectedOres)
    const oreYMin = nullableInteger(source.oreYMin, -144, 319)
    const oreYMax = nullableInteger(source.oreYMax, -144, 319)
    return {
        version: 4,
        seed:
            typeof source.seed === 'string' && source.seed.trim()
                ? source.seed.trim().slice(0, 256)
                : fallback.seed,
        edition: source.edition === 'java-large-biomes' ? source.edition : 'java',
        gameVersion:
            typeof source.gameVersion === 'string' ? source.gameVersion : fallback.gameVersion,
        dimension:
            source.dimension === 'nether' || source.dimension === 'end'
                ? source.dimension
                : 'overworld',
        displayMode:
            source.displayMode === 'ores' || source.displayMode === 'underground'
                ? source.displayMode
                : 'structures',
        undergroundLayers: Math.trunc(finiteNumber(source.undergroundLayers, 7)) & 7,
        undergroundY: Math.max(
            -64,
            Math.min(319, Math.trunc(finiteNumber(source.undergroundY, 16))),
        ),
        center: {
            x: finiteNumber(center.x, fallback.center.x),
            z: finiteNumber(center.z, fallback.center.z),
        },
        zoom:
            Math.round(
                Math.min(Math.max(zoom, SEED_MAP_MIN_ZOOM), SEED_MAP_SCALES.length - 1) * 1_000,
            ) / 1_000,
        elevation: sanitizeElevation(source.elevation, fallback.elevation),
        showGrid: source.showGrid === true,
        showChunkCoordinates: source.showChunkCoordinates === true,
        showSpawn: source.showSpawn !== false,
        terrainEstimation: source.version === 1 ? true : source.terrainEstimation !== false,
        contourLines: source.contourLines === true,
        highlightBiomeEnabled:
            source.highlightBiomeEnabled === true && highlightedBiomes.length > 0,
        highlightedBiomes,
        visibleFeatures: sanitizeFeatureKinds(source.visibleFeatures),
        selectedOres: selectedOres.length > 0 ? selectedOres : [...fallback.selectedOres],
        oreYMin: oreYMin !== null && oreYMax !== null && oreYMin > oreYMax ? oreYMax : oreYMin,
        oreYMax: oreYMin !== null && oreYMax !== null && oreYMax < oreYMin ? oreYMin : oreYMax,
        markers: sanitizeMarkers(source.markers),
        completedFeatures: Array.isArray(source.completedFeatures)
            ? source.completedFeatures
                  .filter((key): key is string => typeof key === 'string')
                  .slice(0, 2_000)
            : [],
        completedOres: Array.isArray(source.completedOres)
            ? source.completedOres
                  .filter((key): key is string => typeof key === 'string')
                  .slice(0, 10_000)
            : [],
    }
}

export function createShareQuery(workspace: SeedMapWorkspace): Record<string, string> {
    return {
        seed: workspace.seed,
        edition: workspace.edition,
        version: workspace.gameVersion,
        dimension: workspace.dimension,
        mode: workspace.displayMode,
        undergroundY: String(workspace.undergroundY),
        undergroundLayers: String(workspace.undergroundLayers),
        ores: workspace.selectedOres.join(','),
        yMin: workspace.oreYMin === null ? '' : String(workspace.oreYMin),
        yMax: workspace.oreYMax === null ? '' : String(workspace.oreYMax),
        x: String(Math.round(workspace.center.x)),
        z: String(Math.round(workspace.center.z)),
        zoom: String(workspace.zoom),
    }
}

export function isCurrentSeedMapEpoch(responseEpoch: number, currentEpoch: number): boolean {
    return responseEpoch === currentEpoch
}

export function applyShareQuery(
    query: Record<string, unknown>,
    workspace = createDefaultSeedMapWorkspace(),
): SeedMapWorkspace {
    return sanitizeSeedMapWorkspace({
        ...workspace,
        seed: typeof query.seed === 'string' ? query.seed : workspace.seed,
        edition: query.edition ?? workspace.edition,
        gameVersion: typeof query.version === 'string' ? query.version : workspace.gameVersion,
        dimension: query.dimension ?? workspace.dimension,
        displayMode: query.mode ?? workspace.displayMode,
        undergroundY: parseQueryNumber(query.undergroundY, workspace.undergroundY),
        undergroundLayers: parseQueryNumber(query.undergroundLayers, workspace.undergroundLayers),
        selectedOres:
            typeof query.ores === 'string' && query.ores.length > 0
                ? query.ores.split(',')
                : workspace.selectedOres,
        oreYMin: parseNullableQueryNumber(query.yMin, workspace.oreYMin),
        oreYMax: parseNullableQueryNumber(query.yMax, workspace.oreYMax),
        center: {
            x: parseQueryNumber(query.x, workspace.center.x),
            z: parseQueryNumber(query.z, workspace.center.z),
        },
        zoom: parseQueryNumber(query.zoom, workspace.zoom),
    })
}

function finiteNumber(value: unknown, fallback: number): number {
    return typeof value === 'number' && Number.isFinite(value) ? value : fallback
}

function sanitizeElevation(value: unknown, fallback: number): number {
    const parsed = Math.round(finiteNumber(value, Number.NaN))
    if (SEED_MAP_ELEVATIONS.some((elevation) => elevation === parsed)) return parsed
    const normalizedFallback = Math.round(finiteNumber(fallback, SEED_MAP_ELEVATIONS[0]))
    return SEED_MAP_ELEVATIONS.some((elevation) => elevation === normalizedFallback)
        ? normalizedFallback
        : SEED_MAP_ELEVATIONS[0]
}

function parseQueryNumber(value: unknown, fallback: number): number {
    if (typeof value !== 'string') return fallback
    const parsed = Number(value)
    return Number.isFinite(parsed) ? parsed : fallback
}

function parseNullableQueryNumber(value: unknown, fallback: number | null): number | null {
    if (value === '') return null
    if (typeof value !== 'string' && typeof value !== 'number') return fallback
    const parsed = Number(value)
    return Number.isFinite(parsed) ? parsed : fallback
}

function nullableInteger(value: unknown, min: number, max: number): number | null {
    if (value === null || value === undefined || value === '') return null
    const parsed = Number(value)
    if (!Number.isFinite(parsed)) return null
    return Math.min(max, Math.max(min, Math.round(parsed)))
}

function getBrowserStorage(): Storage | null {
    return typeof window === 'undefined' ? null : window.localStorage
}

function sanitizeFeatureKinds(value: unknown): SeedMapFeatureKind[] {
    if (!Array.isArray(value)) return createDefaultSeedMapWorkspace().visibleFeatures
    const available = new Set(SEED_MAP_FEATURES.map((feature) => feature.kind))
    return value.filter(
        (kind): kind is SeedMapFeatureKind =>
            typeof kind === 'string' && available.has(kind as SeedMapFeatureKind),
    )
}

function sanitizeOreKinds(value: unknown): SeedMapOreKind[] {
    if (!Array.isArray(value)) return []
    const available = new Set(SEED_MAP_ORES.map((ore) => ore.kind))
    return value
        .filter(
            (ore, index, values): ore is SeedMapOreKind =>
                typeof ore === 'string' &&
                available.has(ore as SeedMapOreKind) &&
                values.indexOf(ore) === index,
        )
        .slice(0, SEED_MAP_ORES.length)
}

function sanitizeMarkers(value: unknown): SeedMapMarker[] {
    if (!Array.isArray(value)) return []
    return value
        .flatMap((marker) => {
            if (!marker || typeof marker !== 'object') return []
            const source = marker as Partial<SeedMapMarker>
            if (typeof source.id !== 'string' || typeof source.name !== 'string') return []
            if (!Number.isFinite(source.x) || !Number.isFinite(source.z)) return []
            return [
                {
                    id: source.id.slice(0, 128),
                    name: source.name.slice(0, 128),
                    x: Math.round(source.x as number),
                    z: Math.round(source.z as number),
                    dimension:
                        source.dimension === 'nether' || source.dimension === 'end'
                            ? source.dimension
                            : ('overworld' as SeedMapDimension),
                    color:
                        typeof source.color === 'string' && /^#[0-9A-Fa-f]{6}$/.test(source.color)
                            ? source.color
                            : '#22C55E',
                },
            ]
        })
        .slice(0, 1_000)
}
