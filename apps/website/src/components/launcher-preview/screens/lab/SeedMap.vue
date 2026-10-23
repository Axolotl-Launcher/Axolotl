<script setup lang="ts">
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import CircleAlertIcon from '@modrinth/assets/icons/circle-alert.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import CompassIcon from '@modrinth/assets/icons/compass.svg?component'
import ContractIcon from '@modrinth/assets/icons/contract.svg?component'
import ExpandIcon from '@modrinth/assets/icons/expand.svg?component'
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import EyeOffIcon from '@modrinth/assets/icons/eye-off.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import HashIcon from '@modrinth/assets/icons/hash.svg?component'
import HistoryIcon from '@modrinth/assets/icons/history.svg?component'
import ImportIcon from '@modrinth/assets/icons/import.svg?component'
import InfoIcon from '@modrinth/assets/icons/info.svg?component'
import LandmarkIcon from '@modrinth/assets/icons/landmark.svg?component'
import LayersIcon from '@modrinth/assets/icons/layers.svg?component'
import MinusIcon from '@modrinth/assets/icons/minus.svg?component'
import PickaxeIcon from '@modrinth/assets/icons/pickaxe.svg?component'
import PinIcon from '@modrinth/assets/icons/pin.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import ScaleIcon from '@modrinth/assets/icons/scale.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import SettingsIcon from '@modrinth/assets/icons/settings.svg?component'
import ShareIcon from '@modrinth/assets/icons/share.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import PopoutMenu from '@modrinth/ui/src/components/base/PopoutMenu.vue'
import Slider from '@modrinth/ui/src/components/base/Slider.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useRelativeTime } from '@modrinth/ui/src/composables/how-ago.ts'
import {
    defineMessages,
    type MessageDescriptor,
    useVIntl,
} from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import {
    computed,
    defineAsyncComponent,
    inject,
    nextTick,
    onBeforeUnmount,
    onMounted,
    onUnmounted,
    reactive,
    ref,
    useTemplateRef,
    watch,
} from 'vue'
import { useRoute } from 'vue-router'

import { loadMinecraftItemImage } from './recipe-generator/item-images.ts'
import {
    getSeedMapStructureDetails,
    type SeedMapStructurePiece,
    type SeedMapStructureQuery,
} from './seed-map/advanced.ts'
import { type AreaLootMatch, type SeedMapArea, seedMapArea } from './seed-map/area-search.ts'
import { compareSeedMapVersions } from './seed-map/backend.ts'
import {
    applyShareQuery,
    clearSeedMapHistory,
    createDefaultSeedMapWorkspace,
    createShareQuery,
    fallbackSeedMapProfiles,
    featureKey,
    featureMask,
    findSeedMapFeatures,
    getSeedMapBiomeAt,
    getSeedMapProfiles,
    getSeedMapSpawn,
    loadSeedMapHistory,
    loadSeedMapWorkspace,
    recordSeedMapHistory,
    removeSeedMapHistoryEntry,
    renderSeedMapTile,
    saveSeedMapWorkspace,
    SEED_MAP_BIOME_NAMES,
    SEED_MAP_BIOMES,
    SEED_MAP_ELEVATIONS,
    SEED_MAP_END_CITY_IMAGE_SOURCES as endCityImageSources,
    SEED_MAP_FEATURE_COLORS as featureColors,
    SEED_MAP_FEATURE_ICONS as featureIcons,
    SEED_MAP_FEATURE_IMAGE_SOURCES as featureImageSources,
    SEED_MAP_FEATURES,
    SEED_MAP_MIN_ZOOM,
    SEED_MAP_ORE_MAX_SCALE,
    SEED_MAP_SCALES,
    SEED_MAP_STRUCTURE_ASSET_ROOT as structureAssetRoot,
    seedMapBiomeMessage,
    type SeedMapDimension,
    type SeedMapDisplayMode,
    type SeedMapEdition,
    type SeedMapFeature,
    type SeedMapFeatureKind,
    type SeedMapHistoryEntry,
    seedMapHistoryId,
    type SeedMapHistorySource,
    type SeedMapMarker,
    seedMapOreDefinition,
    type SeedMapOreHit,
    seedMapOreKey,
    type SeedMapOreKind,
    seedMapOresForDimension,
    seedMapOreYRange,
    type SeedMapSpawnPoint,
    seedMapTileConcurrency,
    type SeedMapVersionProfile,
    updateSeedMapHistoryProgress,
    useSeedMapOreLayer,
    visibleFeatureDefinitions,
} from './seed-map/core.ts'
import { createSeedMapLayoutLayer } from './seed-map/layout-layer.ts'
import {
    drawSeedMapMarkers,
    hitSeedMapMarker,
    layoutSeedMapMarkers,
} from './seed-map/marker-rendering.ts'
import { seedMapPreviewLimitReason } from './seed-map/preview-data.ts'
import SeedMapAreaAnalysis from './seed-map/SeedMapAreaAnalysis.vue'
import SeedMapBiomePicker from './seed-map/SeedMapBiomePicker.vue'
import SeedMapCopyrightModal from './seed-map/SeedMapCopyrightModal.vue'
import SeedMapPreviewNotice from './seed-map/SeedMapPreviewNotice.vue'
import SeedMapSplitView from './seed-map/SeedMapSplitView.vue'
import SeedMapStructureDetails from './seed-map/SeedMapStructureDetails.vue'
import SeedMapWorldImportModal, {
    type SeedMapWorldImport,
} from './seed-map/SeedMapWorldImportModal.vue'
import {
    drawSeedMapLayout,
    hitSeedMapLayout,
    SEED_MAP_LAYOUT_MAX_SCALE,
    seedMapLayoutPalette,
    seedMapLayoutPieces,
    seedMapStructureBadgePositions,
} from './seed-map/structure-layout.ts'
import {
    createSeedMapTileLayer,
    seedMapTileLevel,
    type SeedMapTileView,
} from './seed-map/tile-layer.ts'

const SeedMapUndergroundPreview = defineAsyncComponent(
    () => import('./seed-map/SeedMapUndergroundPreview.vue'),
)

type MapBounds = {
    minX: number
    minZ: number
    maxX: number
    maxZ: number
}

type Selection = {
    x: number
    z: number
    feature?: SeedMapFeature
    ore?: SeedMapOreHit
    markerId?: string
    spawn?: boolean
    biome?: number
}

type ActivePointer = {
    x: number
    y: number
}

type SeedMapElevation = (typeof SEED_MAP_ELEVATIONS)[number]

const { addNotification, handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const route = useRoute()
const canvas = useTemplateRef<HTMLCanvasElement>('mapCanvas')
const fullscreenContainer = useTemplateRef<HTMLElement>('fullscreenContainer')
const workspace = reactive(applyShareQuery(route.query, loadSeedMapWorkspace()))
if (!SEED_MAP_ELEVATIONS.includes(workspace.elevation as SeedMapElevation))
    workspace.elevation = SEED_MAP_ELEVATIONS[0]
const profiles = ref<SeedMapVersionProfile[]>(fallbackSeedMapProfiles())
const profilesResolved = ref(false)
const features = ref<SeedMapFeature[]>([])
const spawn = ref<SeedMapSpawnPoint | null>(null)
const selection = ref<Selection | null>(null)
const selectedArea = ref<SeedMapArea | null>(null)
const areaDrag = ref<{
    id: number
    start: { x: number; z: number }
    end: { x: number; z: number }
    screenX: number
    screenY: number
    moved: boolean
} | null>(null)
let suppressAreaContextMenu = false
let areaContextMenuTimer: ReturnType<typeof setTimeout> | undefined
function captureAreaContextMenu(event: MouseEvent) {
    if (!areaDrag.value && !suppressAreaContextMenu) return
    event.preventDefault()
    if (areaDrag.value?.moved || suppressAreaContextMenu) {
        event.stopImmediatePropagation()
        suppressAreaContextMenu = false
    }
}
const areaStyle = computed(() => {
    const area = areaDrag.value?.moved
        ? seedMapArea(areaDrag.value.start, areaDrag.value.end)
        : selectedArea.value
    if (!area) return undefined
    const min = worldToScreen(area.minX, area.minZ)
    const max = worldToScreen(area.maxX + 1, area.maxZ + 1)
    return {
        left: `${min.x}px`,
        top: `${min.y}px`,
        width: `${max.x - min.x}px`,
        height: `${max.y - min.y}px`,
    }
})
const markerName = ref('')
const markerColor = ref('#22C55E')
const markerDraftOpen = ref(false)
const advancedOpen = ref(false)
const layersExpanded = ref(false)
const showLayerNames = ref(false)
const rulerPoints = ref<{ x: number; z: number }[]>([])
const rulerEnabled = ref(false)
const isFullscreen = ref(false)
const loading = ref(false)
const mapError = ref('')
const mapPreviewLimit = computed(() => seedMapPreviewLimitReason(mapError.value))
const historyEntries = ref<SeedMapHistoryEntry[]>(loadSeedMapHistory())
const worldImportModal =
    useTemplateRef<InstanceType<typeof SeedMapWorldImportModal>>('worldImportModal')
const copyrightModal = useTemplateRef<InstanceType<typeof SeedMapCopyrightModal>>('copyrightModal')
let activeHistoryId = seedMapHistoryId(workspace.seed.trim().slice(0, 256), workspace.edition)
const coordinateX = ref(String(Math.round(workspace.center.x)))
const coordinateZ = ref(String(Math.round(workspace.center.z)))
const featureImages = new Map<string, HTMLImageElement>()
const oreImages = new Map<string, HTMLImageElement>()
let spawnImage: HTMLImageElement | undefined
const activePointers = new Map<number, ActivePointer>()
const pointer = reactive({ id: -1, startX: 0, startY: 0, centerX: 0, centerZ: 0, moved: false })
const viewport = reactive({ width: 0, height: 0 })
let requestEpoch = 0
let refreshTimer: ReturnType<typeof window.setTimeout> | undefined
let saveTimer: ReturnType<typeof window.setTimeout> | undefined
let longPressTimer: ReturnType<typeof window.setTimeout> | undefined
let redrawFrame: number | undefined
let panFrame: number | undefined
let pendingPanCenter: { x: number; z: number } | undefined
let zoomAnimationFrame: number | undefined
let resizeObserver: ResizeObserver | undefined
let pinchDistance = 0
let wheelMode: 'trackpad' | 'wheel' | undefined
let wheelGestureStartedAt = 0
let wheelDelta = 0
let wheelTimer: ReturnType<typeof window.setTimeout> | undefined
let trackpadTimer: ReturnType<typeof window.setTimeout> | undefined
let wheelAnchor = { x: 0, y: 0 }
let suppressPointerSelection = false
let centerOnNextSpawn = false
let spawnContextKey = ''
let spawnRequest: { key: string; promise: Promise<SeedMapSpawnPoint> } | undefined
let disposed = false
let pendingHistorySource: SeedMapHistorySource | null = null
let pendingHistoryLabels: { instanceName?: string; worldName?: string } = {}
let historyTimer: ReturnType<typeof window.setTimeout> | undefined
let progressTimer: ReturnType<typeof window.setTimeout> | undefined
let applyingSharedState = false
let biomeLookupToken = 0

const formatRelativeTime = useRelativeTime()

const {
    hits: oreHits,
    scanning: oreScanning,
    truncated: oreScanTruncated,
    scannedChunks: oreScannedChunks,
    totalChunks: oreTotalChunks,
    refresh: refreshOreLayer,
    refreshFilter: refreshOreFilter,
    dispose: disposeOreLayer,
} = useSeedMapOreLayer({
    onUpdate: () => redraw(),
    onError: (message) => {
        mapError.value = seedMapPreviewLimitReason(message)
            ? message
            : formatMessage(messages.oreScanFailed)
    },
})

const messages = defineMessages({
    retryMap: { id: 'app.lab.seed-map.retry-map', defaultMessage: 'Retry map calculation' },
    title: { id: 'app.lab.seed-map.title', defaultMessage: 'Seed map' },
    seed: { id: 'app.lab.seed-map.seed', defaultMessage: 'Seed' },
    seedPlaceholder: {
        id: 'app.lab.seed-map.seed-placeholder',
        defaultMessage: 'Enter a number or text seed',
    },
    randomSeed: { id: 'app.lab.seed-map.random-seed', defaultMessage: 'Random seed' },
    importFromInstance: {
        id: 'app.lab.seed-map.import-from-instance',
        defaultMessage: 'Load from instance',
    },
    worldSeedLoaded: {
        id: 'app.lab.seed-map.world-seed-loaded',
        defaultMessage: 'Loaded the seed of {world}',
    },
    history: { id: 'app.lab.seed-map.history', defaultMessage: 'Seed history' },
    historyEmpty: {
        id: 'app.lab.seed-map.history-empty',
        defaultMessage: 'Seeds you view will appear here',
    },
    clearHistory: { id: 'app.lab.seed-map.clear-history', defaultMessage: 'Clear history' },
    removeHistoryEntry: {
        id: 'app.lab.seed-map.remove-history-entry',
        defaultMessage: 'Remove from history',
    },
    historySourceManual: {
        id: 'app.lab.seed-map.history-source.manual',
        defaultMessage: 'Entered manually',
    },
    historySourceRandom: {
        id: 'app.lab.seed-map.history-source.random',
        defaultMessage: 'Random seed',
    },
    historySourceShare: {
        id: 'app.lab.seed-map.history-source.share',
        defaultMessage: 'Shared link',
    },
    edition: { id: 'app.lab.seed-map.edition', defaultMessage: 'Edition' },
    java: { id: 'app.lab.seed-map.edition.java', defaultMessage: 'Java Edition' },
    javaLargeBiomes: {
        id: 'app.lab.seed-map.edition.java-large-biomes',
        defaultMessage: 'Java: Large Biomes',
    },
    gameVersion: { id: 'app.lab.seed-map.version', defaultMessage: 'Game version' },
    dimension: { id: 'app.lab.seed-map.dimension', defaultMessage: 'Dimension' },
    overworld: { id: 'app.lab.seed-map.dimension.overworld', defaultMessage: 'Overworld' },
    nether: { id: 'app.lab.seed-map.dimension.nether', defaultMessage: 'Nether' },
    end: { id: 'app.lab.seed-map.dimension.end', defaultMessage: 'The End' },
    coordinates: { id: 'app.lab.seed-map.coordinates', defaultMessage: 'Coordinates' },
    surface: { id: 'app.lab.seed-map.elevation.surface', defaultMessage: 'Surface' },
    shallowUnderground: {
        id: 'app.lab.seed-map.elevation.shallow-underground',
        defaultMessage: 'Shallow underground',
    },
    deepUnderground: {
        id: 'app.lab.seed-map.elevation.deep-underground',
        defaultMessage: 'Deep underground',
    },
    go: { id: 'app.lab.seed-map.go', defaultMessage: 'Go' },
    layers: { id: 'app.lab.seed-map.layers', defaultMessage: 'Map layers' },
    structures: { id: 'app.lab.seed-map.mode.structures', defaultMessage: 'Structures' },
    ores: { id: 'app.lab.seed-map.mode.ores', defaultMessage: 'Ores' },
    oreLayers: { id: 'app.lab.seed-map.ore-layers', defaultMessage: 'Ore layers' },
    zoomToScanOres: {
        id: 'app.lab.seed-map.zoom-to-scan-ores',
        defaultMessage: 'Zoom in to scan ores',
    },
    noOresInEnd: {
        id: 'app.lab.seed-map.no-ores-in-end',
        defaultMessage: 'Ore scanning is not available in The End',
    },
    oresRequireModern: {
        id: 'app.lab.seed-map.ores-require-modern',
        defaultMessage: 'Ore prediction requires Java 1.18 or newer',
    },
    oreYRange: { id: 'app.lab.seed-map.ore-y-range', defaultMessage: 'Y range' },
    oreMinY: { id: 'app.lab.seed-map.ore-min-y', defaultMessage: 'Minimum Y' },
    oreMaxY: { id: 'app.lab.seed-map.ore-max-y', defaultMessage: 'Maximum Y' },
    resetYRange: { id: 'app.lab.seed-map.reset-y-range', defaultMessage: 'Reset Y range' },
    scanningOres: {
        id: 'app.lab.seed-map.scanning-ores',
        defaultMessage: 'Scanning ores {processed}/{total}',
    },
    oreScanFailed: {
        id: 'app.lab.seed-map.ore-scan-failed',
        defaultMessage: 'Ore scanning failed. Try refreshing the map.',
    },
    versionUnavailable: {
        id: 'app.lab.seed-map.version-unavailable',
        defaultMessage:
            'The engine cannot calculate this version. Choose a supported version to continue.',
    },
    oreTruncated: {
        id: 'app.lab.seed-map.ore-truncated',
        defaultMessage: 'Result limit reached. Select fewer ore types to see more positions.',
    },
    undergroundMode: { id: 'app.lab.seed-map.underground-mode', defaultMessage: 'Underground' },
    advancedTour: { id: 'app.lab.seed-map.advanced-tour', defaultMessage: 'Map exploration guide' },
    abandonedCamp: {
        id: 'app.lab.seed-map.feature.abandoned-camp',
        defaultMessage: 'Abandoned camp',
    },
    netherFossil: { id: 'app.lab.seed-map.feature.nether-fossil', defaultMessage: 'Nether fossil' },
    oreCandidate: {
        id: 'app.lab.seed-map.ore-candidate',
        defaultMessage: 'Predicted position; terrain and block replacement are unverified',
    },
    oreCoverage: {
        id: 'app.lab.seed-map.ore-coverage',
        defaultMessage: 'Y {min} to {max}',
    },
    mined: { id: 'app.lab.seed-map.mined', defaultMessage: 'Mined' },
    expandLayers: { id: 'app.lab.seed-map.expand-layers', defaultMessage: 'Expand layers' },
    collapseLayers: { id: 'app.lab.seed-map.collapse-layers', defaultMessage: 'Collapse layers' },
    showNames: { id: 'app.lab.seed-map.show-layer-names', defaultMessage: 'Show layer names' },
    hideNames: { id: 'app.lab.seed-map.hide-layer-names', defaultMessage: 'Hide layer names' },
    selectAll: { id: 'app.lab.seed-map.select-all', defaultMessage: 'Select all' },
    clear: { id: 'app.lab.seed-map.clear', defaultMessage: 'Clear' },
    reset: { id: 'app.lab.seed-map.reset', defaultMessage: 'Reset' },
    spawnPoint: { id: 'app.lab.seed-map.spawn-point', defaultMessage: 'Spawn point' },
    zoomToShow: { id: 'app.lab.seed-map.zoom-to-show', defaultMessage: 'Zoom in to show {layer}' },
    mapSettings: { id: 'app.lab.seed-map.settings', defaultMessage: 'Advanced settings' },
    showGrid: { id: 'app.lab.seed-map.show-grid', defaultMessage: 'Grid' },
    chunkCoordinates: {
        id: 'app.lab.seed-map.chunk-coordinates',
        defaultMessage: 'Chunk coordinates',
    },
    depth: { id: 'app.lab.seed-map.depth', defaultMessage: 'Map depth' },
    terrain: { id: 'app.lab.seed-map.terrain', defaultMessage: 'Terrain' },
    contours: { id: 'app.lab.seed-map.contours', defaultMessage: 'Contour lines' },
    zoomIn: { id: 'app.lab.seed-map.zoom-in', defaultMessage: 'Zoom in' },
    zoomOut: { id: 'app.lab.seed-map.zoom-out', defaultMessage: 'Zoom out' },
    fullscreen: { id: 'app.lab.seed-map.fullscreen', defaultMessage: 'Fullscreen' },
    exitFullscreen: { id: 'app.lab.seed-map.exit-fullscreen', defaultMessage: 'Exit fullscreen' },
    centerSpawn: { id: 'app.lab.seed-map.center-spawn', defaultMessage: 'Center spawn' },
    searchNearby: { id: 'app.lab.seed-map.search-nearby', defaultMessage: 'Nearest structure' },
    location: { id: 'app.lab.seed-map.location', defaultMessage: 'Location' },
    biomeTag: { id: 'app.lab.seed-map.biome-tag', defaultMessage: 'Biome' },
    copyTeleport: { id: 'app.lab.seed-map.copy-teleport', defaultMessage: 'Copy /tp' },
    teleportCopied: {
        id: 'app.lab.seed-map.teleport-copied',
        defaultMessage: 'Teleport command copied',
    },
    addMarker: { id: 'app.lab.seed-map.add-marker', defaultMessage: 'Add marker' },
    markerName: { id: 'app.lab.seed-map.marker-name', defaultMessage: 'Marker name' },
    markerColor: { id: 'app.lab.seed-map.marker-color', defaultMessage: 'Marker color' },
    markerSaved: { id: 'app.lab.seed-map.marker-saved', defaultMessage: 'Marker saved' },
    markers: { id: 'app.lab.seed-map.markers', defaultMessage: 'My markers' },
    noMarkers: { id: 'app.lab.seed-map.no-markers', defaultMessage: 'No saved markers' },
    removeMarker: { id: 'app.lab.seed-map.remove-marker', defaultMessage: 'Remove marker' },
    completed: { id: 'app.lab.seed-map.completed', defaultMessage: 'Completed' },
    ruler: { id: 'app.lab.seed-map.ruler', defaultMessage: 'Measure distance' },
    distance: { id: 'app.lab.seed-map.distance', defaultMessage: '{distance} blocks' },
    clearRuler: { id: 'app.lab.seed-map.clear-ruler', defaultMessage: 'Clear measurement' },
    share: { id: 'app.lab.seed-map.share', defaultMessage: 'Share' },
    shareCopied: { id: 'app.lab.seed-map.share-copied', defaultMessage: 'Map link copied' },
    approximate: { id: 'app.lab.seed-map.approximate', defaultMessage: 'Estimated' },
    mapScale: { id: 'app.lab.seed-map.map-scale', defaultMessage: '1 px = {scale} blocks' },
    copyright: {
        id: 'app.lab.seed-map.copyright.open',
        defaultMessage: 'Copyright and attribution',
    },
    loading: { id: 'app.lab.seed-map.loading', defaultMessage: 'Loading map' },
    mapFailed: { id: 'app.lab.seed-map.map-failed', defaultMessage: 'Map generation failed' },
    closePanel: { id: 'app.lab.seed-map.close-panel', defaultMessage: 'Close panel' },
    village: { id: 'app.lab.seed-map.feature.village', defaultMessage: 'Village' },
    outpost: { id: 'app.lab.seed-map.feature.outpost', defaultMessage: 'Pillager outpost' },
    shipwreck: { id: 'app.lab.seed-map.feature.shipwreck', defaultMessage: 'Shipwreck' },
    monument: { id: 'app.lab.seed-map.feature.monument', defaultMessage: 'Ocean monument' },
    mansion: { id: 'app.lab.seed-map.feature.mansion', defaultMessage: 'Woodland mansion' },
    ancientCity: { id: 'app.lab.seed-map.feature.ancient-city', defaultMessage: 'Ancient city' },
    trailRuins: { id: 'app.lab.seed-map.feature.trail-ruins', defaultMessage: 'Trail ruins' },
    trialChambers: {
        id: 'app.lab.seed-map.feature.trial-chambers',
        defaultMessage: 'Trial chambers',
    },
    ruinedPortal: { id: 'app.lab.seed-map.feature.ruined-portal', defaultMessage: 'Ruined portal' },
    stronghold: { id: 'app.lab.seed-map.feature.stronghold', defaultMessage: 'Stronghold' },
    slimeChunk: { id: 'app.lab.seed-map.feature.slime-chunk', defaultMessage: 'Slime chunks' },
    desertPyramid: {
        id: 'app.lab.seed-map.feature.desert-pyramid',
        defaultMessage: 'Desert pyramid',
    },
    jungleTemple: { id: 'app.lab.seed-map.feature.jungle-temple', defaultMessage: 'Jungle temple' },
    swampHut: { id: 'app.lab.seed-map.feature.swamp-hut', defaultMessage: 'Swamp hut' },
    igloo: { id: 'app.lab.seed-map.feature.igloo', defaultMessage: 'Igloo' },
    oceanRuin: { id: 'app.lab.seed-map.feature.ocean-ruin', defaultMessage: 'Ocean ruin' },
    buriedTreasure: {
        id: 'app.lab.seed-map.feature.buried-treasure',
        defaultMessage: 'Buried treasure',
    },
    mineshaft: { id: 'app.lab.seed-map.feature.mineshaft', defaultMessage: 'Mineshaft' },
    desertWell: { id: 'app.lab.seed-map.feature.desert-well', defaultMessage: 'Desert well' },
    geode: { id: 'app.lab.seed-map.feature.geode', defaultMessage: 'Amethyst geode' },
    fortress: { id: 'app.lab.seed-map.feature.fortress', defaultMessage: 'Nether fortress' },
    bastion: { id: 'app.lab.seed-map.feature.bastion', defaultMessage: 'Bastion remnant' },
    endCity: { id: 'app.lab.seed-map.feature.end-city', defaultMessage: 'End city' },
    endCityWithShip: {
        id: 'app.lab.seed-map.feature.end-city-with-ship',
        defaultMessage: 'End city (with ship)',
    },
    endCityWithoutShip: {
        id: 'app.lab.seed-map.feature.end-city-without-ship',
        defaultMessage: 'End city (without ship)',
    },
    endGateway: { id: 'app.lab.seed-map.feature.end-gateway', defaultMessage: 'End gateway' },
    oreEmerald: { id: 'app.lab.seed-map.ore.emerald', defaultMessage: 'Emerald ore' },
    oreQuartz: { id: 'app.lab.seed-map.ore.quartz', defaultMessage: 'Nether quartz ore' },
    oreNetherGold: { id: 'app.lab.seed-map.ore.nether_gold', defaultMessage: 'Nether gold ore' },
    oreAndesite: { id: 'app.lab.seed-map.ore.andesite', defaultMessage: 'Andesite' },
    oreDiorite: { id: 'app.lab.seed-map.ore.diorite', defaultMessage: 'Diorite' },
    oreGranite: { id: 'app.lab.seed-map.ore.granite', defaultMessage: 'Granite' },
    oreDirt: { id: 'app.lab.seed-map.ore.dirt', defaultMessage: 'Dirt' },
    oreGravel: { id: 'app.lab.seed-map.ore.gravel', defaultMessage: 'Gravel' },
    oreInfested: { id: 'app.lab.seed-map.ore.infested', defaultMessage: 'Infested stone' },
    oreDeepslate: { id: 'app.lab.seed-map.ore.deepslate', defaultMessage: 'Deepslate' },
    oreTuff: { id: 'app.lab.seed-map.ore.tuff', defaultMessage: 'Tuff' },
    oreClay: { id: 'app.lab.seed-map.ore.clay', defaultMessage: 'Clay' },
    oreBlackstone: { id: 'app.lab.seed-map.ore.blackstone', defaultMessage: 'Blackstone' },
    oreMagma: { id: 'app.lab.seed-map.ore.magma', defaultMessage: 'Magma block' },
    oreSoulSand: { id: 'app.lab.seed-map.ore.soul_sand', defaultMessage: 'Soul sand' },
    oreDiamond: { id: 'app.lab.seed-map.ore.diamond', defaultMessage: 'Diamond ore' },
    oreNetherite: { id: 'app.lab.seed-map.ore.netherite', defaultMessage: 'Ancient debris' },
    oreIron: { id: 'app.lab.seed-map.ore.iron', defaultMessage: 'Iron ore' },
    oreIronVein: { id: 'app.lab.seed-map.ore.iron-vein', defaultMessage: 'Iron vein' },
    oreCopper: { id: 'app.lab.seed-map.ore.copper', defaultMessage: 'Copper ore' },
    oreCopperVein: { id: 'app.lab.seed-map.ore.copper-vein', defaultMessage: 'Copper vein' },
    oreGold: { id: 'app.lab.seed-map.ore.gold', defaultMessage: 'Gold ore' },
    oreRedstone: { id: 'app.lab.seed-map.ore.redstone', defaultMessage: 'Redstone ore' },
    oreLapis: { id: 'app.lab.seed-map.ore.lapis', defaultMessage: 'Lapis lazuli ore' },
    oreCoal: { id: 'app.lab.seed-map.ore.coal', defaultMessage: 'Coal ore' },
})

const featureMessages: Record<SeedMapFeatureKind, (typeof messages)[keyof typeof messages]> = {
    village: messages.village,
    outpost: messages.outpost,
    shipwreck: messages.shipwreck,
    monument: messages.monument,
    mansion: messages.mansion,
    'ancient-city': messages.ancientCity,
    'trail-ruins': messages.trailRuins,
    'trial-chambers': messages.trialChambers,
    'ruined-portal': messages.ruinedPortal,
    stronghold: messages.stronghold,
    'slime-chunk': messages.slimeChunk,
    'desert-pyramid': messages.desertPyramid,
    'jungle-temple': messages.jungleTemple,
    'swamp-hut': messages.swampHut,
    igloo: messages.igloo,
    'ocean-ruin': messages.oceanRuin,
    'buried-treasure': messages.buriedTreasure,
    mineshaft: messages.mineshaft,
    'desert-well': messages.desertWell,
    geode: messages.geode,
    fortress: messages.fortress,
    bastion: messages.bastion,
    'end-city': messages.endCity,
    'end-gateway': messages.endGateway,
    'abandoned-camp': messages.abandonedCamp,
    'nether-fossil': messages.netherFossil,
}

const oreMessages: Record<SeedMapOreKind, (typeof messages)[keyof typeof messages]> = {
    emerald: messages.oreEmerald,
    quartz: messages.oreQuartz,
    nether_gold: messages.oreNetherGold,
    andesite: messages.oreAndesite,
    diorite: messages.oreDiorite,
    granite: messages.oreGranite,
    dirt: messages.oreDirt,
    gravel: messages.oreGravel,
    infested: messages.oreInfested,
    deepslate: messages.oreDeepslate,
    tuff: messages.oreTuff,
    clay: messages.oreClay,
    blackstone: messages.oreBlackstone,
    magma: messages.oreMagma,
    soul_sand: messages.oreSoulSand,
    diamond: messages.oreDiamond,
    netherite: messages.oreNetherite,
    iron: messages.oreIron,
    iron_vein: messages.oreIronVein,
    copper: messages.oreCopper,
    copper_vein: messages.oreCopperVein,
    gold: messages.oreGold,
    redstone: messages.oreRedstone,
    lapis: messages.oreLapis,
    coal: messages.oreCoal,
}

const editionOptions: SeedMapEdition[] = ['java', 'java-large-biomes']
const elevationOptions: SeedMapElevation[] = [...SEED_MAP_ELEVATIONS]
const elevationLabels: Record<SeedMapElevation, MessageDescriptor> = {
    128: messages.surface,
    16: messages.shallowUnderground,
    '-32': messages.deepUnderground,
}

function elevationLabel(elevation: SeedMapElevation) {
    return `${formatMessage(elevationLabels[elevation])} (Y=${elevation})`
}

function setElevation(value: unknown) {
    const elevation = Number(value) as SeedMapElevation
    if (elevationOptions.includes(elevation)) workspace.elevation = elevation
}

const selectedProfile = computed(() =>
    profiles.value.find(
        (profile) =>
            profile.edition === workspace.edition && profile.version === workspace.gameVersion,
    ),
)
const availableVersions = computed(() =>
    profiles.value
        .filter((profile) => profile.edition === workspace.edition)
        .map((profile) => profile.version)
        .sort(compareSeedMapVersions),
)
const dimensions = computed(
    () => selectedProfile.value?.dimensions ?? ['overworld', 'nether', 'end'],
)
const scale = computed(() => 4 ** workspace.zoom)
const poiIconScale = computed(() => (scale.value < 4 ? 1.5 : 1))
const tileLevel = computed(() => seedMapTileLevel(scale.value))
const tileScale = computed(() => tileLevel.value.scale)
const undergroundMask = computed(
    () => selectedProfile.value?.undergroundMasks?.[dimensionsIndex.value] ?? 0,
)
const undergroundSupported = computed(
    () =>
        !!selectedProfile.value?.available &&
        !!(undergroundMask.value & 1) &&
        workspace.dimension !== 'end',
)
const focusedPiece = ref<SeedMapStructurePiece | null>(null)
const layoutRevision = ref(0)
const layoutLayer = createSeedMapLayoutLayer(getSeedMapStructureDetails, () => {
    layoutRevision.value++
    redraw()
})
let layoutRefreshTimer: ReturnType<typeof setTimeout> | undefined
const replayOnboarding = inject<(mode: 'seed-map') => Promise<void>>('replayOnboarding')
const structureQuery = computed<SeedMapStructureQuery | null>(() =>
    selection.value?.feature
        ? {
              seed: workspace.seed,
              edition: workspace.edition,
              version: workspace.gameVersion,
              dimension: workspace.dimension,
              x: selection.value.feature.x,
              z: selection.value.feature.z,
              featureMask: featureMask([selection.value.feature.kind]),
          }
        : null,
)
function closeStructureDetails() {
    selection.value = null
    focusedPiece.value = null
}
function focusStructurePiece(piece: SeedMapStructurePiece) {
    focusedPiece.value = piece
    const min = piece.min ?? piece.position
    const max = piece.max ?? piece.position
    workspace.center = { x: (min.x + max.x) / 2, z: (min.z + max.z) / 2 }
    redraw()
}
const mapScaleLabel = computed(() =>
    scale.value >= 10
        ? String(Math.round(scale.value))
        : scale.value.toFixed(2).replace(/\.00$/, ''),
)
const dimensionsIndex = computed(() =>
    workspace.dimension === 'nether' ? 1 : workspace.dimension === 'end' ? 2 : 0,
)
const dimensionFeatures = computed(() =>
    SEED_MAP_FEATURES.filter(
        (feature) =>
            feature.dimensions.includes(workspace.dimension) &&
            (selectedProfile.value?.featureMasks === undefined ||
                (selectedProfile.value.featureMasks[dimensionsIndex.value] & feature.mask) !== 0),
    ),
)
const requestedFeatures = computed(() =>
    visibleFeatureDefinitions(
        workspace.visibleFeatures,
        workspace.dimension,
        tileScale.value,
    ).filter((feature) => dimensionFeatures.value.includes(feature)),
)
const requestedFeatureKinds = computed(
    () => new Set(requestedFeatures.value.map((feature) => feature.kind)),
)
const displayedFeatures = computed(() =>
    features.value.filter((feature) => requestedFeatureKinds.value.has(feature.kind)),
)
const visibleLayoutQueries = computed(() => {
    if (workspace.displayMode !== 'structures' || scale.value > SEED_MAP_LAYOUT_MAX_SCALE) return []
    const bounds = mapBounds()
    return displayedFeatures.value
        .filter(
            (feature) =>
                feature.kind !== 'slime-chunk' &&
                feature.x >= bounds.minX - 256 &&
                feature.x <= bounds.maxX + 256 &&
                feature.z >= bounds.minZ - 256 &&
                feature.z <= bounds.maxZ + 256,
        )
        .sort(
            (a, b) =>
                Math.hypot(a.x - workspace.center.x, a.z - workspace.center.z) -
                Math.hypot(b.x - workspace.center.x, b.z - workspace.center.z),
        )
        .map((feature) => ({
            feature,
            query: {
                seed: workspace.seed,
                edition: workspace.edition,
                version: workspace.gameVersion,
                dimension: workspace.dimension,
                x: feature.x,
                z: feature.z,
                featureMask: featureMask([feature.kind]),
            },
        }))
})
const projectedStructures = computed(() => {
    const currentRevision = layoutRevision.value
    return visibleLayoutQueries.value.flatMap(({ feature, query }) => {
        if (currentRevision < 0) return []
        const pieces = seedMapLayoutPieces(layoutLayer.peek(query) ?? null)
        return pieces.length ? [{ feature, pieces }] : []
    })
})
const projectedFeatureKeys = computed(
    () => new Set(projectedStructures.value.map(({ feature }) => featureKey(feature))),
)
function structureBadges(size: { width: number; height: number }) {
    return seedMapStructureBadgePositions(
        projectedStructures.value.map(({ feature, pieces }) => {
            const min = worldToScreen(
                Math.min(...pieces.map((piece) => piece.min!.x)),
                Math.min(...pieces.map((piece) => piece.min!.z)),
            )
            const max = worldToScreen(
                Math.max(...pieces.map((piece) => piece.max!.x + 1)),
                Math.max(...pieces.map((piece) => piece.max!.z + 1)),
            )
            return { id: featureKey(feature), minX: min.x, minY: min.y, maxX: max.x, maxY: max.y }
        }),
        size,
    ).map((badge) => ({
        ...badge,
        feature: projectedStructures.value.find(({ feature }) => featureKey(feature) === badge.id)!
            .feature,
    }))
}
function selectStructureBadge(feature: SeedMapFeature) {
    selection.value = { x: feature.x, z: feature.z, feature }
    focusedPiece.value = null
    markerDraftOpen.value = false
    redraw()
}
watch(visibleLayoutQueries, () => {
    clearTimeout(layoutRefreshTimer)
    layoutRefreshTimer = setTimeout(
        () => layoutLayer.update(visibleLayoutQueries.value.map((entry) => entry.query)),
        120,
    )
})
onBeforeUnmount(() => {
    if (canvas.value) {
        canvas.value.width = 1
        canvas.value.height = 1
    }
})

onUnmounted(() => {
    clearTimeout(layoutRefreshTimer)
    layoutLayer.dispose()
})
const activeFeatureMask = computed(() =>
    featureMask(requestedFeatures.value.map((feature) => feature.kind)),
)
const oreModeSupported = computed(() => selectedProfile.value?.ores !== false)
const dimensionOres = computed(() =>
    oreModeSupported.value
        ? seedMapOresForDimension(workspace.dimension).filter(
              (ore) =>
                  !selectedProfile.value?.oreKinds ||
                  selectedProfile.value.oreKinds.includes(ore.kind),
          )
        : [],
)
const activeSelectedOres = computed(() => {
    const available = new Set(dimensionOres.value.map((ore) => ore.kind))
    return workspace.selectedOres.filter((ore) => available.has(ore))
})
const activeOreYRange = computed(() => seedMapOreYRange(activeSelectedOres.value))
const oreYMinimum = computed({
    get: () => workspace.oreYMin ?? activeOreYRange.value[0],
    set: (value: number) => {
        workspace.oreYMin = Math.min(
            Math.round(value),
            workspace.oreYMax ?? activeOreYRange.value[1],
        )
    },
})
const oreYMaximum = computed({
    get: () => workspace.oreYMax ?? activeOreYRange.value[1],
    set: (value: number) => {
        workspace.oreYMax = Math.max(
            Math.round(value),
            workspace.oreYMin ?? activeOreYRange.value[0],
        )
    },
})
const oreZoomLimited = computed(
    () => workspace.displayMode === 'ores' && scale.value > SEED_MAP_ORE_MAX_SCALE,
)
const mapBusy = computed(() => loading.value || oreScanning.value)
const availableBiomes = computed(() =>
    SEED_MAP_BIOMES.filter(
        (biome) =>
            biome.dimensions.includes(workspace.dimension) &&
            (!selectedProfile.value?.biomes || selectedProfile.value.biomes.includes(biome.id)),
    ).map((biome) => biome.id),
)
const activeHighlightedBiomes = computed(() =>
    workspace.highlightedBiomes.filter((biome) => availableBiomes.value.includes(biome)),
)
const terrainSupported = computed(
    () => workspace.dimension !== 'nether' && workspace.displayMode !== 'underground',
)
const terrainEnabled = computed({
    get: () => terrainSupported.value && workspace.terrainEstimation,
    set: (enabled: boolean) => {
        if (!terrainSupported.value) return
        workspace.terrainEstimation = enabled
        if (!enabled) workspace.contourLines = false
    },
})
const contoursEnabled = computed({
    get: () => workspace.contourLines,
    set: (enabled: boolean) => {
        workspace.contourLines = enabled
        if (enabled) workspace.terrainEstimation = true
    },
})
const selectedFeatureKey = computed(() =>
    selection.value?.feature ? featureKey(selection.value.feature) : null,
)
const selectedFeatureCompleted = computed({
    get: () => {
        const key = selectedFeatureKey.value
        return key !== null && workspace.completedFeatures.includes(key)
    },
    set: (completed: boolean) => {
        const key = selectedFeatureKey.value
        if (!key) return
        workspace.completedFeatures = completed
            ? [...new Set([...workspace.completedFeatures, key])]
            : workspace.completedFeatures.filter((item) => item !== key)
    },
})
const selectedOreKey = computed(() =>
    selection.value?.ore ? seedMapOreKey(selection.value.ore) : null,
)
const selectedOreCompleted = computed({
    get: () => {
        const key = selectedOreKey.value
        return key !== null && workspace.completedOres.includes(key)
    },
    set: (completed: boolean) => {
        const key = selectedOreKey.value
        if (!key) return
        workspace.completedOres = completed
            ? [...new Set([...workspace.completedOres, key])]
            : workspace.completedOres.filter((item) => item !== key)
    },
})
const dimensionMarkers = computed(() =>
    workspace.markers.filter((marker) => marker.dimension === workspace.dimension),
)
const selectedMarker = computed(() =>
    selection.value?.markerId
        ? (dimensionMarkers.value.find((marker) => marker.id === selection.value?.markerId) ?? null)
        : null,
)
const rulerDistance = computed(() => {
    if (rulerPoints.value.length !== 2) return null
    const [start, end] = rulerPoints.value
    return Math.round(Math.hypot(end.x - start.x, end.z - start.z))
})
const selectionStyle = computed(() => {
    if (!selection.value) return undefined
    const point = worldToScreen(selection.value.x, selection.value.z)
    return {
        left: `${Math.min(Math.max(point.x + 14, 8), Math.max(viewport.width - 286, 8))}px`,
        top: `${Math.min(Math.max(point.y + 14, 8), Math.max(viewport.height - 226, 8))}px`,
    }
})
const selectionPointStyle = computed(() => {
    const current = selection.value
    if (!current || current.feature || current.ore || current.markerId || current.spawn)
        return undefined
    const point = worldToScreen(current.x, current.z)
    return { left: `${point.x}px`, top: `${point.y}px` }
})
const selectionVisible = computed(() => {
    if (!selection.value) return false
    if (selection.value.feature && !requestedFeatureKinds.value.has(selection.value.feature.kind))
        return false
    if (selection.value.ore && workspace.displayMode !== 'ores') return false
    const point = worldToScreen(selection.value.x, selection.value.z)
    return (
        point.x >= -24 &&
        point.x <= viewport.width + 24 &&
        point.y >= -24 &&
        point.y <= viewport.height + 24
    )
})

watch(
    () => JSON.stringify(structureQuery.value),
    () => {
        layoutLayer.select(structureQuery.value)
        focusedPiece.value = null
        redraw()
    },
)
watch(
    () => [profilesResolved.value, undergroundSupported.value, workspace.displayMode],
    () => {
        if (
            profilesResolved.value &&
            workspace.displayMode === 'underground' &&
            !undergroundSupported.value
        )
            workspace.displayMode = 'structures'
    },
    { immediate: true, flush: 'sync' },
)
watch(
    workspace,
    () => {
        if (saveTimer) window.clearTimeout(saveTimer)
        saveTimer = window.setTimeout(() => saveSeedMapWorkspace(workspace), 350)
    },
    { deep: true },
)

watch(
    () => workspace.seed,
    (seed, previousSeed) => {
        if (seed === previousSeed) return
        stashActiveProgress()
        activeHistoryId = seedMapHistoryId(seed.trim().slice(0, 256), workspace.edition)
        const entry = historyEntries.value.find((item) => item.id === activeHistoryId)
        workspace.completedFeatures = entry ? [...entry.completedFeatures] : []
        workspace.completedOres = entry ? [...entry.completedOres] : []
        const source = pendingHistorySource ?? 'manual'
        const labels = pendingHistoryLabels
        pendingHistorySource = null
        pendingHistoryLabels = {}
        if (historyTimer) window.clearTimeout(historyTimer)
        if (seed.trim()) {
            if (source === 'manual') {
                historyTimer = window.setTimeout(() => {
                    historyTimer = undefined
                    recordHistory('manual')
                }, 2_000)
            } else {
                recordHistory(source, labels)
            }
        }
        if (applyingSharedState) {
            spawn.value = null
            spawnContextKey = ''
            features.value = []
            return
        }
        const defaults = createDefaultSeedMapWorkspace()
        centerOnNextSpawn = true
        workspace.dimension = 'overworld'
        workspace.center = { x: 0, z: 0 }
        workspace.zoom = defaults.zoom
        workspace.showSpawn = true
        workspace.visibleFeatures = [...defaults.visibleFeatures]
        spawn.value = null
        spawnContextKey = ''
        features.value = []
    },
)

watch(
    () => route.query,
    (query) => {
        if (typeof query.seed !== 'string' || !query.seed.trim()) return
        const shared = applyShareQuery(query, {
            ...createDefaultSeedMapWorkspace(),
            markers: [...workspace.markers],
        })
        applyingSharedState = true
        pendingHistorySource = 'share'
        const seedChanged = workspace.seed !== shared.seed
        workspace.edition = shared.edition
        workspace.gameVersion = shared.gameVersion
        workspace.dimension = shared.dimension
        workspace.displayMode = shared.displayMode
        workspace.undergroundY = shared.undergroundY
        workspace.undergroundLayers = shared.undergroundLayers
        workspace.selectedOres = [...shared.selectedOres]
        workspace.oreYMin = shared.oreYMin
        workspace.oreYMax = shared.oreYMax
        workspace.zoom = shared.zoom
        workspace.center = { ...shared.center }
        workspace.seed = shared.seed
        selection.value = null
        centerOnNextSpawn = false
        if (!seedChanged) {
            pendingHistorySource = null
            recordHistory('share')
        }
        void nextTick(() => {
            applyingSharedState = false
            scheduleRefresh()
        })
    },
)

watch(
    () => [workspace.completedFeatures, workspace.completedOres],
    () => {
        if (progressTimer) window.clearTimeout(progressTimer)
        progressTimer = window.setTimeout(() => {
            progressTimer = undefined
            stashActiveProgress()
        }, 600)
    },
    { deep: true },
)

watch(
    () => workspace.displayMode,
    () => {
        void nextTick(resizeCanvas)
    },
)

watch(
    () => [
        workspace.seed,
        workspace.edition,
        workspace.gameVersion,
        workspace.dimension,
        workspace.displayMode,
        workspace.undergroundY,
        workspace.undergroundLayers,
        tileScale.value,
        workspace.elevation,
        workspace.terrainEstimation,
        workspace.contourLines,
        workspace.highlightBiomeEnabled,
        workspace.highlightedBiomes.join('|'),
        workspace.showSpawn,
        workspace.visibleFeatures.join('|'),
        workspace.selectedOres.join('|'),
    ],
    () => scheduleRefresh(),
    { flush: 'post' },
)

watch(
    [
        () => workspace.showGrid,
        () => workspace.showChunkCoordinates,
        () => workspace.markers,
        () => workspace.completedFeatures,
        () => workspace.completedOres,
        showLayerNames,
        rulerPoints,
    ],
    () => redraw(),
    { deep: true, flush: 'post' },
)

watch(
    () => [workspace.oreYMin, workspace.oreYMax] as const,
    ([yMin, yMax]) => refreshOreFilter(yMin, yMax),
    { flush: 'post' },
)

watch(
    activeOreYRange,
    ([min, max]) => {
        if (workspace.oreYMin !== null)
            workspace.oreYMin = Math.min(max, Math.max(min, workspace.oreYMin))
        if (workspace.oreYMax !== null)
            workspace.oreYMax = Math.min(max, Math.max(min, workspace.oreYMax))
    },
    { flush: 'post' },
)

watch(
    () => workspace.edition,
    () => {
        const profile = profiles.value.find((item) => item.edition === workspace.edition)
        if (!profile) return
        workspace.gameVersion = profile.version
        workspace.dimension = profile.dimensions.includes(workspace.dimension)
            ? workspace.dimension
            : 'overworld'
    },
)

watch(
    () => workspace.gameVersion,
    () => {
        const profile = selectedProfile.value
        if (profile && !profile.dimensions.includes(workspace.dimension))
            workspace.dimension = 'overworld'
        ensureDimensionOreSelection()
    },
)

watch(oreModeSupported, (supported) => {
    if (!supported && workspace.displayMode === 'ores') workspace.displayMode = 'structures'
})

watch(
    () => workspace.dimension,
    (dimension) => {
        if (dimension !== 'overworld') centerOnNextSpawn = false
        if (dimension === 'nether') workspace.contourLines = false
        ensureDimensionOreSelection()
        resetOreYRange()
        selection.value = null
    },
)

watch(
    () => workspace.showSpawn,
    (showSpawn) => {
        if (!showSpawn) centerOnNextSpawn = false
    },
)

watch(
    () => [workspace.center.x, workspace.center.z],
    ([x, z]) => {
        coordinateX.value = String(Math.round(x))
        coordinateZ.value = String(Math.round(z))
    },
)

onMounted(async () => {
    await nextTick()
    if (disposed) return
    preloadMapIcons()
    resizeCanvas()
    resizeObserver = new ResizeObserver(resizeCanvas)
    if (canvas.value) resizeObserver.observe(canvas.value)
    document.addEventListener('fullscreenchange', onFullscreenChange)
    document.addEventListener('contextmenu', captureAreaContextMenu, true)
    profiles.value = await getSeedMapProfiles().catch(() => fallbackSeedMapProfiles())
    if (disposed) return
    profilesResolved.value = true
    ensureProfile()
    if (typeof route.query.seed === 'string' && route.query.seed.trim()) {
        recordHistory('share')
    }
    scheduleRefresh(0)
})

onUnmounted(() => {
    disposed = true
    requestEpoch++
    if (historyTimer) {
        window.clearTimeout(historyTimer)
        historyTimer = undefined
        recordHistory('manual')
    }
    if (progressTimer) {
        window.clearTimeout(progressTimer)
        progressTimer = undefined
    }
    stashActiveProgress()
    if (refreshTimer) window.clearTimeout(refreshTimer)
    if (saveTimer) window.clearTimeout(saveTimer)
    if (longPressTimer) window.clearTimeout(longPressTimer)
    if (wheelTimer) window.clearTimeout(wheelTimer)
    if (trackpadTimer) window.clearTimeout(trackpadTimer)
    if (redrawFrame !== undefined) window.cancelAnimationFrame(redrawFrame)
    if (panFrame !== undefined) window.cancelAnimationFrame(panFrame)
    if (zoomAnimationFrame !== undefined) window.cancelAnimationFrame(zoomAnimationFrame)
    saveSeedMapWorkspace(workspace)
    disposeOreLayer()
    tileLayer.dispose()
    featureImages.clear()
    oreImages.clear()
    spawnImage = undefined
    resizeObserver?.disconnect()
    document.removeEventListener('fullscreenchange', onFullscreenChange)
    document.removeEventListener('contextmenu', captureAreaContextMenu, true)
    if (areaContextMenuTimer) clearTimeout(areaContextMenuTimer)
})

function ensureProfile() {
    if (selectedProfile.value) return
    profiles.value.push({
        edition: workspace.edition,
        version: workspace.gameVersion,
        available: false,
        dimensions: [workspace.dimension],
        ores: false,
    })
}

function editionLabel(edition: SeedMapEdition) {
    if (edition === 'java-large-biomes') return formatMessage(messages.javaLargeBiomes)
    return formatMessage(messages.java)
}

function dimensionLabel(dimension: SeedMapDimension) {
    return formatMessage(messages[dimension])
}

function featureLabel(kind: SeedMapFeatureKind) {
    return formatMessage(featureMessages[kind])
}

function featureResultLabel(feature: SeedMapFeature) {
    if (feature.kind !== 'end-city' || feature.endShip === undefined) {
        return featureLabel(feature.kind)
    }
    return formatMessage(feature.endShip ? messages.endCityWithShip : messages.endCityWithoutShip)
}

function featureImageKey(feature: SeedMapFeature) {
    if (feature.kind !== 'end-city' || feature.endShip === undefined) return feature.kind
    return `end-city:${feature.endShip ? 'ship' : 'no-ship'}`
}

function oreLabel(kind: SeedMapOreKind) {
    return formatMessage(oreMessages[kind])
}

function setDisplayMode(mode: SeedMapDisplayMode) {
    if (mode === 'ores' && !oreModeSupported.value) return
    if (mode === 'underground' && !undergroundSupported.value) return
    if (mode === 'ores' && workspace.dimension === 'end') workspace.dimension = 'overworld'
    workspace.displayMode = mode
    ensureDimensionOreSelection()
    selection.value = null
}

function ensureDimensionOreSelection() {
    if (dimensionOres.value.length === 0 || activeSelectedOres.value.length > 0) return
    const first = dimensionOres.value[0]?.kind
    if (first) workspace.selectedOres = [...new Set([...workspace.selectedOres, first])]
}

function featureTooltip(kind: SeedMapFeatureKind, maxScale: number) {
    const label = featureLabel(kind)
    return tileScale.value > maxScale ? formatMessage(messages.zoomToShow, { layer: label }) : label
}

function featureImageSource(kind: SeedMapFeatureKind) {
    return featureImageSources[kind]
}

const itemImageSources = ref<Record<string, string>>({})
function oreImageSource(source: string) {
    return itemImageSources.value[source] ?? (source.startsWith('minecraft:') ? undefined : source)
}

function preloadMapIcons() {
    const preloadFeatureImage = (key: string, source: string) => {
        const image = new Image()
        image.onload = () => {
            if (disposed) return
            featureImages.set(key, image)
            redraw()
        }
        image.src = source
    }
    for (const [kind, source] of Object.entries(featureImageSources) as [
        SeedMapFeatureKind,
        string,
    ][]) {
        preloadFeatureImage(kind, source)
    }
    preloadFeatureImage('end-city:ship', endCityImageSources.ship)
    preloadFeatureImage('end-city:no-ship', endCityImageSources.noShip)
    for (const ore of seedMapOresForDimension('overworld').concat(
        seedMapOresForDimension('nether'),
    )) {
        for (const [variant, source] of [
            ['stone', ore.texture],
            ['deepslate', ore.deepslateTexture],
        ] as const) {
            if (!source) continue
            const image = new Image()
            image.onload = () => {
                if (disposed) return
                oreImages.set(`${ore.kind}:${variant}`, image)
                redraw()
            }
            if (source.startsWith('minecraft:')) {
                void loadMinecraftItemImage(source)
                    .then((loaded) => {
                        if (disposed) return
                        itemImageSources.value[source] = loaded.src
                        image.src = loaded.src
                    })
                    .catch(() => {})
            } else image.src = source
        }
    }
    const image = new Image()
    image.onload = () => {
        if (disposed) return
        spawnImage = image
        redraw()
    }
    image.src = `${structureAssetRoot}/spawn_point.webp`
}

function resizeCanvas() {
    const element = canvas.value
    if (!element) return
    if (workspace.displayMode === 'underground') {
        element.width = 1
        element.height = 1
        return
    }
    const bounds = element.getBoundingClientRect()
    viewport.width = Math.max(1, Math.round(bounds.width))
    viewport.height = Math.max(1, Math.round(bounds.height))
    const ratio = window.devicePixelRatio || 1
    const width = Math.round(viewport.width * ratio)
    const height = Math.round(viewport.height * ratio)
    if (element.width === width && element.height === height) return
    element.width = width
    element.height = height
    element.getContext('2d')?.setTransform(ratio, 0, 0, ratio, 0, 0)
    drawMap()
    scheduleRefresh()
}

function mapBounds(): MapBounds {
    const halfWidth = (viewport.width * scale.value) / 2
    const halfHeight = (viewport.height * scale.value) / 2
    return {
        minX: Math.floor(workspace.center.x - halfWidth),
        minZ: Math.floor(workspace.center.z - halfHeight),
        maxX: Math.ceil(workspace.center.x + halfWidth),
        maxZ: Math.ceil(workspace.center.z + halfHeight),
    }
}

/**
 * Floating-point bounds used for canvas transforms. Tile/feature requests
 * still use the integer bounds above, but rounding those bounds for drawing
 * makes the map jump by a pixel while a zoom animation is in progress.
 */
function viewBounds() {
    const halfWidth = (viewport.width * scale.value) / 2
    const halfHeight = (viewport.height * scale.value) / 2
    return {
        minX: workspace.center.x - halfWidth,
        minZ: workspace.center.z - halfHeight,
    }
}

function worldToScreen(x: number, z: number) {
    const bounds = viewBounds()
    return { x: (x - bounds.minX) / scale.value, y: (z - bounds.minZ) / scale.value }
}

function screenToWorld(x: number, y: number) {
    const bounds = viewBounds()
    return {
        x: Math.round(bounds.minX + x * scale.value),
        z: Math.round(bounds.minZ + y * scale.value),
    }
}

function tileView(): SeedMapTileView {
    return {
        epoch: requestEpoch,
        context: {
            seed: workspace.seed,
            edition: workspace.edition,
            version: workspace.gameVersion,
            dimension: workspace.dimension,
            elevation: workspace.elevation,
            terrain: terrainEnabled.value,
            contours: contoursEnabled.value,
            highlightBiomes: workspace.highlightBiomeEnabled
                ? [...activeHighlightedBiomes.value]
                : undefined,
        },
        bounds: mapBounds(),
        center: { ...workspace.center },
        level: { ...tileLevel.value },
        concurrency: seedMapTileConcurrency(tileScale.value),
    }
}

let overlaysEpoch = -1
const tileLayer = createSeedMapTileLayer({
    render: renderSeedMapTile,
    onUpdate: () => redraw(),
    onLoading: (value) => {
        loading.value = value
    },
    onError: (message) => {
        mapError.value = message
    },
    onCoverage: (epoch) => {
        if (epoch !== requestEpoch || epoch === overlaysEpoch) return
        overlaysEpoch = epoch
        if (workspace.displayMode === 'structures') void requestFeatures(epoch)
        refreshCurrentOreLayer()
    },
})

function redraw() {
    if (disposed || workspace.displayMode === 'underground' || redrawFrame !== undefined) return
    redrawFrame = window.requestAnimationFrame(() => {
        redrawFrame = undefined
        drawMap()
    })
}

function drawMap() {
    if (disposed || workspace.displayMode === 'underground') return
    const element = canvas.value
    const context = element?.getContext('2d')
    if (!element || !context || !viewport.width || !viewport.height) return
    const style = getComputedStyle(element)
    const surface2 = style.getPropertyValue('--surface-2').trim() || '#17191D'
    const surface3 = style.getPropertyValue('--surface-3').trim() || '#23262D'
    context.clearRect(0, 0, viewport.width, viewport.height)
    context.fillStyle = surface2
    context.fillRect(0, 0, viewport.width, viewport.height)
    context.fillStyle = surface3
    for (let y = 0; y < viewport.height; y += 32) {
        for (let x = 0; x < viewport.width; x += 32) {
            if ((x / 32 + y / 32) % 2 === 0) context.fillRect(x, y, 32, 32)
        }
    }

    const bounds = mapBounds()
    context.save()
    context.imageSmoothingEnabled = false
    for (const tile of tileLayer.drawable(tileView())) {
        const position = worldToScreen(tile.x, tile.z)
        const size = tile.span / scale.value
        context.drawImage(tile.bitmap, position.x, position.y, size, size)
    }
    context.restore()

    if (workspace.showGrid || workspace.showChunkCoordinates) drawGrid(context, bounds)
    if (workspace.displayMode === 'structures') drawFeatures(context)
    else if (workspace.displayMode === 'ores') drawOres(context)
    for (const { query } of visibleLayoutQueries.value) {
        drawSeedMapLayout(
            context,
            seedMapLayoutPieces(layoutLayer.peek(query) ?? null),
            scale.value,
            worldToScreen,
            JSON.stringify(query) === JSON.stringify(structureQuery.value)
                ? focusedPiece.value?.id
                : undefined,
        )
    }
    if (focusedPiece.value && structureQuery.value) {
        const piece = focusedPiece.value
        const min = piece.min ?? piece.position
        const max = piece.max ?? piece.position
        const position = worldToScreen(min.x, min.z)
        context.save()
        context.strokeStyle = seedMapLayoutPalette(element).selected
        context.lineWidth = 3
        context.strokeRect(
            position.x,
            position.y,
            Math.max(8, (max.x - min.x + 1) / scale.value),
            Math.max(8, (max.z - min.z + 1) / scale.value),
        )
        context.restore()
    }
    drawSpawn(context)
    drawMarkers(context)
    drawRuler(context)
}

function drawGrid(context: CanvasRenderingContext2D, bounds: MapBounds) {
    const step = Math.max(16, 16 * Math.ceil(scale.value / 16))
    const firstX = Math.floor(bounds.minX / step) * step
    const firstZ = Math.floor(bounds.minZ / step) * step
    context.save()
    context.strokeStyle = 'rgba(255, 255, 255, 0.24)'
    context.lineWidth = 1
    context.font = '11px system-ui'
    context.fillStyle = 'rgba(255, 255, 255, 0.84)'
    for (let x = firstX; x <= bounds.maxX; x += step) {
        const screen = worldToScreen(x, bounds.minZ)
        context.beginPath()
        context.moveTo(screen.x, 0)
        context.lineTo(screen.x, viewport.height)
        context.stroke()
        if (workspace.showChunkCoordinates)
            context.fillText(String(Math.floor(x / 16)), screen.x + 4, 14)
    }
    for (let z = firstZ; z <= bounds.maxZ; z += step) {
        const screen = worldToScreen(bounds.minX, z)
        context.beginPath()
        context.moveTo(0, screen.y)
        context.lineTo(viewport.width, screen.y)
        context.stroke()
        if (workspace.showChunkCoordinates)
            context.fillText(String(Math.floor(z / 16)), 4, screen.y - 4)
    }
    context.restore()
}

function drawFeatures(context: CanvasRenderingContext2D) {
    for (const feature of displayedFeatures.value) {
        if (projectedFeatureKeys.value.has(featureKey(feature))) continue
        const point = worldToScreen(feature.x, feature.z)
        if (
            point.x < -28 ||
            point.x > viewport.width + 28 ||
            point.y < -28 ||
            point.y > viewport.height + 28
        )
            continue
        const selected = selectedFeatureKey.value === featureKey(feature)
        const completed = workspace.completedFeatures.includes(featureKey(feature))
        context.save()
        context.globalAlpha = completed ? 0.48 : 1
        if (feature.kind === 'slime-chunk') {
            const size = Math.max(5, Math.min(16 / scale.value, 14))
            context.fillStyle = featureColors[feature.kind]
            context.strokeStyle = selected ? '#FFFFFF' : 'rgba(15, 18, 20, 0.86)'
            context.lineWidth = selected ? 3 : 2
            context.beginPath()
            context.rect(point.x - size / 2, point.y - size / 2, size, size)
            context.fill()
            context.stroke()
        } else {
            const icon = featureImages.get(featureImageKey(feature))
            if (icon) {
                const size = (selected ? 32 : 28) * poiIconScale.value
                context.shadowColor = 'rgba(0, 0, 0, 0.48)'
                context.shadowBlur = 5
                context.shadowOffsetY = 2
                context.drawImage(icon, point.x - size / 2, point.y - size / 2, size, size)
                if (selected) {
                    context.shadowColor = 'transparent'
                    context.strokeStyle = '#FFFFFF'
                    context.lineWidth = 2
                    context.beginPath()
                    context.arc(point.x, point.y, size / 2 + 2, 0, Math.PI * 2)
                    context.stroke()
                }
            } else {
                const radius = (selected ? 10 : 8) * poiIconScale.value
                context.fillStyle = featureColors[feature.kind]
                context.strokeStyle = selected ? '#FFFFFF' : 'rgba(15, 18, 20, 0.86)'
                context.lineWidth = selected ? 3 : 2
                context.beginPath()
                context.arc(point.x, point.y, radius, 0, Math.PI * 2)
                context.fill()
                context.stroke()
                context.fillStyle = '#FFFFFF'
                context.font = `700 ${9 * poiIconScale.value}px system-ui`
                context.textAlign = 'center'
                context.textBaseline = 'middle'
                context.fillText(
                    featureLabel(feature.kind).slice(0, 1).toUpperCase(),
                    point.x,
                    point.y,
                )
            }
        }
        if (completed)
            drawCompletionBadge(
                context,
                point.x + 10 * poiIconScale.value,
                point.y - 10 * poiIconScale.value,
            )
        context.restore()
    }
}

function drawOres(context: CanvasRenderingContext2D) {
    if (scale.value > SEED_MAP_ORE_MAX_SCALE) return
    for (const hit of oreHits.value) {
        const point = worldToScreen(hit.x, hit.z)
        if (
            point.x < -20 ||
            point.x > viewport.width + 20 ||
            point.y < -20 ||
            point.y > viewport.height + 20
        )
            continue
        const key = seedMapOreKey(hit)
        const selected = selectedOreKey.value === key
        const completed = workspace.completedOres.includes(key)
        const definition = seedMapOreDefinition(hit.ore)
        const variant = hit.y < 0 && definition.deepslateTexture ? 'deepslate' : 'stone'
        const image = oreImages.get(`${hit.ore}:${variant}`)
        const size =
            (Math.max(10, Math.min(18, 1.2 / scale.value)) + (selected ? 4 : 0)) *
            poiIconScale.value
        context.save()
        context.globalAlpha = completed ? 0.42 : 0.78
        context.imageSmoothingEnabled = false
        if (image) {
            context.drawImage(image, point.x - size / 2, point.y - size / 2, size, size)
        } else {
            context.fillStyle = '#B6B7BA'
            context.fillRect(point.x - size / 2, point.y - size / 2, size, size)
        }
        context.strokeStyle = selected ? '#FFFFFF' : 'rgba(15, 18, 20, 0.82)'
        context.lineWidth = selected ? 2.5 : 1.5
        context.strokeRect(point.x - size / 2, point.y - size / 2, size, size)
        if (completed) drawCompletionBadge(context, point.x + size / 2, point.y - size / 2)
        context.restore()
    }
}

function drawSpawn(context: CanvasRenderingContext2D) {
    if (spawn.value && workspace.dimension === 'overworld' && workspace.showSpawn) {
        const point = worldToScreen(spawn.value.x, spawn.value.z)
        const selected =
            selection.value?.spawn === true &&
            selection.value.x === spawn.value.x &&
            selection.value.z === spawn.value.z
        const size = (selected ? 36 : 32) * poiIconScale.value
        context.save()
        if (spawnImage) {
            context.shadowColor = 'rgba(0, 0, 0, 0.48)'
            context.shadowBlur = 5
            context.shadowOffsetY = 2
            context.drawImage(spawnImage, point.x - size / 2, point.y - size / 2, size, size)
        } else {
            context.fillStyle = '#2E3138'
            context.strokeStyle = '#FFFFFF'
            context.lineWidth = 2
            context.beginPath()
            context.arc(point.x, point.y, 10 * poiIconScale.value, 0, Math.PI * 2)
            context.fill()
            context.stroke()
        }
        if (selected) {
            context.shadowColor = 'transparent'
            context.strokeStyle = '#FFFFFF'
            context.lineWidth = 2
            context.beginPath()
            context.arc(point.x, point.y, size / 2 + 2, 0, Math.PI * 2)
            context.stroke()
        }
        context.restore()
    }
}

function drawCompletionBadge(context: CanvasRenderingContext2D, x: number, y: number) {
    context.globalAlpha = 1
    context.shadowColor = 'transparent'
    context.fillStyle = '#1BD96A'
    context.strokeStyle = 'rgba(15, 18, 20, 0.9)'
    context.lineWidth = 2
    context.beginPath()
    context.arc(x, y, 7, 0, Math.PI * 2)
    context.fill()
    context.stroke()
    context.strokeStyle = '#FFFFFF'
    context.lineWidth = 1.75
    context.lineCap = 'round'
    context.lineJoin = 'round'
    context.beginPath()
    context.moveTo(x - 3, y)
    context.lineTo(x - 0.5, y + 2.5)
    context.lineTo(x + 3.5, y - 2.5)
    context.stroke()
}

function drawMarkers(context: CanvasRenderingContext2D) {
    drawSeedMapMarkers(context, markerPlacements(context))
}

function markerPlacements(context: CanvasRenderingContext2D) {
    return layoutSeedMapMarkers(
        context,
        dimensionMarkers.value,
        worldToScreen,
        viewport,
        poiIconScale.value,
        selection.value?.markerId,
    )
}

function drawRuler(context: CanvasRenderingContext2D) {
    if (!rulerPoints.value.length) return
    const points = rulerPoints.value.map((point) => worldToScreen(point.x, point.z))
    context.save()
    context.strokeStyle = '#F2B84B'
    context.fillStyle = '#F2B84B'
    context.lineWidth = 2
    context.setLineDash([6, 4])
    context.beginPath()
    context.arc(points[0].x, points[0].y, 4, 0, Math.PI * 2)
    context.fill()
    if (points[1]) {
        context.beginPath()
        context.moveTo(points[0].x, points[0].y)
        context.lineTo(points[1].x, points[1].y)
        context.stroke()
        context.setLineDash([])
        context.beginPath()
        context.arc(points[1].x, points[1].y, 4, 0, Math.PI * 2)
        context.fill()
    }
    context.restore()
}

async function retryMap() {
    try {
        profiles.value = await getSeedMapProfiles()
        ensureProfile()
        scheduleRefresh(0)
    } catch (error) {
        mapError.value = error instanceof Error ? error.message : String(error)
    }
}
async function startMapGuide() {
    if (document.fullscreenElement) await document.exitFullscreen()
    await replayOnboarding?.('seed-map')
}

function scheduleRefresh(delay = 80) {
    if (disposed || !viewport.width || !viewport.height) return
    requestEpoch++
    if (workspace.displayMode === 'underground') tileLayer.clear()
    else tileLayer.invalidate(tileView())
    refreshCurrentOreLayer(null)
    redraw()
    if (refreshTimer) window.clearTimeout(refreshTimer)
    refreshTimer = window.setTimeout(refreshMap, delay)
}

function refreshMap() {
    refreshTimer = undefined
    if (workspace.displayMode === 'underground') return
    if (!workspace.seed.trim()) {
        features.value = []
        spawn.value = null
        loading.value = false
        refreshCurrentOreLayer(0)
        redraw()
        return
    }
    if (!selectedProfile.value?.available) {
        mapError.value = selectedProfile.value?.note ?? formatMessage(messages.versionUnavailable)
        features.value = []
        spawn.value = null
        refreshCurrentOreLayer(0)
        redraw()
        return
    }
    mapError.value = ''
    tileLayer.refresh(tileView())
    if (workspace.displayMode !== 'structures') features.value = []
    void requestSpawn()
}

function refreshCurrentOreLayer(delay: number | null = 180) {
    refreshOreLayer(
        {
            enabled:
                workspace.displayMode === 'ores' &&
                !!selectedProfile.value?.available &&
                !!workspace.seed.trim(),
            seed: workspace.seed,
            edition: workspace.edition,
            version: workspace.gameVersion,
            dimension: workspace.dimension,
            scale: scale.value,
            selectedOres: [...activeSelectedOres.value],
            yMin: workspace.oreYMin,
            yMax: workspace.oreYMax,
            bounds: mapBounds(),
            center: { ...workspace.center },
        },
        delay,
    )
}

async function requestFeatures(epoch: number) {
    if (activeFeatureMask.value === 0) {
        features.value = []
        redraw()
        return
    }
    const bounds = mapBounds()
    try {
        const result = await findSeedMapFeatures({
            seed: workspace.seed,
            edition: workspace.edition,
            version: workspace.gameVersion,
            dimension: workspace.dimension,
            minX: bounds.minX,
            minZ: bounds.minZ,
            maxX: bounds.maxX,
            maxZ: bounds.maxZ,
            featureMask: activeFeatureMask.value,
        })
        if (epoch !== requestEpoch) return
        features.value = result
        redraw()
    } catch (error) {
        if (epoch === requestEpoch)
            mapError.value = error instanceof Error ? error.message : String(error)
    }
}

async function requestSpawn() {
    if (workspace.dimension !== 'overworld' || !workspace.showSpawn) {
        spawn.value = null
        return
    }
    const contextKey = `${workspace.seed}|${workspace.edition}|${workspace.gameVersion}`
    if (spawn.value && spawnContextKey === contextKey) {
        redraw()
        return
    }
    spawn.value = null
    const request =
        spawnRequest?.key === contextKey
            ? spawnRequest
            : {
                  key: contextKey,
                  promise: getSeedMapSpawn(
                      workspace.seed,
                      workspace.edition,
                      workspace.gameVersion,
                  ),
              }
    spawnRequest = request
    const isCurrent = () =>
        !disposed &&
        workspace.dimension === 'overworld' &&
        workspace.showSpawn &&
        contextKey === `${workspace.seed}|${workspace.edition}|${workspace.gameVersion}`
    try {
        const result = await request.promise
        if (!isCurrent()) return
        spawn.value = result
        spawnContextKey = contextKey
        if (centerOnNextSpawn) {
            centerOnNextSpawn = false
            workspace.center = { x: result.x, z: result.z }
            selection.value = { x: result.x, z: result.z, spawn: true }
            scheduleRefresh()
            return
        }
        redraw()
    } catch {
        if (isCurrent()) {
            spawn.value = null
            spawnContextKey = ''
            if (centerOnNextSpawn) {
                centerOnNextSpawn = false
                scheduleRefresh()
            }
        }
    } finally {
        if (spawnRequest === request) spawnRequest = undefined
    }
}

function onPointerDown(event: PointerEvent) {
    if (canvas.value && event.button === 2) {
        event.preventDefault()
        clearWheelGesture()
        stopZoomAnimation()
        flushPendingPan()
        const start = screenToWorld(event.offsetX, event.offsetY)
        areaDrag.value = {
            id: event.pointerId,
            start,
            end: start,
            screenX: event.offsetX,
            screenY: event.offsetY,
            moved: false,
        }
        suppressAreaContextMenu = false
        if (areaContextMenuTimer) clearTimeout(areaContextMenuTimer)
        canvas.value.setPointerCapture(event.pointerId)
        return
    }
    if (!canvas.value || event.button !== 0) return
    clearWheelGesture()
    stopZoomAnimation()
    canvas.value.setPointerCapture(event.pointerId)
    activePointers.set(event.pointerId, { x: event.offsetX, y: event.offsetY })
    if (activePointers.size === 2) {
        flushPendingPan()
        pinchDistance = currentPinchDistance()
        suppressPointerSelection = true
        clearLongPress()
        return
    }
    pointer.id = event.pointerId
    pointer.startX = event.offsetX
    pointer.startY = event.offsetY
    pointer.centerX = workspace.center.x
    pointer.centerZ = workspace.center.z
    pointer.moved = false
    suppressPointerSelection = false
    if (event.pointerType === 'touch') {
        clearLongPress()
        longPressTimer = window.setTimeout(() => {
            if (pointer.id !== event.pointerId || pointer.moved) return
            openMarkerDraftAt(event.offsetX, event.offsetY)
            suppressPointerSelection = true
        }, 650)
    }
}

function onPointerMove(event: PointerEvent) {
    if (areaDrag.value?.id === event.pointerId) {
        const drag = areaDrag.value
        drag.end = screenToWorld(event.offsetX, event.offsetY)
        drag.moved ||= Math.hypot(event.offsetX - drag.screenX, event.offsetY - drag.screenY) > 4
        suppressAreaContextMenu ||= drag.moved
        return
    }
    if (!activePointers.has(event.pointerId)) return
    activePointers.set(event.pointerId, { x: event.offsetX, y: event.offsetY })
    if (activePointers.size === 2) {
        const distance = currentPinchDistance()
        if (
            pinchDistance > 0 &&
            (distance > pinchDistance * 1.45 || distance < pinchDistance * 0.69)
        ) {
            const points = [...activePointers.values()]
            zoomAt(
                (points[0].x + points[1].x) / 2,
                (points[0].y + points[1].y) / 2,
                distance > pinchDistance ? -0.35 : 0.35,
            )
            pinchDistance = distance
        }
        return
    }
    if (pointer.id !== event.pointerId) return
    const deltaX = event.offsetX - pointer.startX
    const deltaY = event.offsetY - pointer.startY
    if (Math.abs(deltaX) > 3 || Math.abs(deltaY) > 3) {
        pointer.moved = true
        clearLongPress()
    }
    if (!pointer.moved) return
    pendingPanCenter = {
        x: clampWorldCoordinate(pointer.centerX - deltaX * scale.value),
        z: clampWorldCoordinate(pointer.centerZ - deltaY * scale.value),
    }
    if (panFrame === undefined) panFrame = window.requestAnimationFrame(flushPendingPan)
}

function onPointerUp(event: PointerEvent) {
    if (areaDrag.value?.id === event.pointerId) {
        const drag = areaDrag.value
        if (event.type !== 'pointercancel' && drag.moved) {
            selectedArea.value = seedMapArea(
                drag.start,
                screenToWorld(event.offsetX, event.offsetY),
            )
            selection.value = null
            markerDraftOpen.value = false
        }
        areaDrag.value = null
        if (suppressAreaContextMenu)
            areaContextMenuTimer = setTimeout(() => {
                suppressAreaContextMenu = false
            }, 1000)
        canvas.value?.releasePointerCapture(event.pointerId)
        return
    }
    if (!activePointers.has(event.pointerId)) return
    activePointers.delete(event.pointerId)
    clearLongPress()
    canvas.value?.releasePointerCapture(event.pointerId)
    if (pointer.id !== event.pointerId) return
    flushPendingPan()
    const clicked = !pointer.moved && !suppressPointerSelection
    pointer.id = -1
    if (clicked) selectAt(event.offsetX, event.offsetY)
    else scheduleRefresh(40)
    if (activePointers.size < 2) pinchDistance = 0
}

function flushPendingPan() {
    if (panFrame !== undefined) window.cancelAnimationFrame(panFrame)
    panFrame = undefined
    if (!pendingPanCenter) return
    workspace.center.x = pendingPanCenter.x
    workspace.center.z = pendingPanCenter.z
    pendingPanCenter = undefined
    redraw()
}

function onWheel(event: WheelEvent) {
    event.preventDefault()
    const now = performance.now()
    const deltaMultiplier =
        event.deltaMode === WheelEvent.DOM_DELTA_LINE
            ? 40
            : event.deltaMode === WheelEvent.DOM_DELTA_PAGE
              ? 300
              : 1
    const normalizedDelta = event.deltaY * deltaMultiplier
    if (normalizedDelta === 0) return
    if (!wheelMode || now - wheelGestureStartedAt > 400) {
        clearWheelGesture()
        wheelMode = Math.abs(normalizedDelta) < 4 ? 'trackpad' : 'wheel'
        wheelGestureStartedAt = now
        if (wheelMode === 'trackpad') stopZoomAnimation()
    }
    wheelAnchor = { x: event.offsetX, y: event.offsetY }
    if (wheelMode === 'trackpad') {
        wheelGestureStartedAt = now
        applyZoomAt(event.offsetX, event.offsetY, normalizedDelta / 600)
        if (trackpadTimer) window.clearTimeout(trackpadTimer)
        trackpadTimer = window.setTimeout(() => {
            trackpadTimer = undefined
            scheduleRefresh(60)
        }, 80)
        return
    }

    wheelDelta += normalizedDelta
    if (wheelTimer) window.clearTimeout(wheelTimer)
    const timeLeft = Math.max(80 - (now - wheelGestureStartedAt), 0)
    wheelTimer = window.setTimeout(applyWheelZoom, timeLeft)
}

function applyWheelZoom() {
    wheelTimer = undefined
    const deltaZoom = Math.min(Math.max(wheelDelta, -300), 300) / 600
    const anchor = wheelAnchor
    clearWheelGesture()
    if (deltaZoom === 0) return
    animateZoomTo(
        anchor.x,
        anchor.y,
        Math.min(
            Math.max(workspace.zoom + deltaZoom, SEED_MAP_MIN_ZOOM),
            SEED_MAP_SCALES.length - 1,
        ),
        250,
    )
}

function clearWheelGesture() {
    if (wheelTimer) window.clearTimeout(wheelTimer)
    if (trackpadTimer) window.clearTimeout(trackpadTimer)
    wheelTimer = undefined
    trackpadTimer = undefined
    wheelMode = undefined
    wheelGestureStartedAt = 0
    wheelDelta = 0
}

function onDoubleClick(event: MouseEvent) {
    event.preventDefault()
    zoomAt(event.offsetX, event.offsetY, -0.5)
}

function onContextMenu(event: MouseEvent) {
    event.preventDefault()
    if (suppressAreaContextMenu) {
        suppressAreaContextMenu = false
        return
    }
    openMarkerDraftAt(event.offsetX, event.offsetY)
}

function onMapKeydown(event: KeyboardEvent) {
    if (workspace.displayMode === 'underground') return
    const target = event.target
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return
    let mapMoved = false
    const panDistance = Math.max(
        64,
        Math.round(Math.min(viewport.width, viewport.height) * scale.value * 0.18),
    )
    if (event.key === 'ArrowLeft') {
        workspace.center.x = clampWorldCoordinate(workspace.center.x - panDistance)
        mapMoved = true
    } else if (event.key === 'ArrowRight') {
        workspace.center.x = clampWorldCoordinate(workspace.center.x + panDistance)
        mapMoved = true
    } else if (event.key === 'ArrowUp') {
        workspace.center.z = clampWorldCoordinate(workspace.center.z - panDistance)
        mapMoved = true
    } else if (event.key === 'ArrowDown') {
        workspace.center.z = clampWorldCoordinate(workspace.center.z + panDistance)
        mapMoved = true
    } else if (event.key === '+' || event.key === '=') zoomBy(-0.5)
    else if (event.key === '-') zoomBy(0.5)
    else if (event.key.toLowerCase() === 'f') void toggleFullscreen()
    else if (event.key === 'Escape') {
        selectedArea.value = null
        areaDrag.value = null
        selection.value = null
        advancedOpen.value = false
        layersExpanded.value = false
        if (isFullscreen.value) void document.exitFullscreen()
        else return
    } else return
    event.preventDefault()
    if (mapMoved) scheduleRefresh()
}

function currentPinchDistance() {
    const points = [...activePointers.values()]
    if (points.length !== 2) return 0
    return Math.hypot(points[1].x - points[0].x, points[1].y - points[0].y)
}

function clearLongPress() {
    if (!longPressTimer) return
    window.clearTimeout(longPressTimer)
    longPressTimer = undefined
}

function selectAt(x: number, y: number) {
    const point = screenToWorld(x, y)
    const context = canvas.value?.getContext('2d')
    const marker = context ? hitSeedMapMarker(markerPlacements(context), x, y) : undefined
    if (
        !marker &&
        scale.value <= SEED_MAP_LAYOUT_MAX_SCALE &&
        workspace.displayMode === 'structures'
    ) {
        for (const { feature, query } of visibleLayoutQueries.value) {
            const piece = hitSeedMapLayout(
                seedMapLayoutPieces(layoutLayer.peek(query) ?? null),
                point.x,
                point.z,
                focusedPiece.value?.id,
            )
            if (!piece) continue
            selection.value = { x: feature.x, z: feature.z, feature }
            nextTick(() => {
                focusedPiece.value = piece
                redraw()
            })
            markerDraftOpen.value = false
            return
        }
    }
    const feature =
        workspace.displayMode === 'structures'
            ? displayedFeatures.value
                  .map((item) => ({ item, point: worldToScreen(item.x, item.z) }))
                  .find(
                      (candidate) =>
                          Math.hypot(candidate.point.x - x, candidate.point.y - y) <=
                          13 * (candidate.item.kind === 'slime-chunk' ? 1 : poiIconScale.value),
                  )?.item
            : undefined
    const ore =
        workspace.displayMode === 'ores' && scale.value <= SEED_MAP_ORE_MAX_SCALE
            ? oreHits.value
                  .map((item) => ({ item, point: worldToScreen(item.x, item.z) }))
                  .filter(
                      (candidate) =>
                          Math.hypot(candidate.point.x - x, candidate.point.y - y) <=
                          12 * poiIconScale.value,
                  )
                  .sort(
                      (a, b) =>
                          Math.hypot(a.point.x - x, a.point.y - y) -
                          Math.hypot(b.point.x - x, b.point.y - y),
                  )[0]?.item
            : undefined
    const spawnSelected =
        workspace.showSpawn &&
        workspace.dimension === 'overworld' &&
        spawn.value !== null &&
        Math.hypot(
            worldToScreen(spawn.value.x, spawn.value.z).x - x,
            worldToScreen(spawn.value.x, spawn.value.z).y - y,
        ) <=
            14 * poiIconScale.value
    selection.value = marker
        ? { x: marker.x, z: marker.z, markerId: marker.id }
        : feature
          ? { x: feature.x, z: feature.z, feature }
          : ore
            ? { x: ore.x, z: ore.z, ore }
            : spawnSelected && spawn.value
              ? { x: spawn.value.x, z: spawn.value.z, spawn: true }
              : point
    if (!marker && !feature && !ore && !(spawnSelected && spawn.value)) {
        void lookupSelectionBiome(point)
    }
    markerDraftOpen.value = false
    if (rulerEnabled.value) {
        rulerPoints.value = rulerPoints.value.length === 1 ? [rulerPoints.value[0], point] : [point]
    }
    redraw()
}

function locateAreaLoot(match: AreaLootMatch) {
    workspace.center = { x: match.container.x, z: match.container.z }
    selection.value = { ...workspace.center }
    scheduleRefresh()
}

watch(
    () => [
        workspace.seed,
        workspace.edition,
        workspace.gameVersion,
        workspace.dimension,
        workspace.displayMode,
    ],
    () => {
        selectedArea.value = null
        areaDrag.value = null
    },
)

async function lookupSelectionBiome(point: { x: number; z: number }) {
    if (!workspace.seed.trim() || !selectedProfile.value?.available) return
    const token = ++biomeLookupToken
    try {
        const biome = await getSeedMapBiomeAt({
            seed: workspace.seed,
            edition: workspace.edition,
            version: workspace.gameVersion,
            dimension: workspace.dimension,
            // A tile pixel is sampled at the centre of its source-scale cell.
            // Querying the same cell centre keeps the popup biome in
            // sync with the color rendered underneath it when zoomed out.
            x:
                Math.floor(point.x / tileScale.value) * tileScale.value +
                Math.floor(tileScale.value / 2),
            y: workspace.elevation,
            z:
                Math.floor(point.z / tileScale.value) * tileScale.value +
                Math.floor(tileScale.value / 2),
        })
        if (token !== biomeLookupToken || biome < 0) return
        const current = selection.value
        if (
            !current ||
            current.x !== point.x ||
            current.z !== point.z ||
            current.feature ||
            current.ore ||
            current.markerId ||
            current.spawn
        ) {
            return
        }
        selection.value = { ...current, biome }
    } catch {
        // The plain location popup stays when the engine cannot resolve a biome.
    }
}

function biomeDisplayName(biome: number) {
    const name = SEED_MAP_BIOME_NAMES[biome]
    if (!name) return `#${biome}`
    const descriptor = seedMapBiomeMessage(name)
    return descriptor ? formatMessage(descriptor) : name
}

function biomeColorOf(biome: number) {
    return SEED_MAP_BIOMES.find((item) => item.id === biome)?.color ?? 'transparent'
}

function openMarkerDraftAt(x: number, y: number) {
    const point = screenToWorld(x, y)
    selection.value = point
    markerName.value = ''
    markerDraftOpen.value = true
    redraw()
}

function zoomAt(x: number, y: number, delta: number) {
    clearWheelGesture()
    const nextZoom = Math.min(
        Math.max(workspace.zoom + delta, SEED_MAP_MIN_ZOOM),
        SEED_MAP_SCALES.length - 1,
    )
    animateZoomTo(x, y, nextZoom, 250)
}

function zoomBy(delta: number) {
    zoomAt(viewport.width / 2, viewport.height / 2, delta)
}

function applyZoomAt(x: number, y: number, delta: number) {
    const targetZoom = Math.min(
        Math.max(workspace.zoom + delta, SEED_MAP_MIN_ZOOM),
        SEED_MAP_SCALES.length - 1,
    )
    if (Math.abs(targetZoom - workspace.zoom) < 0.0001) return
    const anchor = {
        x: workspace.center.x + (x - viewport.width / 2) * scale.value,
        z: workspace.center.z + (y - viewport.height / 2) * scale.value,
    }
    workspace.zoom = targetZoom
    workspace.center.x = clampWorldCoordinate(anchor.x - (x - viewport.width / 2) * scale.value)
    workspace.center.z = clampWorldCoordinate(anchor.z - (y - viewport.height / 2) * scale.value)
    redraw()
}

function animateZoomTo(x: number, y: number, targetZoom: number, duration: number) {
    if (Math.abs(targetZoom - workspace.zoom) < 0.0001) return
    if (zoomAnimationFrame !== undefined) window.cancelAnimationFrame(zoomAnimationFrame)
    const startTime = performance.now()
    const startZoom = workspace.zoom
    const anchor = {
        x: workspace.center.x + (x - viewport.width / 2) * scale.value,
        z: workspace.center.z + (y - viewport.height / 2) * scale.value,
    }
    const step = (now: number) => {
        const progress = Math.min(1, (now - startTime) / duration)
        const easedProgress = 1 - (1 - progress) ** 3
        workspace.zoom = startZoom + (targetZoom - startZoom) * easedProgress
        workspace.center.x = clampWorldCoordinate(anchor.x - (x - viewport.width / 2) * scale.value)
        workspace.center.z = clampWorldCoordinate(
            anchor.z - (y - viewport.height / 2) * scale.value,
        )
        redraw()
        if (progress < 1) {
            zoomAnimationFrame = window.requestAnimationFrame(step)
            return
        }
        // Store the exact endpoint so repeated wheel gestures at the zoom limits
        // do not leave a tiny floating-point remainder that restarts the animation.
        workspace.zoom = targetZoom
        zoomAnimationFrame = undefined
        scheduleRefresh(60)
    }
    zoomAnimationFrame = window.requestAnimationFrame(step)
}

function stopZoomAnimation() {
    if (zoomAnimationFrame !== undefined) window.cancelAnimationFrame(zoomAnimationFrame)
    zoomAnimationFrame = undefined
}

function toggleFeature(feature: SeedMapFeatureKind) {
    workspace.visibleFeatures = workspace.visibleFeatures.includes(feature)
        ? workspace.visibleFeatures.filter((kind) => kind !== feature)
        : [...workspace.visibleFeatures, feature]
}

function toggleOre(ore: SeedMapOreKind) {
    workspace.selectedOres = workspace.selectedOres.includes(ore)
        ? workspace.selectedOres.filter((kind) => kind !== ore)
        : [...workspace.selectedOres, ore]
}

function selectAllOres() {
    workspace.selectedOres = [
        ...new Set([...workspace.selectedOres, ...dimensionOres.value.map((ore) => ore.kind)]),
    ]
}

function clearOres() {
    const current = new Set(dimensionOres.value.map((ore) => ore.kind))
    workspace.selectedOres = workspace.selectedOres.filter((ore) => !current.has(ore))
}

function resetOres() {
    const current = new Set(dimensionOres.value.map((ore) => ore.kind))
    const defaultOre = workspace.dimension === 'nether' ? 'netherite' : 'diamond'
    workspace.selectedOres = [
        ...workspace.selectedOres.filter((ore) => !current.has(ore)),
        ...(current.has(defaultOre) ? [defaultOre] : []),
    ]
    resetOreYRange()
}

function resetOreYRange() {
    workspace.oreYMin = null
    workspace.oreYMax = null
}

function selectAllFeatures() {
    const selected = new Set(workspace.visibleFeatures)
    for (const feature of dimensionFeatures.value) selected.add(feature.kind)
    workspace.visibleFeatures = [...selected]
}

function clearFeatures() {
    const current = new Set(dimensionFeatures.value.map((feature) => feature.kind))
    workspace.visibleFeatures = workspace.visibleFeatures.filter((feature) => !current.has(feature))
    workspace.showSpawn = false
}

function resetFeatures() {
    const defaults = createDefaultSeedMapWorkspace().visibleFeatures
    const current = new Set(dimensionFeatures.value.map((feature) => feature.kind))
    workspace.visibleFeatures = [
        ...workspace.visibleFeatures.filter((feature) => !current.has(feature)),
        ...defaults.filter((feature) => current.has(feature)),
    ]
    workspace.showSpawn = true
}

function randomizeSeed() {
    const bytes = new Uint32Array(2)
    crypto.getRandomValues(bytes)
    const value = (BigInt(bytes[0]) << 32n) | BigInt(bytes[1])
    pendingHistorySource = 'random'
    workspace.seed = BigInt.asIntN(64, value).toString()
}

function onWorldImported(selection: SeedMapWorldImport) {
    workspace.edition = 'java'
    if (selection.version) workspace.gameVersion = selection.version
    ensureProfile()
    pendingHistorySource = 'instance'
    pendingHistoryLabels = {
        instanceName: selection.instance.name,
        worldName: selection.world.name,
    }
    if (workspace.seed === selection.seed) {
        recordHistory('instance', pendingHistoryLabels)
        pendingHistorySource = null
        pendingHistoryLabels = {}
    } else {
        workspace.seed = selection.seed
    }
    addNotification({
        type: 'success',
        title: formatMessage(messages.worldSeedLoaded, { world: selection.world.name }),
    })
}

function recordHistory(
    source: SeedMapHistorySource,
    labels: { instanceName?: string; worldName?: string } = {},
) {
    if (!workspace.seed.trim()) return
    historyEntries.value = recordSeedMapHistory({
        seed: workspace.seed,
        edition: workspace.edition,
        gameVersion: workspace.gameVersion,
        source,
        completedFeatures: [...workspace.completedFeatures],
        completedOres: [...workspace.completedOres],
        ...labels,
    })
}

function stashActiveProgress() {
    historyEntries.value = updateSeedMapHistoryProgress(
        activeHistoryId,
        [...workspace.completedFeatures],
        [...workspace.completedOres],
    )
}

function applyHistoryEntry(entry: SeedMapHistoryEntry) {
    pendingHistorySource = entry.source
    pendingHistoryLabels = { instanceName: entry.instanceName, worldName: entry.worldName }
    workspace.edition = entry.edition
    workspace.gameVersion = entry.gameVersion
    ensureProfile()
    if (workspace.seed === entry.seed) {
        if (activeHistoryId !== entry.id) {
            stashActiveProgress()
            activeHistoryId = entry.id
        }
        workspace.completedFeatures = [...entry.completedFeatures]
        workspace.completedOres = [...entry.completedOres]
        recordHistory(entry.source, pendingHistoryLabels)
        pendingHistorySource = null
        pendingHistoryLabels = {}
    } else {
        workspace.seed = entry.seed
    }
}

function removeHistory(id: string) {
    historyEntries.value = removeSeedMapHistoryEntry(id)
}

function clearHistory() {
    historyEntries.value = clearSeedMapHistory()
}

function historySourceLabel(entry: SeedMapHistoryEntry) {
    if (entry.source === 'instance') {
        const parts = [entry.instanceName, entry.worldName].filter(Boolean)
        if (parts.length > 0) return parts.join(' · ')
    }
    if (entry.source === 'random') return formatMessage(messages.historySourceRandom)
    if (entry.source === 'share') return formatMessage(messages.historySourceShare)
    return formatMessage(messages.historySourceManual)
}

function goToCoordinates() {
    const x = Number(coordinateX.value)
    const z = Number(coordinateZ.value)
    if (!Number.isFinite(x) || !Number.isFinite(z)) return
    workspace.center.x = clampWorldCoordinate(x)
    workspace.center.z = clampWorldCoordinate(z)
    selection.value = { x: Math.round(workspace.center.x), z: Math.round(workspace.center.z) }
    scheduleRefresh()
}

function centerSpawn() {
    if (!spawn.value) return
    workspace.center.x = spawn.value.x
    workspace.center.z = spawn.value.z
    selection.value = { x: spawn.value.x, z: spawn.value.z, spawn: true }
    scheduleRefresh()
}

function addMarker() {
    if (!selection.value) return
    const marker: SeedMapMarker = {
        id: crypto.randomUUID(),
        name: markerName.value.trim() || `${selection.value.x}, ${selection.value.z}`,
        x: selection.value.x,
        z: selection.value.z,
        color: markerColor.value,
        dimension: workspace.dimension,
    }
    workspace.markers.push(marker)
    selection.value = { x: marker.x, z: marker.z, markerId: marker.id }
    markerName.value = ''
    markerDraftOpen.value = false
    addNotification({ type: 'success', title: formatMessage(messages.markerSaved) })
}

function removeMarker(id: string) {
    workspace.markers = workspace.markers.filter((marker) => marker.id !== id)
    if (selection.value?.markerId === id) selection.value = null
}

function jumpToMarker(marker: SeedMapMarker) {
    if (marker.dimension !== workspace.dimension) return
    workspace.center = { x: marker.x, z: marker.z }
    selection.value = { x: marker.x, z: marker.z, markerId: marker.id }
    advancedOpen.value = false
    scheduleRefresh()
}

async function copyTeleport() {
    if (!selection.value) return
    try {
        const y = selection.value.ore?.y ?? '~'
        await navigator.clipboard.writeText(`/tp @s ${selection.value.x} ${y} ${selection.value.z}`)
        addNotification({ type: 'success', title: formatMessage(messages.teleportCopied) })
    } catch (error) {
        handleError(error)
    }
}

async function copyShareLink() {
    try {
        const query = new URLSearchParams(createShareQuery(workspace))
        await navigator.clipboard.writeText(`axolotl://lab/seed-map?${query.toString()}`)
        addNotification({ type: 'success', title: formatMessage(messages.shareCopied) })
    } catch (error) {
        handleError(error)
    }
}

function focusNearestFeature() {
    if (!displayedFeatures.value.length) return
    const nearest = [...displayedFeatures.value].sort(
        (a, b) =>
            Math.hypot(a.x - workspace.center.x, a.z - workspace.center.z) -
            Math.hypot(b.x - workspace.center.x, b.z - workspace.center.z),
    )[0]
    selection.value = { x: nearest.x, z: nearest.z, feature: nearest }
    workspace.center.x = nearest.x
    workspace.center.z = nearest.z
    scheduleRefresh()
}

async function toggleFullscreen() {
    try {
        if (document.fullscreenElement) await document.exitFullscreen()
        else await fullscreenContainer.value?.requestFullscreen()
    } catch (error) {
        handleError(error)
    }
}

function onFullscreenChange() {
    isFullscreen.value = document.fullscreenElement === fullscreenContainer.value
    nextTick(resizeCanvas)
}

function clampWorldCoordinate(value: number) {
    return Math.min(Math.max(value, -30_000_000), 30_000_000)
}
</script>

<template>
    <main
        class="seed-map-page mx-auto flex h-full min-h-0 w-full max-w-[110rem] flex-col gap-3 p-4"
    >
        <header class="flex min-h-9 items-center justify-between gap-3">
            <div class="min-w-0">
                <h1 class="m-0 truncate text-xl font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.title) }}
                </h1>
                <p class="m-0 truncate text-xs text-[var(--color-text-tertiary)]">
                    {{ editionLabel(workspace.edition) }} {{ workspace.gameVersion }} ·
                    {{ dimensionLabel(workspace.dimension) }}
                </p>
            </div>
            <div class="seed-map-header-actions flex shrink-0 items-center gap-2">
                <span class="map-scale-badge">{{
                    formatMessage(messages.mapScale, { scale: mapScaleLabel })
                }}</span>
                <Button
                    v-tooltip="formatMessage(messages.copyright)"
                    type="outlined"
                    circular
                    icon-only
                    :aria-label="formatMessage(messages.copyright)"
                    @click="copyrightModal?.show($event)"
                    ><CircleAlertIcon />
                </Button>
            </div>
        </header>

        <section data-onboarding-id="seed-map-toolbar" class="seed-map-toolbar">
            <div class="toolbar-primary">
                <div class="control-group seed-map-edition">
                    <span class="control-label">{{ formatMessage(messages.edition) }}</span>
                    <div class="seed-map-dropdown min-w-0">
                        <Combobox
                            v-model="workspace.edition"
                            :options="
                                editionOptions.map((value) => ({
                                    value,
                                    label: editionLabel(value),
                                }))
                            "
                        />
                    </div>
                </div>
                <div class="control-group seed-map-version">
                    <span class="control-label">{{ formatMessage(messages.gameVersion) }}</span>
                    <div class="seed-map-dropdown min-w-0">
                        <Combobox
                            v-model="workspace.gameVersion"
                            :options="availableVersions.map((value) => ({ value, label: value }))"
                        />
                    </div>
                </div>
                <div class="control-group seed-control">
                    <span class="control-label">{{ formatMessage(messages.seed) }}</span>
                    <div class="seed-input-row">
                        <StyledInput
                            v-model="workspace.seed"
                            :icon="HashIcon"
                            :placeholder="formatMessage(messages.seedPlaceholder)"
                            wrapper-class="seed-map-seed w-full"
                        />

                        <Button
                            v-tooltip="formatMessage(messages.randomSeed)"
                            type="outlined"
                            circular
                            icon-only
                            :aria-label="formatMessage(messages.randomSeed)"
                            @click="randomizeSeed"
                            ><RefreshCwIcon />
                        </Button>
                        <PopoutMenu dropdown-class="seed-map-history-popout" placement="bottom-end">
                            <template #trigger>
                                <Button
                                    v-tooltip="formatMessage(messages.history)"
                                    type="outlined"
                                    circular
                                    icon-only
                                    :aria-label="formatMessage(messages.history)"
                                    ><HistoryIcon />
                                </Button>
                            </template>
                            <template #menu>
                                <div class="seed-history-menu">
                                    <div class="seed-history-heading">
                                        <strong>{{ formatMessage(messages.history) }}</strong>
                                        <Button
                                            size="2xs"
                                            type="quiet"
                                            :disabled="historyEntries.length === 0"
                                            @click="clearHistory"
                                        >
                                            {{ formatMessage(messages.clearHistory) }}
                                        </Button>
                                    </div>
                                    <p
                                        v-if="historyEntries.length === 0"
                                        class="seed-history-empty"
                                    >
                                        {{ formatMessage(messages.historyEmpty) }}
                                    </p>
                                    <div v-else class="seed-history-list">
                                        <div
                                            v-for="entry in historyEntries"
                                            :key="entry.id"
                                            class="seed-history-row"
                                        >
                                            <button
                                                class="seed-history-load"
                                                @click="applyHistoryEntry(entry)"
                                            >
                                                <span class="seed-history-seed">{{
                                                    entry.seed
                                                }}</span>
                                                <span class="seed-history-meta">
                                                    <template v-if="entry.gameVersion"
                                                        >{{ entry.gameVersion }} ·
                                                    </template>
                                                    <template
                                                        v-if="entry.edition === 'java-large-biomes'"
                                                        >{{
                                                            formatMessage(messages.javaLargeBiomes)
                                                        }}
                                                        · </template
                                                    >{{ historySourceLabel(entry) }} ·
                                                    {{
                                                        formatRelativeTime(
                                                            new Date(
                                                                entry.lastViewedAt,
                                                            ).toISOString(),
                                                        )
                                                    }}<span
                                                        v-if="
                                                            entry.completedFeatures.length +
                                                                entry.completedOres.length >
                                                            0
                                                        "
                                                        class="seed-history-progress"
                                                        >✓
                                                        {{
                                                            entry.completedFeatures.length +
                                                            entry.completedOres.length
                                                        }}</span
                                                    >
                                                </span>
                                            </button>
                                            <Button
                                                v-tooltip="
                                                    formatMessage(messages.removeHistoryEntry)
                                                "
                                                circular
                                                icon-only
                                                size="2xs"
                                                type="quiet"
                                                class="seed-history-remove"
                                                :label="formatMessage(messages.removeHistoryEntry)"
                                                @click="removeHistory(entry.id)"
                                            >
                                                <TrashIcon />
                                            </Button>
                                        </div>
                                    </div>
                                </div>
                            </template>
                        </PopoutMenu>
                        <Button
                            v-tooltip="formatMessage(messages.importFromInstance)"
                            type="outlined"
                            :aria-label="formatMessage(messages.importFromInstance)"
                            @click="worldImportModal?.show()"
                            ><ImportIcon />{{ formatMessage(messages.importFromInstance) }}
                        </Button>
                    </div>
                </div>
            </div>

            <div class="toolbar-secondary">
                <div class="control-group dimension-group shrink-0">
                    <span class="control-label">{{ formatMessage(messages.dimension) }}</span>
                    <div
                        class="dimension-control flex h-10 items-stretch overflow-hidden rounded-[var(--radius-md)] border border-surface-5 bg-surface-3"
                        role="group"
                        :aria-label="formatMessage(messages.dimension)"
                    >
                        <Button
                            v-for="dimension in dimensions"
                            :key="dimension"
                            size="2xs"
                            :type="workspace.dimension === dimension ? 'colored' : 'quiet'"
                            :color="workspace.dimension === dimension ? 'brand' : undefined"
                            class="dimension-option"
                            :aria-pressed="workspace.dimension === dimension"
                            @click="workspace.dimension = dimension"
                        >
                            {{ dimensionLabel(dimension) }}
                        </Button>
                    </div>
                </div>
                <div class="control-group coordinate-group min-w-80">
                    <span class="control-label">{{ formatMessage(messages.coordinates) }}</span>
                    <div
                        class="coordinate-control"
                        :aria-label="formatMessage(messages.coordinates)"
                    >
                        <label class="coordinate-field relative flex min-w-0 items-center"
                            ><span>X</span
                            ><StyledInput
                                v-model="coordinateX"
                                type="number"
                                @keydown.enter="goToCoordinates"
                        /></label>
                        <label class="coordinate-field relative flex min-w-0 items-center"
                            ><span>Z</span
                            ><StyledInput
                                v-model="coordinateZ"
                                type="number"
                                @keydown.enter="goToCoordinates"
                        /></label>
                        <Button
                            type="colored"
                            color="brand"
                            class="coordinate-go h-10"
                            @click="goToCoordinates"
                            ><CompassIcon />{{ formatMessage(messages.go) }}
                        </Button>
                    </div>
                </div>
                <div
                    v-if="
                        workspace.dimension === 'overworld' &&
                        workspace.displayMode !== 'underground'
                    "
                    class="control-group elevation-group shrink-0"
                >
                    <span class="control-label">{{ formatMessage(messages.depth) }}</span>
                    <div class="seed-map-dropdown min-w-0">
                        <Combobox
                            :model-value="workspace.elevation as SeedMapElevation"
                            :options="
                                elevationOptions.map((value) => ({
                                    value,
                                    label: elevationLabel(value),
                                }))
                            "
                            @update:model-value="setElevation"
                        />
                    </div>
                </div>
                <div class="share-button">
                    <Button type="outlined" @click="copyShareLink"
                        ><ShareIcon />{{ formatMessage(messages.share) }}</Button
                    >
                </div>
            </div>
        </section>

        <SeedMapPreviewNotice
            v-if="mapPreviewLimit"
            class="seed-map-error p-3"
            :reason="mapPreviewLimit"
        />
        <Admonition
            v-else-if="mapError"
            class="seed-map-error p-3"
            type="critical"
            :header="formatMessage(messages.mapFailed)"
            :body="mapError"
            ><template #actions
                ><Button type="outlined" size="2xs" @click="retryMap"
                    ><RefreshCwIcon />{{ formatMessage(messages.retryMap) }}</Button
                ></template
            ></Admonition
        >

        <section
            ref="fullscreenContainer"
            class="seed-map-experience min-h-0 flex-1 bg-surface-1"
            data-onboarding-id="seed-map-workspace"
            tabindex="0"
            @keydown="onMapKeydown"
        >
            <div class="layer-strip border border-surface-5 bg-surface-2">
                <div class="layer-strip-heading">
                    <span>{{
                        formatMessage(
                            workspace.displayMode === 'structures'
                                ? messages.layers
                                : workspace.displayMode === 'ores'
                                  ? messages.oreLayers
                                  : messages.undergroundMode,
                        )
                    }}</span>
                    <div
                        class="layer-mode-switch"
                        role="group"
                        :aria-label="formatMessage(messages.layers)"
                    >
                        <Button
                            size="2xs"
                            :type="workspace.displayMode === 'structures' ? 'colored' : 'quiet'"
                            :color="workspace.displayMode === 'structures' ? 'brand' : undefined"
                            :aria-pressed="workspace.displayMode === 'structures'"
                            @click="setDisplayMode('structures')"
                        >
                            <LandmarkIcon />{{ formatMessage(messages.structures) }}
                        </Button>
                        <Button
                            v-tooltip="
                                oreModeSupported
                                    ? undefined
                                    : formatMessage(messages.oresRequireModern)
                            "
                            size="2xs"
                            :type="workspace.displayMode === 'ores' ? 'colored' : 'quiet'"
                            :color="workspace.displayMode === 'ores' ? 'brand' : undefined"
                            :aria-pressed="workspace.displayMode === 'ores'"
                            :disabled="!oreModeSupported"
                            @click="setDisplayMode('ores')"
                        >
                            <PickaxeIcon />{{ formatMessage(messages.ores) }}
                        </Button>
                        <Button
                            :type="workspace.displayMode === 'underground' ? 'colored' : 'quiet'"
                            :disabled="!undergroundSupported"
                            size="2xs"
                            data-onboarding-id="seed-map-underground-mode"
                            @click="setDisplayMode('underground')"
                            ><LayersIcon />{{ formatMessage(messages.undergroundMode) }}</Button
                        >
                        <Button
                            type="quiet"
                            icon-only
                            :aria-label="formatMessage(messages.advancedTour)"
                            data-onboarding-id="seed-map-advanced-guide"
                            @click="startMapGuide"
                            ><InfoIcon
                        /></Button>
                    </div>
                    <div class="flex items-center gap-1">
                        <Button
                            v-tooltip="
                                formatMessage(
                                    showLayerNames ? messages.hideNames : messages.showNames,
                                )
                            "
                            circular
                            icon-only
                            :aria-label="
                                formatMessage(
                                    showLayerNames ? messages.hideNames : messages.showNames,
                                )
                            "
                            @click="showLayerNames = !showLayerNames"
                            ><EyeOffIcon v-if="showLayerNames" /><EyeIcon v-else
                        /></Button>
                        <Button
                            v-tooltip="
                                formatMessage(
                                    layersExpanded
                                        ? messages.collapseLayers
                                        : messages.expandLayers,
                                )
                            "
                            circular
                            icon-only
                            :aria-label="
                                formatMessage(
                                    layersExpanded
                                        ? messages.collapseLayers
                                        : messages.expandLayers,
                                )
                            "
                            @click="layersExpanded = !layersExpanded"
                            ><ChevronDownIcon :class="{ 'rotate-180': layersExpanded }"
                        /></Button>
                    </div>
                </div>
                <div v-if="workspace.displayMode !== 'underground'" class="layer-options">
                    <template v-if="workspace.displayMode === 'structures'">
                        <Button
                            v-for="feature in dimensionFeatures"
                            :key="feature.kind"
                            v-tooltip="featureTooltip(feature.kind, feature.maxScale)"
                            :type="
                                workspace.visibleFeatures.includes(feature.kind)
                                    ? 'colored'
                                    : 'base'
                            "
                            :color="
                                workspace.visibleFeatures.includes(feature.kind)
                                    ? 'brand'
                                    : undefined
                            "
                            class="layer-option w-10 p-1"
                            :class="{
                                'opacity-[0.68]': tileScale > feature.maxScale,
                                named: showLayerNames,
                            }"
                            :aria-pressed="workspace.visibleFeatures.includes(feature.kind)"
                            @click="toggleFeature(feature.kind)"
                        >
                            <img
                                v-if="featureImageSource(feature.kind)"
                                :src="featureImageSource(feature.kind)"
                                alt=""
                            /><component :is="featureIcons[feature.kind]" v-else /><span
                                v-if="showLayerNames"
                                >{{ featureLabel(feature.kind) }}</span
                            >
                        </Button>
                    </template>
                    <template v-else>
                        <Button
                            v-for="ore in dimensionOres"
                            :key="ore.kind"
                            v-tooltip="oreLabel(ore.kind)"
                            :type="workspace.selectedOres.includes(ore.kind) ? 'colored' : 'base'"
                            :color="workspace.selectedOres.includes(ore.kind) ? 'brand' : undefined"
                            class="layer-option w-10 p-1"
                            :class="{ named: showLayerNames }"
                            :aria-pressed="workspace.selectedOres.includes(ore.kind)"
                            @click="toggleOre(ore.kind)"
                        >
                            <img :src="oreImageSource(ore.image)" alt="" /><span
                                v-if="showLayerNames"
                                >{{ oreLabel(ore.kind) }}</span
                            >
                        </Button>
                    </template>
                    <span
                        v-if="workspace.displayMode === 'ores' && dimensionOres.length === 0"
                        class="layer-empty"
                    >
                        {{ formatMessage(messages.noOresInEnd) }}
                    </span>
                    <Button
                        v-if="workspace.dimension === 'overworld'"
                        v-tooltip="formatMessage(messages.spawnPoint)"
                        :type="workspace.showSpawn ? 'colored' : 'base'"
                        :color="workspace.showSpawn ? 'brand' : undefined"
                        class="layer-option w-10 p-1"
                        :class="{ named: showLayerNames }"
                        :aria-pressed="workspace.showSpawn"
                        @click="workspace.showSpawn = !workspace.showSpawn"
                    >
                        <img :src="`${structureAssetRoot}/spawn_point.webp`" alt="" /><span
                            v-if="showLayerNames"
                            >{{ formatMessage(messages.spawnPoint) }}</span
                        >
                    </Button>
                </div>
                <div
                    v-if="layersExpanded && workspace.displayMode !== 'underground'"
                    class="layer-expanded-panel"
                >
                    <div class="layer-bulk-actions">
                        <Button
                            type="outlined"
                            size="2xs"
                            @click="
                                workspace.displayMode === 'structures'
                                    ? selectAllFeatures()
                                    : selectAllOres()
                            "
                            >{{ formatMessage(messages.selectAll) }}
                        </Button>
                        <Button
                            type="outlined"
                            size="2xs"
                            @click="
                                workspace.displayMode === 'structures'
                                    ? clearFeatures()
                                    : clearOres()
                            "
                            >{{ formatMessage(messages.clear) }}
                        </Button>
                        <Button
                            type="outlined"
                            size="2xs"
                            @click="
                                workspace.displayMode === 'structures'
                                    ? resetFeatures()
                                    : resetOres()
                            "
                            >{{ formatMessage(messages.reset) }}
                        </Button>
                    </div>
                    <div class="layer-checklist">
                        <template v-if="workspace.displayMode === 'structures'">
                            <Checkbox
                                v-for="feature in dimensionFeatures"
                                :key="feature.kind"
                                :model-value="workspace.visibleFeatures.includes(feature.kind)"
                                :disabled="tileScale > feature.maxScale"
                                :description="featureTooltip(feature.kind, feature.maxScale)"
                                @update:model-value="toggleFeature(feature.kind)"
                            >
                                <span class="layer-checklist-label flex min-w-0 items-center gap-2">
                                    <img
                                        v-if="featureImageSource(feature.kind)"
                                        :src="featureImageSource(feature.kind)"
                                        alt=""
                                    /><component :is="featureIcons[feature.kind]" v-else />
                                    <span>{{ featureLabel(feature.kind) }}</span>
                                </span>
                            </Checkbox>
                        </template>
                        <template v-else>
                            <Checkbox
                                v-for="ore in dimensionOres"
                                :key="ore.kind"
                                :model-value="workspace.selectedOres.includes(ore.kind)"
                                :description="oreLabel(ore.kind)"
                                @update:model-value="toggleOre(ore.kind)"
                            >
                                <span class="layer-checklist-label flex min-w-0 items-center gap-2">
                                    <img :src="oreImageSource(ore.image)" alt="" />
                                    <span>{{ oreLabel(ore.kind) }}</span>
                                    <small>Y {{ ore.yMin }}~{{ ore.yMax }}</small>
                                </span>
                            </Checkbox>
                        </template>
                        <Checkbox
                            v-if="workspace.dimension === 'overworld'"
                            :model-value="workspace.showSpawn"
                            :description="formatMessage(messages.spawnPoint)"
                            @update:model-value="workspace.showSpawn = !workspace.showSpawn"
                        >
                            <span class="layer-checklist-label flex min-w-0 items-center gap-2">
                                <img :src="`${structureAssetRoot}/spawn_point.webp`" alt="" />
                                <span>{{ formatMessage(messages.spawnPoint) }}</span>
                            </span>
                        </Checkbox>
                    </div>
                    <div
                        v-if="workspace.displayMode === 'ores' && dimensionOres.length > 0"
                        class="ore-range-controls"
                    >
                        <div class="ore-range-heading">
                            <strong>{{ formatMessage(messages.oreYRange) }}</strong>
                            <span>Y {{ oreYMinimum }} - {{ oreYMaximum }}</span>
                            <Button type="quiet" size="2xs" @click="resetOreYRange">{{
                                formatMessage(messages.resetYRange)
                            }}</Button>
                        </div>
                        <label class="ore-range-control">
                            <span>{{ formatMessage(messages.oreMinY) }}</span>
                            <Slider
                                v-model="oreYMinimum"
                                :min="activeOreYRange[0]"
                                :max="oreYMaximum"
                                :step="1"
                            />
                        </label>
                        <label class="ore-range-control">
                            <span>{{ formatMessage(messages.oreMaxY) }}</span>
                            <Slider
                                v-model="oreYMaximum"
                                :min="oreYMinimum"
                                :max="activeOreYRange[1]"
                                :step="1"
                            />
                        </label>
                    </div>
                </div>
            </div>

            <SeedMapSplitView
                :open="!!structureQuery || !!selectedArea"
                class="seed-map-workspace-panes min-h-[32rem] rounded-b-[var(--radius-md)] border-x border-b border-surface-5"
            >
                <template #default="{ canvasSize, visibleSize }">
                    <div
                        class="seed-map-canvas-shell relative min-h-0 min-w-0 flex-1 overflow-hidden bg-surface-2"
                        data-onboarding-id="seed-map-canvas"
                        @contextmenu.prevent
                    >
                        <canvas
                            v-show="workspace.displayMode !== 'underground'"
                            ref="mapCanvas"
                            class="absolute left-0 top-0 block size-full touch-none"
                            :style="
                                canvasSize
                                    ? {
                                          width: `${canvasSize.width}px`,
                                          height: `${canvasSize.height}px`,
                                      }
                                    : undefined
                            "
                            :class="{ 'cursor-crosshair': rulerEnabled }"
                            @pointerdown="onPointerDown"
                            @mousedown.right.prevent
                            @pointermove="onPointerMove"
                            @pointerup="onPointerUp"
                            @pointercancel="onPointerUp"
                            @wheel="onWheel"
                            @dblclick="onDoubleClick"
                            @contextmenu.prevent.stop="onContextMenu"
                        />

                        <template v-if="workspace.displayMode !== 'underground'">
                            <div
                                v-if="areaStyle"
                                class="pointer-events-none absolute z-[4] border-2 border-solid border-purple-800 bg-highlight-purple"
                                :style="areaStyle"
                                data-testid="seed-map-area-selection"
                            />
                            <span
                                v-if="selectionVisible && selectionPointStyle"
                                class="pointer-events-none absolute z-[5] size-6 -translate-x-1/2 -translate-y-1/2"
                                :style="selectionPointStyle"
                                data-testid="seed-map-selection-point"
                                aria-hidden="true"
                            >
                                <PlusIcon
                                    class="absolute inset-0 size-full text-black [stroke-width:4]"
                                />
                                <PlusIcon class="absolute inset-0 size-full text-white" />
                            </span>
                            <button
                                v-for="badge in structureBadges(visibleSize)"
                                :key="badge.id"
                                type="button"
                                class="absolute z-[6] flex h-8 items-center gap-2 rounded-lg border border-surface-4 bg-surface-3 px-2 text-xs font-semibold text-[var(--color-text-primary)] shadow-md hover:bg-surface-4"
                                :class="{ 'ring-2 ring-brand': selectedFeatureKey === badge.id }"
                                :style="{
                                    left: `${badge.x}px`,
                                    top: `${badge.y}px`,
                                    width: `${badge.width}px`,
                                }"
                                :title="`${featureResultLabel(badge.feature)} · ${badge.feature.x}, ${badge.feature.z}`"
                                :aria-pressed="selectedFeatureKey === badge.id"
                                data-testid="seed-map-structure-badge"
                                @pointerdown.stop
                                @dblclick.stop
                                @keydown.stop
                                @click="selectStructureBadge(badge.feature)"
                            >
                                <img
                                    :src="
                                        badge.feature.kind === 'end-city' &&
                                        badge.feature.endShip !== undefined
                                            ? endCityImageSources[
                                                  badge.feature.endShip ? 'ship' : 'noShip'
                                              ]
                                            : featureImageSource(badge.feature.kind)
                                    "
                                    alt=""
                                    class="shrink-0 [image-rendering:pixelated]"
                                    :style="{
                                        width: `${20 * poiIconScale}px`,
                                        height: `${20 * poiIconScale}px`,
                                    }"
                                />
                                <span class="truncate">{{
                                    featureResultLabel(badge.feature)
                                }}</span>
                            </button>

                            <div class="map-coordinate-ruler map-coordinate-ruler-x">
                                <span v-for="offset in [-2, -1, 0, 1, 2]" :key="offset">{{
                                    Math.round(
                                        viewBounds().minX +
                                            ((offset + 2) * visibleSize.width * scale) / 4,
                                    )
                                }}</span>
                            </div>
                            <div class="map-coordinate-ruler map-coordinate-ruler-z">
                                <span v-for="offset in [-1, 0, 1]" :key="offset">{{
                                    Math.round(
                                        viewBounds().minZ +
                                            ((offset + 1.5) * visibleSize.height * scale) / 3,
                                    )
                                }}</span>
                            </div>

                            <div class="map-control-stack map-control-stack-left left-3">
                                <Button
                                    v-tooltip="formatMessage(messages.zoomIn)"
                                    circular
                                    icon-only
                                    :aria-label="formatMessage(messages.zoomIn)"
                                    :disabled="workspace.zoom <= SEED_MAP_MIN_ZOOM"
                                    @click="zoomBy(-0.5)"
                                    ><PlusIcon
                                /></Button>
                                <Button
                                    v-tooltip="formatMessage(messages.zoomOut)"
                                    circular
                                    icon-only
                                    :aria-label="formatMessage(messages.zoomOut)"
                                    :disabled="workspace.zoom >= SEED_MAP_SCALES.length - 1"
                                    @click="zoomBy(0.5)"
                                    ><MinusIcon
                                /></Button>
                                <Button
                                    v-tooltip="formatMessage(messages.centerSpawn)"
                                    circular
                                    icon-only
                                    :aria-label="formatMessage(messages.centerSpawn)"
                                    :disabled="!spawn"
                                    @click="centerSpawn"
                                    ><CompassIcon
                                /></Button>
                            </div>

                            <div class="map-control-stack map-control-stack-right right-3">
                                <Button
                                    v-tooltip="
                                        formatMessage(
                                            isFullscreen
                                                ? messages.exitFullscreen
                                                : messages.fullscreen,
                                        )
                                    "
                                    circular
                                    icon-only
                                    :aria-label="
                                        formatMessage(
                                            isFullscreen
                                                ? messages.exitFullscreen
                                                : messages.fullscreen,
                                        )
                                    "
                                    @click="toggleFullscreen"
                                    ><ContractIcon v-if="isFullscreen" /><ExpandIcon v-else
                                /></Button>
                                <Button
                                    v-tooltip="formatMessage(messages.ruler)"
                                    :type="rulerEnabled ? 'base' : 'outlined'"
                                    circular
                                    icon-only
                                    :aria-label="formatMessage(messages.ruler)"
                                    :aria-pressed="rulerEnabled"
                                    @click="rulerEnabled = !rulerEnabled"
                                    ><ScaleIcon
                                /></Button>
                                <Button
                                    v-tooltip="formatMessage(messages.mapSettings)"
                                    :type="advancedOpen ? 'base' : 'outlined'"
                                    circular
                                    icon-only
                                    :aria-label="formatMessage(messages.mapSettings)"
                                    :aria-pressed="advancedOpen"
                                    @click="advancedOpen = !advancedOpen"
                                    ><SettingsIcon
                                /></Button>
                            </div>

                            <div v-if="oreZoomLimited" class="map-status map-ore-status">
                                <PickaxeIcon class="size-3" />{{
                                    formatMessage(messages.zoomToScanOres)
                                }}
                            </div>
                            <div
                                v-else-if="
                                    workspace.displayMode === 'ores' &&
                                    workspace.dimension === 'end'
                                "
                                class="map-status map-ore-status"
                            >
                                <PickaxeIcon class="size-3" />{{
                                    formatMessage(messages.noOresInEnd)
                                }}
                            </div>
                            <div v-else-if="mapBusy" class="map-status map-ore-status">
                                <RefreshCwIcon class="size-3 animate-spin" />{{
                                    oreScanning
                                        ? formatMessage(messages.scanningOres, {
                                              processed: oreScannedChunks,
                                              total: oreTotalChunks,
                                          })
                                        : formatMessage(messages.loading)
                                }}
                            </div>
                            <div
                                v-if="
                                    workspace.displayMode === 'ores' && !mapBusy && !oreZoomLimited
                                "
                                class="map-status map-ore-status"
                            >
                                {{
                                    formatMessage(
                                        oreScanTruncated
                                            ? messages.oreTruncated
                                            : messages.oreCandidate,
                                    )
                                }}
                            </div>
                            <div v-if="rulerDistance !== null" class="map-status map-ruler-status">
                                <ScaleIcon class="size-3" />{{
                                    formatMessage(messages.distance, { distance: rulerDistance })
                                }}<Button
                                    v-tooltip="formatMessage(messages.clearRuler)"
                                    type="quiet"
                                    size="2xs"
                                    circular
                                    icon-only
                                    :aria-label="formatMessage(messages.clearRuler)"
                                    @click="rulerPoints = []"
                                    ><XIcon
                                /></Button>
                            </div>

                            <div
                                v-if="selection && selectionVisible && !structureQuery"
                                class="map-popup"
                                :style="selectionStyle"
                            >
                                <div class="flex items-start justify-between gap-3">
                                    <div class="min-w-0">
                                        <strong
                                            class="block truncate text-sm text-[var(--color-text-primary)]"
                                        >
                                            <i
                                                v-if="
                                                    selection.biome !== undefined && !selectedMarker
                                                "
                                                class="popup-biome-dot"
                                                :style="{
                                                    backgroundColor: biomeColorOf(selection.biome),
                                                }"
                                            ></i
                                            >{{
                                                selection.feature
                                                    ? featureResultLabel(selection.feature)
                                                    : selection.ore
                                                      ? oreLabel(selection.ore.ore)
                                                      : selection.spawn
                                                        ? formatMessage(messages.spawnPoint)
                                                        : selectedMarker?.name ||
                                                          (selection.biome !== undefined
                                                              ? biomeDisplayName(selection.biome)
                                                              : formatMessage(messages.location))
                                            }}
                                        </strong>
                                        <span class="text-xs text-[var(--color-text-tertiary)]"
                                            >X {{ selection.x
                                            }}<template v-if="selection.ore"
                                                >, Y {{ selection.ore.y }}</template
                                            >, Z {{ selection.z
                                            }}<template
                                                v-if="
                                                    selection.biome !== undefined && !selectedMarker
                                                "
                                            >
                                                · {{ formatMessage(messages.biomeTag) }}</template
                                            ></span
                                        >
                                    </div>
                                    <Button
                                        type="quiet"
                                        size="2xs"
                                        circular
                                        icon-only
                                        :aria-label="formatMessage(messages.closePanel)"
                                        @click="selection = null"
                                        ><XIcon
                                    /></Button>
                                </div>
                                <div v-if="selection.ore" class="ore-hit-details">
                                    <span>{{ formatMessage(messages.oreCandidate) }}</span>
                                    <span>{{
                                        formatMessage(messages.oreCoverage, {
                                            min: selection.ore.yMin,
                                            max: selection.ore.yMax,
                                        })
                                    }}</span>
                                </div>
                                <div
                                    v-if="markerDraftOpen"
                                    class="mt-3 grid grid-cols-[minmax(0,1fr)_2.25rem] gap-2"
                                >
                                    <StyledInput
                                        v-model="markerName"
                                        :placeholder="formatMessage(messages.markerName)"
                                    />
                                    <input
                                        v-model="markerColor"
                                        type="color"
                                        class="marker-color-input"
                                        :aria-label="formatMessage(messages.markerColor)"
                                    />
                                </div>
                                <div class="mt-3 flex flex-wrap gap-2">
                                    <Button
                                        v-if="markerDraftOpen"
                                        type="colored"
                                        color="brand"
                                        size="2xs"
                                        @click="addMarker"
                                        ><PinIcon />{{ formatMessage(messages.addMarker) }}
                                    </Button>
                                    <Button type="outlined" size="2xs" @click="copyTeleport"
                                        ><ClipboardCopyIcon />{{
                                            formatMessage(messages.copyTeleport)
                                        }}
                                    </Button>
                                    <Button
                                        v-if="!markerDraftOpen && !selectedMarker"
                                        type="outlined"
                                        size="2xs"
                                        @click="markerDraftOpen = true"
                                        ><PlusIcon />{{ formatMessage(messages.addMarker) }}
                                    </Button>
                                    <Checkbox
                                        v-if="selection.feature"
                                        v-model="selectedFeatureCompleted"
                                        class="feature-completed-checkbox"
                                        :label="formatMessage(messages.completed)"
                                    />
                                    <Checkbox
                                        v-if="selection.ore"
                                        v-model="selectedOreCompleted"
                                        class="feature-completed-checkbox"
                                        :label="formatMessage(messages.mined)"
                                    />
                                    <Button
                                        v-if="selectedMarker"
                                        type="outlined"
                                        size="2xs"
                                        @click="removeMarker(selectedMarker.id)"
                                        ><TrashIcon />{{ formatMessage(messages.removeMarker) }}
                                    </Button>
                                </div>
                            </div>

                            <div v-if="advancedOpen" class="advanced-panel">
                                <div class="flex items-center justify-between gap-3">
                                    <strong class="text-sm text-[var(--color-text-primary)]">{{
                                        formatMessage(messages.mapSettings)
                                    }}</strong
                                    ><Button
                                        type="quiet"
                                        size="2xs"
                                        circular
                                        icon-only
                                        :aria-label="formatMessage(messages.closePanel)"
                                        @click="advancedOpen = false"
                                        ><XIcon
                                    /></Button>
                                </div>
                                <label class="advanced-panel-option"
                                    ><span>{{ formatMessage(messages.chunkCoordinates) }}</span
                                    ><Toggle v-model="workspace.showChunkCoordinates" small
                                /></label>
                                <label class="advanced-panel-option"
                                    ><span>{{ formatMessage(messages.contours) }}</span
                                    ><Toggle
                                        v-model="contoursEnabled"
                                        :disabled="workspace.dimension === 'nether'"
                                        small
                                /></label>
                                <div class="mt-3 border-t border-surface-5 pt-3">
                                    <div class="mb-2 flex items-center justify-between">
                                        <strong class="text-sm text-[var(--color-text-primary)]">{{
                                            formatMessage(messages.markers)
                                        }}</strong
                                        ><span class="text-xs text-[var(--color-text-tertiary)]">{{
                                            dimensionMarkers.length
                                        }}</span>
                                    </div>
                                    <p
                                        v-if="!dimensionMarkers.length"
                                        class="m-0 text-xs text-[var(--color-text-tertiary)]"
                                    >
                                        {{ formatMessage(messages.noMarkers) }}
                                    </p>
                                    <div v-else class="marker-list flex flex-col gap-1">
                                        <div v-for="marker in dimensionMarkers" :key="marker.id">
                                            <button
                                                class="marker-jump"
                                                @click="jumpToMarker(marker)"
                                            >
                                                <i :style="{ backgroundColor: marker.color }"></i
                                                ><span>{{ marker.name }}</span
                                                ><small>{{ marker.x }}, {{ marker.z }}</small>
                                            </button>
                                            <Button
                                                type="quiet"
                                                size="2xs"
                                                circular
                                                icon-only
                                                :aria-label="formatMessage(messages.removeMarker)"
                                                @click="removeMarker(marker.id)"
                                                ><TrashIcon
                                            /></Button>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </template>
                        <SeedMapUndergroundPreview
                            v-if="workspace.displayMode === 'underground' && undergroundSupported"
                            :key="`${workspace.seed}:${workspace.edition}:${workspace.gameVersion}:${workspace.dimension}`"
                            :query="{
                                seed: workspace.seed,
                                edition: workspace.edition,
                                version: workspace.gameVersion,
                                dimension: workspace.dimension,
                                x: workspace.center.x,
                                y: workspace.undergroundY,
                                z: workspace.center.z,
                            }"
                            @close="setDisplayMode('structures')"
                        />
                        <div
                            v-if="workspace.displayMode !== 'underground'"
                            class="map-bottom-toolbar"
                        >
                            <SeedMapBiomePicker
                                class="map-bottom-biomes"
                                :dimension="workspace.dimension"
                                :enabled="workspace.highlightBiomeEnabled"
                                :highlighted-biomes="workspace.highlightedBiomes"
                                :available-biome-ids="selectedProfile?.biomes"
                                :available-dimensions="dimensions"
                                :container="isFullscreen ? fullscreenContainer : undefined"
                                @update:dimension="workspace.dimension = $event"
                                @update:enabled="workspace.highlightBiomeEnabled = $event"
                                @update:highlighted-biomes="workspace.highlightedBiomes = $event"
                            />
                            <div
                                class="map-bottom-actions max-sm:ml-0 max-sm:w-full max-sm:flex-wrap max-sm:justify-start"
                            >
                                <Button
                                    :type="terrainEnabled ? 'chip-text' : 'outlined'"
                                    color="brand"
                                    class="map-action"
                                    :disabled="!terrainSupported"
                                    :aria-pressed="terrainEnabled"
                                    @click="terrainEnabled = !terrainEnabled"
                                >
                                    <LayersIcon />{{ formatMessage(messages.terrain) }}
                                </Button>
                                <Button
                                    :type="workspace.showGrid ? 'chip-text' : 'outlined'"
                                    color="brand"
                                    class="map-action"
                                    :aria-pressed="workspace.showGrid"
                                    @click="workspace.showGrid = !workspace.showGrid"
                                >
                                    <GridIcon />{{ formatMessage(messages.showGrid) }}
                                </Button>
                                <Button
                                    v-if="workspace.displayMode === 'structures'"
                                    type="outlined"
                                    class="map-action"
                                    :disabled="!displayedFeatures.length"
                                    @click="focusNearestFeature"
                                    ><SearchIcon />{{ formatMessage(messages.searchNearby) }}
                                </Button>
                            </div>
                        </div>
                    </div>
                </template>
                <template #details>
                    <SeedMapAreaAnalysis
                        v-if="selectedArea"
                        :area="selectedArea"
                        :context="{
                            seed: workspace.seed,
                            edition: workspace.edition,
                            version: workspace.gameVersion,
                            dimension: workspace.dimension,
                        }"
                        :ores="dimensionOres.map((ore) => ore.kind)"
                        :y-range="
                            selectedProfile?.undergroundYRanges?.[dimensionsIndex] ??
                            (workspace.dimension === 'nether' ? [0, 127] : [-64, 319])
                        "
                        :feature-mask="
                            selectedProfile?.featureMasks?.[dimensionsIndex] ?? 0x3ffffff
                        "
                        :feature-label="featureLabel"
                        :ore-label="oreLabel"
                        @close="selectedArea = null"
                        @locate="locateAreaLoot"
                    />
                    <SeedMapStructureDetails
                        v-else-if="structureQuery && selection?.feature"
                        :query="structureQuery"
                        :kind="selection.feature.kind"
                        :title="featureResultLabel(selection.feature)"
                        :selected="focusedPiece?.id"
                        :load="layoutLayer.load"
                        @close="closeStructureDetails"
                        @focus="focusStructurePiece"
                    >
                        <template #actions>
                            <div
                                v-if="markerDraftOpen"
                                class="grid grid-cols-[minmax(0,1fr)_2.25rem] gap-2"
                            >
                                <StyledInput
                                    v-model="markerName"
                                    :placeholder="formatMessage(messages.markerName)"
                                />
                                <input
                                    v-model="markerColor"
                                    type="color"
                                    class="size-9 rounded border border-surface-4 bg-surface-3"
                                    :aria-label="formatMessage(messages.markerColor)"
                                />
                            </div>
                            <div class="flex flex-wrap items-center gap-2">
                                <Button type="outlined" size="2xs" @click="copyTeleport"
                                    ><ClipboardCopyIcon />{{
                                        formatMessage(messages.copyTeleport)
                                    }}</Button
                                >
                                <Button
                                    :type="markerDraftOpen ? 'colored' : 'outlined'"
                                    size="2xs"
                                    @click="
                                        markerDraftOpen ? addMarker() : (markerDraftOpen = true)
                                    "
                                    ><PinIcon />{{ formatMessage(messages.addMarker) }}</Button
                                >
                                <Checkbox
                                    v-model="selectedFeatureCompleted"
                                    :label="formatMessage(messages.completed)"
                                />
                            </div>
                        </template>
                    </SeedMapStructureDetails>
                </template>
            </SeedMapSplitView>
        </section>

        <SeedMapWorldImportModal ref="worldImportModal" @import="onWorldImported" />
        <SeedMapCopyrightModal ref="copyrightModal" />
    </main>
</template>

<style scoped>
.map-scale-badge {
    flex: 0 0 auto;
    min-width: 7.5rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    padding: 0.35rem 0.55rem;
    color: var(--color-text-tertiary);
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    text-align: center;
}

.seed-map-toolbar {
    position: relative;
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    padding: 0.75rem;
}

.toolbar-primary {
    display: grid;
    grid-template-columns: minmax(11rem, 0.8fr) minmax(7rem, 0.45fr) minmax(18rem, 1.75fr);
    gap: 0.75rem;
}

/*
 * The secondary row is a strict three-column grid whose controls all share
 * a 2.5rem height, so the dimension switch, both coordinate fields, Go, and
 * Share stay width- and baseline-aligned.
 */
.toolbar-secondary {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: end;
    gap: 0.75rem;
    border-top: 1px solid var(--surface-5);
    padding-top: 0.75rem;
}

.control-group {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 0.35rem;
}

.control-label {
    color: var(--color-text-tertiary);
    font-size: 0.7rem;
    font-weight: 700;
    line-height: 1;
}

:deep(.seed-map-dropdown [data-combobox]) {
    width: 100%;
    max-width: none;
}

.seed-input-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto auto;
    align-items: center;
    gap: 0.5rem;
}

.seed-history-menu {
    display: flex;
    width: min(23rem, calc(1280px - 1.5rem));
    max-height: min(24rem, calc(800px - 4rem));
    min-height: 0;
    flex-direction: column;
    gap: 0;
    overflow: hidden;
}

.seed-history-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    border-bottom: 1px solid var(--seed-history-border, var(--surface-5));
    padding: 0.1rem 0.2rem 0.6rem;
    color: var(--color-text-primary);
    font-size: 0.8rem;
}

.seed-history-empty {
    margin: 0;
    padding: 1.25rem 0.5rem;
    color: var(--color-text-tertiary);
    font-size: 0.78rem;
    text-align: center;
}

.seed-history-list {
    display: flex;
    min-height: 0;
    flex-direction: column;
    gap: 0.35rem;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 0.6rem 0.2rem 0.1rem 0;
    scrollbar-color: var(--seed-history-border, var(--surface-5)) transparent;
    scrollbar-width: thin;
}

.seed-history-row {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    border: 1px solid var(--seed-history-border, var(--surface-5));
    border-radius: var(--radius-sm);
    background: var(--seed-history-row-bg, var(--surface-4));
    padding-right: 0.25rem;
    transition:
        background-color 0.15s ease,
        border-color 0.15s ease,
        box-shadow 0.15s ease;
}

.seed-history-row:hover {
    border-color: color-mix(
        in srgb,
        var(--color-brand) 45%,
        var(--seed-history-border, var(--surface-5))
    );
    background: var(--seed-history-row-hover-bg, var(--surface-3));
}

.seed-history-row:focus-within {
    border-color: var(--color-brand);
    box-shadow: 0 0 0 2px var(--color-brand-highlight);
}

.seed-history-load {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    gap: 0.15rem;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    padding: 0.5rem 0.6rem;
    cursor: pointer;
    text-align: left;
}

.seed-history-load:focus-visible {
    outline: none;
}

.seed-history-seed {
    overflow: hidden;
    color: var(--color-text-primary);
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.seed-history-meta {
    overflow: hidden;
    color: var(--color-text-tertiary);
    font-size: 0.68rem;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.seed-history-progress {
    display: inline-flex;
    align-items: center;
    border: 1px solid var(--color-brand-highlight);
    border-radius: var(--radius-sm);
    background: var(--color-brand-highlight);
    margin-left: 0.3rem;
    padding: 0 0.3rem;
    color: var(--color-brand);
    font-weight: 700;
}

.seed-history-remove {
    --_text: var(--color-text-tertiary);
    --_icon: var(--color-text-tertiary);
    --_hover-bg: var(--color-red-highlight);
    --_hover-text: var(--color-red);
    --_hover-icon: var(--color-red);
}

:global(.seed-map-history-popout) {
    --seed-history-panel-bg: var(--surface-2);
    --seed-history-row-bg: var(--surface-4);
    --seed-history-row-hover-bg: var(--surface-3);
    --seed-history-border: var(--surface-5);
    --seed-history-shadow: 0 18px 48px color-mix(in srgb, var(--surface-5) 55%, transparent);
    z-index: 10050 !important;
}

:global(html.dark-mode .seed-map-history-popout),
:global(html.oled-mode .seed-map-history-popout) {
    --seed-history-panel-bg: color-mix(in srgb, var(--surface-4) 88%, var(--color-text-primary));
    --seed-history-row-bg: color-mix(in srgb, var(--surface-5) 82%, var(--color-text-primary));
    --seed-history-row-hover-bg: color-mix(
        in srgb,
        var(--surface-5) 70%,
        var(--color-text-primary)
    );
    --seed-history-border: color-mix(in srgb, var(--surface-5) 80%, var(--color-text-primary));
    --seed-history-shadow: 0 18px 48px color-mix(in srgb, var(--surface-5) 38%, transparent);
}

:global(.seed-map-history-popout) {
    border-color: var(--seed-history-border) !important;
    background-color: var(--seed-history-panel-bg) !important;
    box-shadow: var(--seed-history-shadow) !important;
}

:global(.seed-map-history-popout .menu-arrow) {
    fill: var(--seed-history-panel-bg);
    stroke: var(--seed-history-border);
}

.popup-biome-dot {
    display: inline-block;
    width: 0.6rem;
    height: 0.6rem;
    margin-right: 0.4rem;
    border: 1px solid var(--surface-5);
    border-radius: 50%;
}

.dimension-control > :deep([data-button].dimension-option) {
    height: 100%;
    min-width: 5.5rem;
    justify-content: center;
    border-radius: 0;
}

.dimension-control > :deep([data-button].dimension-option:first-child) {
    border-radius: calc(var(--radius-md) - 1px) 0 0 calc(var(--radius-md) - 1px);
}

.dimension-control > :deep([data-button].dimension-option:last-child) {
    border-radius: 0 calc(var(--radius-md) - 1px) calc(var(--radius-md) - 1px) 0;
}

.coordinate-control {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.5rem;
}

.coordinate-control :deep(input) {
    height: 2.5rem;
}

.coordinate-field > span {
    position: absolute;
    z-index: 2;
    left: 0.65rem;
    color: var(--color-text-tertiary);
    font-size: 0.7rem;
    font-weight: 800;
    pointer-events: none;
}

.coordinate-field :deep(.relative) {
    width: 100%;
}

.coordinate-field :deep(input) {
    padding-left: 1.75rem;
}

.share-button :deep(button) {
    height: 2.5rem;
}

.seed-map-experience {
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-md);
    outline: none;
}

.seed-map-experience:fullscreen {
    /* Keep the fullscreen surface inside the viewport, including its padding. */
    box-sizing: border-box;
    width: 100vw;
    height: 100dvh;
    max-height: 100dvh;
    min-height: 0;
    overflow: hidden;
    padding: 0.75rem;
}

.seed-map-experience:fullscreen .seed-map-workspace-panes {
    /* The normal map has a 32rem minimum for usability; fullscreen must be able
	 * to shrink so the toolbar and canvas do not push the surface past the viewport. */
    min-height: 0;
}

.layer-strip {
    position: relative;
    z-index: 10;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 0.5rem;
    min-height: 4rem;
    border: 1px solid var(--surface-5);
    padding: 0.45rem 0.6rem;
    border-radius: var(--radius-md) var(--radius-md) 0 0;
    background: var(--surface-3);
    box-shadow: 0 8px 20px rgb(0 0 0 / 0.2);
}

/*
 * The expanded checklist floats above the map canvas so its options are never
 * covered by map overlays and the map does not shift while it is open.
 */
.layer-expanded-panel {
    position: absolute;
    z-index: 30;
    top: calc(100% + 0.4rem);
    right: -1px;
    left: -1px;
    display: flex;
    max-height: min(26rem, 448px);
    flex-direction: column;
    gap: 0.5rem;
    overflow: hidden;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-3);
    box-shadow: 0 16px 34px rgb(0 0 0 / 0.36);
    padding: 0.6rem;
}

.layer-strip-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.25rem;
    padding-left: 0.25rem;
    color: var(--color-text-tertiary);
    font-size: 0.75rem;
    font-weight: 800;
    text-transform: uppercase;
}

.layer-strip-heading > span {
    white-space: nowrap;
}

.layer-mode-switch {
    display: flex;
    align-items: center;
    gap: 0.2rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    padding: 0.2rem;
    text-transform: none;
}

.layer-mode-switch :deep(button) {
    border-radius: var(--radius-sm);
    white-space: nowrap;
}

.layer-bulk-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
}

.layer-options {
    display: flex;
    min-width: 0;
    gap: 0.4rem;
    overflow-x: auto;
    padding: 0.1rem;
    scrollbar-width: none;
}

.layer-options::-webkit-scrollbar {
    display: none;
}

.layer-checklist {
    display: grid;
    min-height: 0;
    flex: 1;
    grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
    gap: 0.35rem 0.75rem;
    overflow-y: auto;
    overscroll-behavior: contain;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    padding: 0.65rem;
}

.layer-checklist > :deep(button) {
    min-width: 0;
    border-radius: var(--radius-sm);
    padding: 0.35rem;
}

.layer-checklist > :deep(button:hover) {
    background: var(--surface-4);
}

.layer-checklist-label img,
.layer-checklist-label svg {
    width: 1.35rem;
    height: 1.35rem;
    flex: 0 0 auto;
    object-fit: contain;
}

.layer-checklist-label span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.layer-checklist-label small {
    margin-left: auto;
    color: var(--color-text-tertiary);
    font-size: 0.68rem;
    font-variant-numeric: tabular-nums;
}

.layer-empty {
    display: inline-flex;
    align-items: center;
    padding: 0.35rem 0.5rem;
    color: var(--color-text-tertiary);
    font-size: 0.75rem;
}

.ore-range-controls {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.65rem 1rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    padding: 0.65rem;
}

.ore-range-heading {
    display: flex;
    grid-column: 1 / -1;
    align-items: center;
    gap: 0.65rem;
    color: var(--color-text-primary);
    font-size: 0.75rem;
}

.ore-range-heading span {
    color: var(--color-text-tertiary);
    font-variant-numeric: tabular-nums;
}

.ore-range-heading > :last-child {
    margin-left: auto;
}

.ore-range-control {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 0.4rem;
    color: var(--color-text-tertiary);
    font-size: 0.7rem;
    font-weight: 700;
}

.layer-option svg,
.layer-option img {
    width: 1.5rem;
    height: 1.5rem;
    flex: 0 0 auto;
    object-fit: contain;
}

.layer-option.named {
    width: auto;
    min-width: 8rem;
    justify-content: flex-start;
    padding: 0 0.65rem;
}

.layer-option.named span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.seed-map-canvas-shell > canvas {
    image-rendering: pixelated;
}

.map-control-stack {
    position: absolute;
    z-index: 7;
    top: 2.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-3);
    box-shadow: 0 8px 20px rgb(0 0 0 / 0.28);
    padding: 0.35rem;
}

.map-control-stack :deep(button) {
    border-radius: var(--radius-sm);
}

.map-coordinate-ruler {
    pointer-events: none;
    position: absolute;
    z-index: 2;
    display: flex;
    color: var(--color-text-primary);
    font-size: 0.65rem;
    font-weight: 700;
}

.map-coordinate-ruler-x {
    inset: 0 3.5rem auto 3.5rem;
    justify-content: space-between;
}

.map-coordinate-ruler-x span,
.map-coordinate-ruler-z span {
    border: 1px solid var(--surface-5);
    border-radius: 0 0 var(--radius-sm) var(--radius-sm);
    background: var(--surface-3);
    padding: 0.15rem 0.35rem;
    box-shadow: 0 3px 8px rgb(0 0 0 / 0.24);
}

.map-coordinate-ruler-z {
    inset: 3rem auto 3rem 0;
    align-items: flex-start;
    flex-direction: column;
    justify-content: space-between;
}

.map-coordinate-ruler-z span {
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
}

.map-status {
    position: absolute;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-3);
    padding: 0.35rem 0.55rem;
    color: var(--color-text-tertiary);
    font-size: 0.72rem;
    font-weight: 700;
}

.map-status > :deep(button) {
    color: var(--color-text-tertiary);
}

.map-status > button svg {
    width: 0.9rem;
    height: 0.9rem;
}

.map-ruler-status {
    bottom: 5.25rem;
    left: 0.75rem;
}

.map-ore-status {
    top: 0.75rem;
    left: 50%;
    transform: translateX(-50%);
}

.map-popup,
.advanced-panel {
    position: absolute;
    z-index: 8;
    width: min(17rem, calc(100% - 1rem));
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-3);
    box-shadow: 0 12px 28px rgb(0 0 0 / 0.28);
    padding: 0.75rem;
}

.advanced-panel {
    right: 0.75rem;
    bottom: 4.3rem;
    max-height: min(28rem, calc(100% - 7rem));
    overflow-y: auto;
}

.advanced-panel-option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin-top: 0.75rem;
    color: var(--color-text-primary);
    font-size: 0.8rem;
}

.marker-list > div {
    display: flex;
    align-items: center;
    gap: 0.35rem;
}

.marker-jump {
    display: grid;
    min-width: 0;
    flex: 1;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.45rem;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    padding: 0.35rem;
    color: var(--color-text-primary);
    text-align: left;
}

.marker-jump:hover {
    background: var(--surface-4);
}

.marker-jump i {
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 50%;
}

.marker-jump span {
    overflow: hidden;
    font-size: 0.75rem;
    font-style: normal;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.marker-jump small {
    color: var(--color-text-tertiary);
    font-size: 0.65rem;
}

.feature-completed-checkbox {
    min-height: 2rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    background: var(--surface-4);
    padding: 0.35rem 0.55rem;
}

.ore-hit-details {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    margin-top: 0.6rem;
}

.ore-hit-details span {
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    background: var(--surface-4);
    padding: 0.2rem 0.4rem;
    color: var(--color-text-tertiary);
    font-size: 0.68rem;
    font-weight: 700;
}

.ore-hit-details span.verified {
    border-color: var(--color-brand-highlight);
    background: var(--color-brand-highlight);
    color: var(--color-brand);
}

/*
 * One flat row of same-height controls: the biome cluster grows, the height
 * pill stays compact, and the action buttons on the right share one width so
 * the bar reads as a single aligned toolbar instead of mismatched boxes.
 */
.map-bottom-toolbar {
    position: absolute;
    z-index: 6;
    right: 0.75rem;
    bottom: 0.75rem;
    left: 0.75rem;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-3);
    padding: 0.5rem 0.6rem;
    box-shadow: 0 10px 26px rgb(0 0 0 / 0.3);
}

.map-bottom-biomes {
    min-width: min(20rem, 100%);
    flex: 1 1 20rem;
}

.map-bottom-actions {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 0.5rem;
    margin-left: auto;
}

.map-bottom-actions :deep(button.map-action) {
    min-width: 7.25rem;
    height: 2.5rem;
    justify-content: center;
    white-space: nowrap;
}

.marker-color-input {
    width: 2.25rem;
    height: 2.25rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    background: var(--surface-4);
    padding: 0.2rem;
}

@media (max-width: 1200px) {
    .toolbar-primary {
        grid-template-columns: minmax(11rem, 1fr) minmax(8rem, 0.7fr);
    }

    .seed-control {
        grid-column: 1 / -1;
    }

    .toolbar-secondary {
        grid-template-columns: auto minmax(0, 1fr);
    }

    .share-button {
        grid-column: 1 / -1;
        justify-self: end;
    }

    .coordinate-group {
        min-width: min(100%, 22rem);
    }

    .map-ruler-status {
        bottom: 8.5rem;
    }

    .advanced-panel {
        bottom: 7.5rem;
    }
}

@media (max-width: 900px) {
    .seed-map-page {
        height: auto;
        min-height: 100%;
    }

    .toolbar-primary {
        display: flex;
        flex-direction: column;
    }

    .toolbar-secondary {
        grid-template-columns: minmax(0, 1fr);
    }

    .dimension-group,
    .coordinate-group {
        width: 100%;
        min-width: 0;
    }

    .dimension-control,
    .coordinate-control {
        max-width: 100%;
        flex-wrap: wrap;
    }

    .coordinate-field {
        min-width: 7rem;
    }

    .share-button {
        margin-left: 0;
    }

    .share-button :deep(button) {
        width: 100%;
    }

    .layer-strip {
        grid-template-columns: 1fr;
    }

    .layer-strip-heading {
        flex-wrap: wrap;
        justify-content: space-between;
    }

    .layer-mode-switch {
        order: 3;
        width: 100%;
    }

    .layer-mode-switch :deep(.button-base),
    .layer-mode-switch :deep(button) {
        flex: 1;
    }
}

@media (max-width: 640px) {
    .layer-checklist {
        grid-template-columns: minmax(0, 1fr);
    }

    .map-bottom-toolbar {
        max-height: calc(100% - 5rem);
        overflow-y: auto;
    }

    .map-ruler-status {
        bottom: 12rem;
    }

    .advanced-panel {
        bottom: 10.5rem;
    }
    .ore-range-controls {
        grid-template-columns: minmax(0, 1fr);
    }
}
</style>

<style>
/* The seed map is a fixed work surface on desktop. Give the route layers a
 * definite height so the page's flex-1 map area cannot grow after a fullscreen
 * enter/exit reflow and create a second page scrollbar. */
@media (min-width: 901px) {
    .app-viewport:has(.seed-map-page),
    .app-viewport:has(.seed-map-page) .page-transition-grid,
    .app-viewport:has(.seed-map-page) .page-transition-layer {
        height: 100%;
        min-height: 0;
        overflow: hidden;
    }
}
</style>
