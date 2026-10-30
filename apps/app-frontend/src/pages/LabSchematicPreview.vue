<script setup lang="ts">
import {
    ArrowLeftRightIcon,
    ArrowUpDownIcon,
    BoxesIcon,
    BoxIcon,
    CheckIcon,
    ChevronDownIcon,
    ContractIcon,
    CopyIcon,
    CubeIcon,
    DownloadIcon,
    EditIcon,
    EyeIcon,
    EyeOffIcon,
    FileArchiveIcon,
    FolderSearchIcon,
    GridIcon,
    ImageIcon,
    InfoIcon,
    LayersIcon,
    ListIcon,
    MaximizeIcon,
    MoreHorizontalIcon,
    MoveIcon,
    RedoIcon,
    RefreshCwIcon,
    RotateClockwiseIcon,
    RotateCounterClockwiseIcon,
    SaveIcon,
    ScanEyeIcon,
    SearchIcon,
    SettingsIcon,
    SpinnerIcon,
    TrashIcon,
    TriangleAlertIcon,
    UndoIcon,
    UnfoldVerticalIcon,
    XIcon,
} from '@modrinth/assets'
import {
    Button,
    defineMessages,
    EmptyState,
    injectNotificationManager,
    OverflowMenu,
    type OverflowMenuOption,
    Slider,
    StyledInput,
    Tabs,
    TagItem,
    useVIntl,
} from '@modrinth/ui'
import { isTauri } from '@tauri-apps/api/core'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { open, save } from '@tauri-apps/plugin-dialog'
import { writeFile, writeTextFile } from '@tauri-apps/plugin-fs'
import { platform } from '@tauri-apps/plugin-os'
import {
    computed,
    nextTick,
    onBeforeUnmount,
    onMounted,
    reactive,
    ref,
    shallowRef,
    useTemplateRef,
    watch,
} from 'vue'
import { useRoute } from 'vue-router'

import SchematicBlockPickerModal from '@/components/lab/schematic-preview/SchematicBlockPickerModal.vue'
import SchematicInfoModal from '@/components/lab/schematic-preview/SchematicInfoModal.vue'
import SchematicInstancePickerModal from '@/components/lab/schematic-preview/SchematicInstancePickerModal.vue'
import SchematicMaterialSwatch from '@/components/lab/schematic-preview/SchematicMaterialSwatch.vue'
import SchematicStateCard from '@/components/lab/schematic-preview/SchematicStateCard.vue'
import { list } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types.d.ts'
import {
    applySchematicEdits,
    cancelSchematicPreview,
    closeSchematicPreview,
    exportSchematicLitematic,
    exportSchematicSponge,
    openSchematicPreview,
    readSchematicChunk,
    type SchematicBlockEdit,
    type SchematicBlockState,
    type SchematicPreviewManifest,
    type SchematicPreviewSource,
    type SchematicTransform,
    transformSchematic,
} from '@/lab/schematic-preview/backend'
import { schematicStateProperties } from '@/lab/schematic-preview/block-state'
import {
    filterSchematicAirGeometry,
    isSchematicAir,
    measureSchematicPoints,
    normalizeSchematicAirBlocks,
    schematicBlockKey,
    type SchematicBlockLocation,
    schematicBlockPaletteIndex,
    schematicBlockStateKey,
    type SchematicCachedChunk,
    schematicChunkKey,
    schematicSelectionBounds,
    selectConnectedSchematicBlocks,
    selectSchematicBlockState,
    selectSchematicCuboid,
    selectSchematicLayer,
    selectSchematicMaterial,
} from '@/lab/schematic-preview/editing'
import type {
    SchematicMeshWorkerRequest,
    SchematicMeshWorkerResponse,
} from '@/lab/schematic-preview/mesh-worker'
import {
    extractSchematicNeighborFace,
    SCHEMATIC_DIRECTIONS,
    type SchematicDirection,
    schematicNeighborChunkPosition,
} from '@/lab/schematic-preview/meshing'
import {
    createSchematicResources,
    type LoadedSchematicResources,
    minecraftVersionFromDataVersion,
    resolveSchematicBlockName,
    resolveSchematicMaterialTexture,
} from '@/lab/schematic-preview/resources'
import {
    SchematicPreviewScene,
    type SchematicSceneSelection,
    type ViewMode,
} from '@/lab/schematic-preview/scene'
import {
    clearRecentSchematics,
    loadRecentSchematics,
    type RecentSchematic,
    recordRecentSchematic,
    removeRecentSchematic,
} from '@/lab/schematic-preview/storage'
import { escapeSchematicCsvCell } from '@/lab/schematic-preview/utils'

type InspectorTab = 'edit' | 'materials'
type WorkspaceTool = 'select' | 'box' | 'measure' | 'layer-spacing'
type LoadingStage = 'parse' | 'resources' | 'mesh'
type MeshJob = {
    manifest: SchematicPreviewManifest
    epoch: number
    jobId: string
    regionId: string
    position: [number, number, number]
}
type WorkerSlot = {
    worker: Worker
    ready: boolean
    busy: boolean
}
type BlockEditHistoryEntry = {
    kind: 'blocks'
    label: string
    changes: Array<SchematicBlockEdit & { before: number }>
}
type TransformHistoryEntry = {
    kind: 'transform'
    label: string
    transform: SchematicTransform
}
type EditHistoryEntry = BlockEditHistoryEntry | TransformHistoryEntry

const { addNotification, handleError } = injectNotificationManager()
const { formatMessage, locale } = useVIntl()
const route = useRoute()
const canvas = useTemplateRef<HTMLCanvasElement>('canvas')
const workspace = useTemplateRef<HTMLElement>('workspace')
const viewport = useTemplateRef<HTMLElement>('viewport')
const instancePicker =
    useTemplateRef<InstanceType<typeof SchematicInstancePickerModal>>('instancePicker')
const blockPicker = useTemplateRef<InstanceType<typeof SchematicBlockPickerModal>>('blockPicker')
const infoModal = useTemplateRef<InstanceType<typeof SchematicInfoModal>>('infoModal')

const manifest = shallowRef<SchematicPreviewManifest>()
const currentSource = shallowRef<SchematicPreviewSource>()
const resources = shallowRef<LoadedSchematicResources>()
const instances = ref<GameInstance[]>([])
const recent = ref<RecentSchematic[]>(loadRecentSchematics())
const unavailableRecent = reactive(new Set<string>())
const regionVisibility = reactive<Record<string, boolean>>({})
const selected = ref<SchematicSceneSelection>()
const selectedBlock = ref<SchematicBlockState>()
const selectedBlocks = ref<SchematicBlockLocation[]>([])
const workspaceTool = ref<WorkspaceTool>('select')
const selectionAnchor = ref<SchematicBlockLocation>()
const measurementStart = ref<SchematicSceneSelection>()
const measurementEnd = ref<SchematicSceneSelection>()
const hiddenBlocks = reactive(new Set<string>())
const isolateSelection = ref(false)
const isolatedBlocks = reactive(new Set<string>())
const lastReplacementName = ref('')
const editHistory = ref<EditHistoryEntry[]>([])
const redoHistory = ref<EditHistoryEntry[]>([])
const applyingEdit = ref(false)
const inspectorTab = ref<InspectorTab>('edit')
const layerMaximum = ref(0)
const layerExplosion = ref(0)
const materialSearch = ref('')
const expandedMaterial = ref('')
const projection = ref<'perspective' | 'orthographic'>('perspective')
const viewMode = ref<ViewMode>('orbit')
const walkLocked = ref(false)
const walkSpeed = ref(8)
const useNativeWalkLock = isTauri() && platform() === 'macos'
const showGrid = ref(true)
const showBounds = ref(true)
const showTranslucent = ref(true)
const seamlessGlass = ref(true)
const inspectorOpen = ref(true)
const isFullscreen = ref(false)
const dragging = ref(false)
const contextLost = ref(false)
const loadingStage = ref<LoadingStage>()
const loadingProgress = ref(0)
const error = ref('')
const missingBlocks = reactive(new Set<string>())
const transientWarnings = ref<string[]>([])

let scene: SchematicPreviewScene | undefined
let workers: WorkerSlot[] = []
let meshQueue: MeshJob[] = []
let meshJobs = new Map<string, MeshJob>()
const chunkReadPromises = new Map<string, Promise<Uint32Array>>()
let requestEpoch = 0
let resourceEpoch = 0
let activeOpenRequestId: string | undefined
let disposed = false
let resourceController: AbortController | undefined
let completedMeshes = 0
let totalMeshes = 0
let unlistenNativeDrop: (() => void) | undefined
const chunkCache = new Map<string, SchematicCachedChunk>()
const materialTextureCache = new Map<string, [number, number, number, number] | undefined>()

function captureSession(opened: SchematicPreviewManifest) {
    return { epoch: requestEpoch, sessionId: opened.sessionId }
}

function isCurrentSession(session: ReturnType<typeof captureSession>) {
    return session.epoch === requestEpoch && session.sessionId === manifest.value?.sessionId
}

const messages = defineMessages({
    title: { id: 'app.lab.schematic-preview.title', defaultMessage: 'Schematic workshop' },
    emptyTitle: {
        id: 'app.lab.schematic-preview.empty.title',
        defaultMessage: 'Edit a Minecraft schematic',
    },
    emptyDescription: {
        id: 'app.lab.schematic-preview.empty.description',
        defaultMessage:
            'Open a Litematica or Sponge schematic to inspect and edit it locally in 3D.',
    },
    openFile: { id: 'app.lab.schematic-preview.open-file', defaultMessage: 'Open file' },
    open: { id: 'app.lab.schematic-preview.open', defaultMessage: 'Open' },
    fromInstance: {
        id: 'app.lab.schematic-preview.from-instance',
        defaultMessage: 'From instance',
    },
    recent: { id: 'app.lab.schematic-preview.recent', defaultMessage: 'Recent schematics' },
    clearRecent: { id: 'app.lab.schematic-preview.clear-recent', defaultMessage: 'Clear recent' },
    missingFile: {
        id: 'app.lab.schematic-preview.missing-file',
        defaultMessage: 'File does not exist',
    },
    removeRecent: {
        id: 'app.lab.schematic-preview.remove-recent',
        defaultMessage: 'Remove from recent',
    },
    parse: { id: 'app.lab.schematic-preview.loading.parse', defaultMessage: 'Parsing schematic' },
    resources: {
        id: 'app.lab.schematic-preview.loading.resources',
        defaultMessage: 'Reading resources',
    },
    mesh: { id: 'app.lab.schematic-preview.loading.mesh', defaultMessage: 'Generating models' },
    reload: { id: 'app.lab.schematic-preview.reload', defaultMessage: 'Reload' },
    screenshot: { id: 'app.lab.schematic-preview.screenshot', defaultMessage: 'Export PNG' },
    materialsCsv: {
        id: 'app.lab.schematic-preview.materials-csv',
        defaultMessage: 'Export material CSV',
    },
    resetView: { id: 'app.lab.schematic-preview.reset-view', defaultMessage: 'Reset view' },
    projection: {
        id: 'app.lab.schematic-preview.projection',
        defaultMessage: 'Switch camera projection',
    },
    walkView: { id: 'app.lab.schematic-preview.walk-view', defaultMessage: 'Walk view' },
    orbitView: { id: 'app.lab.schematic-preview.orbit-view', defaultMessage: 'Orbit view' },
    fullscreen: { id: 'app.lab.schematic-preview.fullscreen', defaultMessage: 'Toggle fullscreen' },
    exitFullscreen: {
        id: 'app.lab.schematic-preview.exit-fullscreen',
        defaultMessage: 'Exit fullscreen',
    },
    grid: { id: 'app.lab.schematic-preview.grid', defaultMessage: 'Show grid' },
    bounds: { id: 'app.lab.schematic-preview.bounds', defaultMessage: 'Show region boundaries' },
    translucent: {
        id: 'app.lab.schematic-preview.translucent',
        defaultMessage: 'Show transparent blocks',
    },
    seamlessGlass: {
        id: 'app.lab.schematic-preview.seamless-glass',
        defaultMessage: 'Seamless glass',
    },
    moreActions: {
        id: 'app.lab.schematic-preview.more-actions',
        defaultMessage: 'More actions',
    },
    schematicInfo: {
        id: 'app.lab.schematic-preview.info.title',
        defaultMessage: 'Schematic info',
    },
    edit: { id: 'app.lab.schematic-preview.tab.edit', defaultMessage: 'Edit' },
    materials: { id: 'app.lab.schematic-preview.tab.materials', defaultMessage: 'Materials' },
    coordinates: { id: 'app.lab.schematic-preview.coordinates', defaultMessage: 'Coordinates' },
    blocks: { id: 'app.lab.schematic-preview.blocks', defaultMessage: 'Blocks' },
    allLayers: { id: 'app.lab.schematic-preview.layers.all', defaultMessage: 'All' },
    visibleLayers: {
        id: 'app.lab.schematic-preview.layers.visible',
        defaultMessage: 'Y {current} · {visible}/{total} layers',
    },
    layerSpacing: {
        id: 'app.lab.schematic-preview.layers.spacing',
        defaultMessage: 'Layer spacing',
    },
    searchMaterials: {
        id: 'app.lab.schematic-preview.materials.search',
        defaultMessage: 'Search materials',
    },
    noMaterials: {
        id: 'app.lab.schematic-preview.materials.empty',
        defaultMessage: 'No matching materials',
    },
    warnings: { id: 'app.lab.schematic-preview.warnings', defaultMessage: 'Warnings' },
    missingModels: {
        id: 'app.lab.schematic-preview.missing-models',
        defaultMessage: '{count} block models use the missing-texture fallback.',
    },
    selectedBlock: {
        id: 'app.lab.schematic-preview.selected-block',
        defaultMessage: 'Selected block',
    },
    selectedCount: {
        id: 'app.lab.schematic-preview.selection.count',
        defaultMessage: '{count} selected',
    },
    blockState: {
        id: 'app.lab.schematic-preview.block-state',
        defaultMessage: 'Block state',
    },
    singleSelect: {
        id: 'app.lab.schematic-preview.selection.single',
        defaultMessage: 'Select',
    },
    boxSelect: { id: 'app.lab.schematic-preview.selection.box', defaultMessage: 'Area select' },
    measure: { id: 'app.lab.schematic-preview.measure', defaultMessage: 'Measure' },
    measurementStart: {
        id: 'app.lab.schematic-preview.measure.start',
        defaultMessage: 'Start',
    },
    measurementEnd: { id: 'app.lab.schematic-preview.measure.end', defaultMessage: 'End' },
    measurementDistance: {
        id: 'app.lab.schematic-preview.measure.distance',
        defaultMessage: 'Distance',
    },
    measurementSize: {
        id: 'app.lab.schematic-preview.measure.size',
        defaultMessage: 'Block span',
    },
    clearMeasurement: {
        id: 'app.lab.schematic-preview.measure.clear',
        defaultMessage: 'Clear measurement',
    },
    workspaceTools: {
        id: 'app.lab.schematic-preview.tools',
        defaultMessage: 'Workspace tools',
    },
    boxSelectPending: {
        id: 'app.lab.schematic-preview.selection.box-pending',
        defaultMessage: 'Choose the opposite corner',
    },
    selectMaterial: {
        id: 'app.lab.schematic-preview.selection.material',
        defaultMessage: 'Same material',
    },
    selectLayer: {
        id: 'app.lab.schematic-preview.selection.layer',
        defaultMessage: 'Same layer',
    },
    selectConnected: {
        id: 'app.lab.schematic-preview.selection.connected',
        defaultMessage: 'Connected',
    },
    clearSelection: {
        id: 'app.lab.schematic-preview.selection.clear',
        defaultMessage: 'Clear selection',
    },
    hideSelection: {
        id: 'app.lab.schematic-preview.visibility.hide',
        defaultMessage: 'Hide selected',
    },
    isolateSelection: {
        id: 'app.lab.schematic-preview.visibility.isolate',
        defaultMessage: 'Only selected',
    },
    showAll: {
        id: 'app.lab.schematic-preview.visibility.show-all',
        defaultMessage: 'Show all',
    },
    visibility: { id: 'app.lab.schematic-preview.visibility', defaultMessage: 'Visibility' },
    replaceSelected: {
        id: 'app.lab.schematic-preview.edit.replace',
        defaultMessage: 'Replace selected',
    },
    deleteSelected: {
        id: 'app.lab.schematic-preview.edit.delete',
        defaultMessage: 'Delete selected',
    },
    undo: { id: 'app.lab.schematic-preview.edit.undo', defaultMessage: 'Undo' },
    redo: { id: 'app.lab.schematic-preview.edit.redo', defaultMessage: 'Redo' },
    exportSchematic: {
        id: 'app.lab.schematic-preview.export-schematic',
        defaultMessage: 'Export schematic',
    },
    exportSponge: {
        id: 'app.lab.schematic-preview.export-sponge',
        defaultMessage: 'Sponge schematic (.schem)',
    },
    exportLitematic: {
        id: 'app.lab.schematic-preview.export-litematic',
        defaultMessage: 'Litematica schematic (.litematic)',
    },
    materialsJson: {
        id: 'app.lab.schematic-preview.materials-json',
        defaultMessage: 'Export material JSON',
    },
    useForReplace: {
        id: 'app.lab.schematic-preview.materials.use-replace',
        defaultMessage: 'Use for replace',
    },
    transform: { id: 'app.lab.schematic-preview.transform', defaultMessage: 'Transform' },
    rotateClockwise: {
        id: 'app.lab.schematic-preview.transform.rotate-clockwise',
        defaultMessage: 'Rotate clockwise',
    },
    rotateCounterClockwise: {
        id: 'app.lab.schematic-preview.transform.rotate-counter-clockwise',
        defaultMessage: 'Rotate counterclockwise',
    },
    mirrorX: { id: 'app.lab.schematic-preview.transform.mirror-x', defaultMessage: 'Mirror X' },
    mirrorZ: { id: 'app.lab.schematic-preview.transform.mirror-z', defaultMessage: 'Mirror Z' },
    selectAllMaterial: {
        id: 'app.lab.schematic-preview.materials.select-all',
        defaultMessage: 'Select all',
    },
    webglLost: {
        id: 'app.lab.schematic-preview.webgl-lost',
        defaultMessage: 'The WebGL context was lost. Rendering will resume when it is restored.',
    },
    drop: { id: 'app.lab.schematic-preview.drop', defaultMessage: 'Drop schematic to open' },
    exportComplete: {
        id: 'app.lab.schematic-preview.export-complete',
        defaultMessage: 'Export complete',
    },
    inspector: { id: 'app.lab.schematic-preview.inspector', defaultMessage: 'Inspector' },
    walkPreview: {
        id: 'app.lab.schematic-preview.walk-preview',
        defaultMessage: 'Read-only walk preview',
    },
    walkSpeed: {
        id: 'app.lab.schematic-preview.walk-speed',
        defaultMessage: 'Speed {speed} · Scroll to adjust',
    },
})

const inspectorTabs = computed(() => [
    { value: 'edit', label: formatMessage(messages.edit), icon: EditIcon },
    { value: 'materials', label: formatMessage(messages.materials), icon: ListIcon },
])
const openMenuOptions = computed<OverflowMenuOption[]>(() => [
    { id: 'open-file', icon: FileArchiveIcon, action: chooseLocalFile },
    {
        id: 'from-instance',
        icon: FolderSearchIcon,
        action: () => instancePicker.value?.show(),
        disabled: instances.value.length === 0,
    },
])
const exportMenuOptions = computed<OverflowMenuOption[]>(() => [
    { id: 'export-sponge', icon: FileArchiveIcon, action: () => exportSchematic('schem') },
    { id: 'export-litematic', icon: FileArchiveIcon, action: () => exportSchematic('litematic') },
])
const moreMenuOptions = computed<OverflowMenuOption[]>(() => [
    { id: 'schematic-info', icon: InfoIcon, action: () => infoModal.value?.show() },
    { divider: true },
    {
        id: 'seamless-glass',
        icon: GridIcon,
        action: () => (seamlessGlass.value = !seamlessGlass.value),
        remainOnClick: true,
    },
    { divider: true },
    { id: 'screenshot', icon: ImageIcon, action: exportPng },
    { id: 'materials-csv', icon: DownloadIcon, action: exportMaterials },
    { divider: true },
    { id: 'reload', icon: RefreshCwIcon, action: reloadCurrent },
])
const loadingLabel = computed(() =>
    loadingStage.value ? formatMessage(messages[loadingStage.value]) : '',
)
const layerFloor = computed(() => manifest.value?.min[1] ?? 0)
const layerCeiling = computed(() => manifest.value?.max[1] ?? 0)
const visibleLayerCount = computed(() => layerMaximum.value - layerFloor.value + 1)
const totalLayerCount = computed(() => layerCeiling.value - layerFloor.value + 1)
const showingAllLayers = computed(() => layerMaximum.value >= layerCeiling.value)
const measurement = computed(() => {
    if (!measurementStart.value || !measurementEnd.value) return undefined
    return measureSchematicPoints(measurementStart.value.position, measurementEnd.value.position)
})
const visibleMaterials = computed(() => {
    const query = materialSearch.value.trim().toLocaleLowerCase(locale.value)
    return (manifest.value?.materials ?? []).filter((material) => {
        if (isSchematicAir(material.name)) return false
        if (!query) return true
        return [material.name, blockDisplayName(material.name)].some((value) =>
            value.toLocaleLowerCase(locale.value).includes(query),
        )
    })
})
const replacementBlocks = computed(() => {
    const blocks = new Map<string, SchematicBlockState>()
    for (const state of resources.value?.availableBlockStates ?? []) {
        if (!isSchematicAir(state.name)) blocks.set(schematicBlockStateKey(state), state)
    }
    for (const state of manifest.value?.palette ?? []) {
        if (!isSchematicAir(state.name) && !blocks.has(schematicBlockStateKey(state))) {
            blocks.set(schematicBlockStateKey(state), state)
        }
    }
    return [...blocks.values()]
})
const canEditSelection = computed(
    () => selectedBlocks.value.length > 0 && !loadingStage.value && !applyingEdit.value,
)
const warnings = computed(() => {
    const items = [...(manifest.value?.warnings ?? []), ...transientWarnings.value]
    if (missingBlocks.size) {
        items.push(formatMessage(messages.missingModels, { count: missingBlocks.size }))
    }
    return [...new Set(items)]
})
const selectedStateProperties = computed(() => {
    const state = selectedBlock.value
    if (!state) return []
    return schematicStateProperties(
        state,
        resources.value?.workerResources.blockDefinitions[state.name],
    )
})
const selectedStateText = computed(() => {
    if (!selectedBlock.value) return ''
    const properties = Object.entries(selectedBlock.value.properties)
        .map(([key, value]) => `${key}=${value}`)
        .join(',')
    const name = blockDisplayName(selectedBlock.value.name)
    return properties ? `${name} [${properties}]` : name
})

function blockDisplayName(name: string) {
    return resolveSchematicBlockName(
        name,
        resources.value?.blockNames ?? { en_us: {}, zh_cn: {} },
        locale.value,
    )
}

function stageProgress(stage: LoadingStage) {
    if (loadingStage.value !== stage) return 0
    return stage === 'parse' ? 12 : stage === 'resources' ? 38 : 38 + loadingProgress.value * 62
}

async function loadInstances() {
    instances.value = (await list().catch(() => []))
        .filter((instance) => instance.install_stage === 'installed')
        .sort(
            (left, right) =>
                Number(new Date(right.last_played ?? 0)) - Number(new Date(left.last_played ?? 0)),
        )
}

async function chooseLocalFile() {
    const path = await open({
        multiple: false,
        filters: [{ name: 'Minecraft schematics', extensions: ['litematic', 'schem'] }],
    })
    if (!disposed && typeof path === 'string') await openSource({ kind: 'external', path })
}

async function openSource(source: SchematicPreviewSource) {
    if (disposed) return
    const epoch = ++requestEpoch
    resourceEpoch++
    resourceController?.abort()
    applyingEdit.value = false
    const requestId = `schematic-open-${epoch}`
    if (activeOpenRequestId) void cancelSchematicPreview(activeOpenRequestId)
    activeOpenRequestId = requestId
    terminateWorkers()
    loadingStage.value = 'parse'
    loadingProgress.value = 0
    error.value = ''
    transientWarnings.value = []
    missingBlocks.clear()
    selected.value = undefined
    selectedBlock.value = undefined
    selectedBlocks.value = []
    selectionAnchor.value = undefined
    measurementStart.value = undefined
    measurementEnd.value = undefined
    workspaceTool.value = 'select'
    hiddenBlocks.clear()
    isolatedBlocks.clear()
    isolateSelection.value = false
    editHistory.value = []
    redoHistory.value = []
    chunkCache.clear()
    dragging.value = false
    try {
        const opened = await openSchematicPreview(source, requestId)
        if (activeOpenRequestId === requestId) activeOpenRequestId = undefined
        if (epoch !== requestEpoch) {
            await closeSchematicPreview(opened.sessionId)
            return
        }
        const previousSession = manifest.value?.sessionId
        manifest.value = opened
        lastReplacementName.value = ''
        currentSource.value = source
        if (previousSession) void closeSchematicPreview(previousSession)
        Object.keys(regionVisibility).forEach((key) =>
            Reflect.deleteProperty(regionVisibility, key),
        )
        for (const region of opened.regions) regionVisibility[region.id] = true
        layerMaximum.value = opened.max[1]
        layerExplosion.value = 0
        await nextTick()
        if (epoch !== requestEpoch) return
        ensureScene()
        scene?.setRegions(opened.regions)
        scene?.fitView()
        recent.value = recordRecentSchematic(source, opened.fileName)
        unavailableRecent.delete(recent.value[0]?.id ?? '')
        await applyResources(epoch)
    } catch (caught) {
        if (activeOpenRequestId === requestId) activeOpenRequestId = undefined
        if (epoch !== requestEpoch) return
        error.value = caught instanceof Error ? caught.message : String(caught)
        loadingStage.value = undefined
        const record = recent.value.find(
            (item) => JSON.stringify(item.source) === JSON.stringify(source),
        )
        if (record && /does not exist|no such file/i.test(error.value))
            unavailableRecent.add(record.id)
    }
}

function reloadCurrent() {
    if (!currentSource.value) return
    void openSource(currentSource.value)
}

async function applyResources(epoch = requestEpoch) {
    const opened = manifest.value
    if (!opened || epoch !== requestEpoch) return
    const session = captureSession(opened)
    const resourceRequest = ++resourceEpoch
    resourceController?.abort()
    const controller = new AbortController()
    resourceController = controller
    loadingStage.value = 'resources'
    loadingProgress.value = 0
    const builtInVersion = minecraftVersionFromDataVersion(opened.dataVersion) ?? 'latest'
    const loaded = await createSchematicResources(builtInVersion, opened.palette, controller.signal)
    if (!isCurrentSession(session) || resourceRequest !== resourceEpoch) {
        loaded.texture.dispose()
        return
    }
    resources.value?.texture.dispose()
    resources.value = loaded
    materialTextureCache.clear()
    scene?.setTexture(loaded.texture)
    startMeshWorkers(opened, loaded, resourceRequest)
}

function startMeshWorkers(
    opened: SchematicPreviewManifest,
    loaded: LoadedSchematicResources,
    meshEpoch: number,
) {
    terminateWorkers()
    scene?.clearChunks()
    chunkCache.clear()
    meshQueue = opened.regions
        .flatMap((region) =>
            region.chunks.map((chunk) => ({
                manifest: opened,
                epoch: meshEpoch,
                jobId: `${region.id}:${chunk.position.join(':')}`,
                regionId: region.id,
                position: chunk.position,
            })),
        )
        .sort((left, right) => {
            const leftDistance = left.position.reduce((sum, value) => sum + value * value, 0)
            const rightDistance = right.position.reduce((sum, value) => sum + value * value, 0)
            return leftDistance - rightDistance
        })
    meshJobs = new Map(meshQueue.map((job) => [schematicChunkKey(job.regionId, job.position), job]))
    totalMeshes = meshQueue.length
    completedMeshes = 0
    loadingStage.value = totalMeshes ? 'mesh' : undefined
    loadingProgress.value = totalMeshes ? 0 : 1
    const workerCount = Math.max(1, Math.min(4, (navigator.hardwareConcurrency || 4) - 2))
    for (let index = 0; index < workerCount; index += 1) {
        const worker = new Worker(
            new URL('../lab/schematic-preview/mesh-worker.ts', import.meta.url),
            {
                type: 'module',
            },
        )
        const slot: WorkerSlot = { worker, ready: false, busy: false }
        worker.onmessage = (event: MessageEvent<SchematicMeshWorkerResponse>) =>
            handleWorkerMessage(slot, event.data)
        worker.onerror = (event) => {
            if (meshEpoch !== resourceEpoch) return
            transientWarnings.value = [...transientWarnings.value, event.message]
            slot.busy = false
            completeMeshJob()
            pumpMeshQueue()
        }
        workers.push(slot)
        worker.postMessage({
            type: 'init',
            epoch: meshEpoch,
            palette: opened.palette,
            resources: loaded.workerResources,
            seamlessGlass: seamlessGlass.value,
        } satisfies SchematicMeshWorkerRequest)
    }
}

function handleWorkerMessage(slot: WorkerSlot, message: SchematicMeshWorkerResponse) {
    if (message.epoch !== resourceEpoch) return
    if (message.type === 'ready') {
        slot.ready = true
        if (message.warnings.length) {
            transientWarnings.value = [
                ...new Set([...transientWarnings.value, ...message.warnings]),
            ]
        }
        pumpMeshQueue()
        return
    }
    if (message.type === 'mesh') {
        const chunk = chunkCache.get(schematicChunkKey(message.regionId, message.chunkPosition))
        const palette = manifest.value?.palette
        scene?.setChunk(
            message.regionId,
            message.chunkPosition,
            chunk && palette
                ? filterSchematicAirGeometry(message.opaque, chunk, palette)
                : message.opaque,
            chunk && palette
                ? filterSchematicAirGeometry(message.translucent, chunk, palette)
                : message.translucent,
        )
        for (const name of message.missing) missingBlocks.add(name)
    } else {
        transientWarnings.value = [...transientWarnings.value, message.message]
    }
    slot.busy = false
    completeMeshJob()
    pumpMeshQueue()
}

function completeMeshJob() {
    completedMeshes += 1
    loadingProgress.value = totalMeshes ? completedMeshes / totalMeshes : 1
    if (completedMeshes >= totalMeshes) {
        loadingStage.value = undefined
    }
}

function pumpMeshQueue() {
    if (contextLost.value) return
    for (const slot of workers) {
        if (!slot.ready || slot.busy) continue
        const job = meshQueue.shift()
        if (!job) continue
        slot.busy = true
        void prepareMeshJob(job)
            .then(({ blocks, neighborFaces }) => {
                if (job.epoch !== resourceEpoch) return
                const renderBlocks = filterChunkVisibility(job, blocks).slice()
                const serializedNeighborFaces: Partial<Record<SchematicDirection, ArrayBuffer>> = {}
                const transferables: Transferable[] = [renderBlocks.buffer]
                for (const direction of SCHEMATIC_DIRECTIONS) {
                    const face = neighborFaces[direction]
                    if (!face) continue
                    serializedNeighborFaces[direction] = face.buffer as ArrayBuffer
                    transferables.push(face.buffer as ArrayBuffer)
                }
                slot.worker.postMessage(
                    {
                        type: 'mesh',
                        epoch: job.epoch,
                        jobId: job.jobId,
                        regionId: job.regionId,
                        chunkPosition: job.position,
                        blocks: renderBlocks.buffer,
                        neighborFaces: serializedNeighborFaces,
                    } satisfies SchematicMeshWorkerRequest,
                    transferables,
                )
            })
            .catch((caught) => {
                if (job.epoch !== resourceEpoch) return
                transientWarnings.value = [
                    ...transientWarnings.value,
                    caught instanceof Error ? caught.message : String(caught),
                ]
                slot.busy = false
                completeMeshJob()
                pumpMeshQueue()
            })
    }
}

function readMeshJobChunk(job: MeshJob) {
    const key = schematicChunkKey(job.regionId, job.position)
    const pending = chunkReadPromises.get(key)
    if (pending) return pending
    const request = readSchematicChunk(job.manifest.sessionId, job.regionId, job.position).then(
        (blocks) => {
            const normalized = normalizeSchematicAirBlocks(blocks, job.manifest.palette)
            if (job.epoch === resourceEpoch) {
                chunkCache.set(key, {
                    regionId: job.regionId,
                    position: job.position,
                    blocks: normalized.slice(),
                })
            }
            return normalized
        },
    )
    chunkReadPromises.set(key, request)
    return request
}

async function prepareMeshJob(job: MeshJob) {
    const blocks = await readMeshJobChunk(job)
    const neighborFaces: Partial<Record<SchematicDirection, Uint32Array>> = {}
    if (job.epoch !== resourceEpoch) return { blocks, neighborFaces }
    await Promise.all(
        SCHEMATIC_DIRECTIONS.map(async (direction) => {
            const position = schematicNeighborChunkPosition(job.position, direction)
            const neighborJob = meshJobs.get(schematicChunkKey(job.regionId, position))
            if (!neighborJob) return
            const neighborBlocks = await readMeshJobChunk(neighborJob)
            if (job.epoch !== resourceEpoch) return
            neighborFaces[direction] = extractSchematicNeighborFace(
                filterChunkVisibility(neighborJob, neighborBlocks),
                direction,
            )
        }),
    )
    return { blocks, neighborFaces }
}

function filterChunkVisibility(job: MeshJob, blocks: Uint32Array) {
    if (hiddenBlocks.size === 0 && !isolateSelection.value) return blocks
    const filtered = blocks.slice()
    for (let index = 0; index < filtered.length; index += 1) {
        if (filtered[index] === 0) continue
        const location: SchematicBlockLocation = {
            regionId: job.regionId,
            position: [
                job.position[0] * 16 + (index % 16),
                job.position[1] * 16 + Math.floor(index / 256),
                job.position[2] * 16 + (Math.floor(index / 16) % 16),
            ],
        }
        const key = schematicBlockKey(location)
        if (hiddenBlocks.has(key) || (isolateSelection.value && !isolatedBlocks.has(key))) {
            filtered[index] = 0
        }
    }
    return filtered
}

function terminateWorkers() {
    for (const slot of workers) slot.worker.terminate()
    workers = []
    meshQueue = []
    meshJobs.clear()
    chunkReadPromises.clear()
}

function ensureScene() {
    if (scene || !canvas.value) return
    try {
        scene = new SchematicPreviewScene({
            canvas: canvas.value,
            onSelect: selectBlock,
            onFocus: (selection) => {
                if (workspaceTool.value === 'select' || workspaceTool.value === 'box') {
                    scene?.focusSelection(selection)
                }
            },
            onContextLost: () => {
                contextLost.value = true
            },
            onContextRestored: () => {
                contextLost.value = false
                pumpMeshQueue()
            },
            onViewModeChange: (mode) => {
                viewMode.value = mode
                if (mode === 'orbit') walkLocked.value = false
            },
            onWalkLockChange: (locked) => {
                walkLocked.value = locked
            },
            onWalkSpeedChange: (speed) => {
                walkSpeed.value = speed
            },
            ...(useNativeWalkLock
                ? {
                      onNativeWalkLock: async () => {
                          const appWindow = getCurrentWindow()
                          try {
                              await appWindow.setCursorGrab(true)
                              await appWindow.setCursorVisible(false)
                              return true
                          } catch {
                              await appWindow.setCursorGrab(false).catch(() => undefined)
                              return false
                          }
                      },
                      onNativeWalkUnlock: async () => {
                          const appWindow = getCurrentWindow()
                          await appWindow.setCursorVisible(true)
                          await appWindow.setCursorGrab(false)
                      },
                  }
                : {}),
        })
    } catch (caught) {
        error.value = caught instanceof Error ? caught.message : String(caught)
    }
}

function setSelectedBlocks(locations: SchematicBlockLocation[]) {
    selectedBlocks.value = locations
    const first = locations[0]
    selected.value = first
    const paletteIndex = first ? schematicBlockPaletteIndex(chunkCache, first) : 0
    selectedBlock.value = manifest.value?.palette[paletteIndex]
    const bounds = schematicSelectionBounds(locations)
    scene?.setSelectionBlocks(locations.slice(0, 4000), bounds)
}

function clearBlockSelection() {
    selectionAnchor.value = undefined
    setSelectedBlocks([])
}

function clearMeasurement() {
    measurementStart.value = undefined
    measurementEnd.value = undefined
    scene?.setMeasurement()
}

function selectBlock(selection?: SchematicSceneSelection) {
    if (workspaceTool.value === 'layer-spacing') return
    if (workspaceTool.value === 'measure') {
        if (!selection) return
        if (!measurementStart.value || measurementEnd.value) {
            measurementStart.value = selection
            measurementEnd.value = undefined
        } else {
            measurementEnd.value = selection
        }
        scene?.setMeasurement(measurementStart.value, measurementEnd.value)
        return
    }
    if (!selection) {
        clearBlockSelection()
        return
    }
    if (workspaceTool.value === 'box') {
        if (!selectionAnchor.value || selectionAnchor.value.regionId !== selection.regionId) {
            selectionAnchor.value = selection
            setSelectedBlocks([selection])
            return
        }
        setSelectedBlocks(selectSchematicCuboid(chunkCache, selectionAnchor.value, selection))
        selectionAnchor.value = undefined
        return
    }
    selectionAnchor.value = undefined
    setSelectedBlocks([selection])
}

function expandSelectionByMaterial(name?: string) {
    if (!manifest.value) return
    if (!name && selectedBlock.value) {
        setSelectedBlocks(
            selectSchematicBlockState(chunkCache, manifest.value.palette, selectedBlock.value),
        )
        return
    }
    if (!name) return
    setSelectedBlocks(selectSchematicMaterial(chunkCache, manifest.value.palette, name))
}

function expandSelectionByLayer() {
    const first = selectedBlocks.value[0]
    if (first) setSelectedBlocks(selectSchematicLayer(chunkCache, first.position[1]))
}

function expandConnectedSelection() {
    const first = selectedBlocks.value[0]
    if (first) setSelectedBlocks(selectConnectedSchematicBlocks(chunkCache, first))
}

function rebuildMeshes() {
    if (loadingStage.value === 'parse' || loadingStage.value === 'resources') return
    if (manifest.value && resources.value) {
        startMeshWorkers(manifest.value, resources.value, ++resourceEpoch)
    }
}

function hideSelectedBlocks() {
    if (!selectedBlocks.value.length) return
    for (const location of selectedBlocks.value) hiddenBlocks.add(schematicBlockKey(location))
    isolateSelection.value = false
    isolatedBlocks.clear()
    clearBlockSelection()
    rebuildMeshes()
}

function showOnlySelectedBlocks() {
    if (!selectedBlocks.value.length) return
    isolatedBlocks.clear()
    for (const location of selectedBlocks.value) isolatedBlocks.add(schematicBlockKey(location))
    isolateSelection.value = true
    rebuildMeshes()
}

function showAllBlocks() {
    hiddenBlocks.clear()
    isolatedBlocks.clear()
    isolateSelection.value = false
    rebuildMeshes()
}

type BlockEditRequest = {
    locations: SchematicBlockLocation[]
    label: string
    targetState?: SchematicBlockState
    paletteIndex?: number
    overrideSourceProperties?: boolean
}

function inspectorShowsEditedBlock(locations: SchematicBlockLocation[]) {
    const current = selected.value
    if (!current) return false
    const key = schematicBlockKey(current)
    return locations.some((location) => schematicBlockKey(location) === key)
}

async function commitBlockEdits({
    locations,
    label,
    targetState,
    paletteIndex = -1,
    overrideSourceProperties = false,
}: BlockEditRequest) {
    if (!manifest.value || !canEditSelection.value || (!targetState && paletteIndex < 0)) return
    const session = captureSession(manifest.value)
    const targetKey = targetState ? schematicBlockStateKey(targetState) : undefined
    const existingTargetIndex = targetKey
        ? manifest.value.palette.findIndex((state) => schematicBlockStateKey(state) === targetKey)
        : paletteIndex
    const requestedPaletteIndex =
        existingTargetIndex >= 0 ? existingTargetIndex : Math.max(0, paletteIndex)
    const changes = locations
        .map((location) => ({
            ...location,
            paletteIndex: requestedPaletteIndex,
            before: schematicBlockPaletteIndex(chunkCache, location),
        }))
        .filter((change) => {
            if (!targetKey) return change.before !== requestedPaletteIndex
            const before = manifest.value?.palette[change.before]
            return !before || schematicBlockStateKey(before) !== targetKey
        })
    if (!changes.length) return
    applyingEdit.value = true
    try {
        const needsResourceReload = Boolean(
            targetState && !resources.value?.workerResources.blockDefinitions[targetState.name],
        )
        const result = await applySchematicEdits(
            session.sessionId,
            changes,
            targetState,
            overrideSourceProperties,
        )
        if (!isCurrentSession(session)) return
        manifest.value = result.manifest
        const appliedChanges = changes.map((change, index) => ({
            ...change,
            paletteIndex: result.appliedPaletteIndices[index] ?? requestedPaletteIndex,
        }))
        editHistory.value = [
            ...editHistory.value.slice(-49),
            { kind: 'blocks', label, changes: appliedChanges },
        ]
        redoHistory.value = []
        const firstAppliedPaletteIndex = appliedChanges[0]?.paletteIndex ?? 0
        if (inspectorShowsEditedBlock(locations)) {
            if (firstAppliedPaletteIndex === 0) clearBlockSelection()
            else selectedBlock.value = result.manifest.palette[firstAppliedPaletteIndex]
        }
        if (needsResourceReload) await applyResources(session.epoch)
        else rebuildMeshes()
    } catch (caught) {
        if (isCurrentSession(session)) handleError(caught)
    } finally {
        if (isCurrentSession(session)) applyingEdit.value = false
    }
}

function commitSelectionEdit(
    paletteIndex: number,
    label: string,
    targetState?: SchematicBlockState,
) {
    return commitBlockEdits({ locations: selectedBlocks.value, paletteIndex, label, targetState })
}

function commitStateProperty(key: string, value: string) {
    const state = selectedBlock.value
    const location = selected.value
    if (!state || !location || state.properties[key] === value) return
    return commitBlockEdits({
        locations: [location],
        label: formatMessage(messages.blockState),
        targetState: {
            name: state.name,
            properties: { ...state.properties, [key]: value },
        },
        overrideSourceProperties: true,
    })
}

async function applyHistory(entry: EditHistoryEntry, undo: boolean) {
    if (!manifest.value || loadingStage.value || applyingEdit.value) return false
    const session = captureSession(manifest.value)
    let applied = false
    applyingEdit.value = true
    try {
        if (entry.kind === 'blocks') {
            const edits = entry.changes.map((change) => ({
                regionId: change.regionId,
                position: change.position,
                paletteIndex: undo ? change.before : change.paletteIndex,
            }))
            const result = await applySchematicEdits(session.sessionId, edits)
            if (!isCurrentSession(session)) return false
            manifest.value = result.manifest
        } else {
            const transform = undo ? inverseTransform(entry.transform) : entry.transform
            const transformed = await transformSchematic(session.sessionId, transform)
            if (!isCurrentSession(session)) return false
            manifest.value = transformed
            resetSceneAfterTransform()
        }
        if (undo) redoHistory.value = [...redoHistory.value, entry]
        else editHistory.value = [...editHistory.value, entry]
        clearBlockSelection()
        rebuildMeshes()
        applied = true
    } catch (caught) {
        if (isCurrentSession(session)) handleError(caught)
    } finally {
        if (isCurrentSession(session)) applyingEdit.value = false
    }
    return applied
}

function inverseTransform(transform: SchematicTransform): SchematicTransform {
    if (transform === 'rotate_clockwise') return 'rotate_counter_clockwise'
    if (transform === 'rotate_counter_clockwise') return 'rotate_clockwise'
    return transform
}

function resetSceneAfterTransform() {
    if (!manifest.value) return
    hiddenBlocks.clear()
    isolatedBlocks.clear()
    isolateSelection.value = false
    layerMaximum.value = manifest.value.max[1]
    scene?.setRegions(manifest.value.regions)
    scene?.fitView()
}

async function transformStructure(transform: SchematicTransform, label: string) {
    if (!manifest.value || loadingStage.value || applyingEdit.value) return
    const session = captureSession(manifest.value)
    applyingEdit.value = true
    try {
        const transformed = await transformSchematic(session.sessionId, transform)
        if (!isCurrentSession(session)) return
        manifest.value = transformed
        editHistory.value = [
            ...editHistory.value.slice(-49),
            { kind: 'transform', label, transform },
        ]
        redoHistory.value = []
        clearBlockSelection()
        resetSceneAfterTransform()
        rebuildMeshes()
    } catch (caught) {
        if (isCurrentSession(session)) handleError(caught)
    } finally {
        if (isCurrentSession(session)) applyingEdit.value = false
    }
}

async function undoEdit() {
    if (!manifest.value || loadingStage.value || applyingEdit.value) return
    const session = captureSession(manifest.value)
    const entry = editHistory.value.at(-1)
    if (!entry) return
    editHistory.value = editHistory.value.slice(0, -1)
    if (!(await applyHistory(entry, true)) && isCurrentSession(session))
        editHistory.value = [...editHistory.value, entry]
}

async function redoEdit() {
    if (!manifest.value || loadingStage.value || applyingEdit.value) return
    const session = captureSession(manifest.value)
    const entry = redoHistory.value.at(-1)
    if (!entry) return
    redoHistory.value = redoHistory.value.slice(0, -1)
    if (!(await applyHistory(entry, false)) && isCurrentSession(session))
        redoHistory.value = [...redoHistory.value, entry]
}

function setRegionVisibility(regionId: string, visible: boolean) {
    scene?.setRegionVisible(regionId, visible)
    if (!visible && selected.value?.regionId === regionId) {
        selected.value = undefined
        selectedBlock.value = undefined
        scene?.setSelection()
    }
}

function toggleProjection() {
    projection.value = projection.value === 'perspective' ? 'orthographic' : 'perspective'
    scene?.setProjection(projection.value)
}

function applyLayerRange() {
    layerMaximum.value = Math.max(
        layerFloor.value,
        Math.min(layerMaximum.value, layerCeiling.value),
    )
    scene?.setLayerRange(
        layerMaximum.value >= layerCeiling.value
            ? undefined
            : [layerFloor.value, layerMaximum.value],
    )
}

function showAllLayers() {
    layerMaximum.value = layerCeiling.value
}

function applyLayerExplosion(value: number) {
    if (value > 0) {
        showAllLayers()
        clearBlockSelection()
    }
    scene?.setExplosion(value / 100)
}

function materialColor(name: string) {
    let hash = 0
    for (const character of name) hash = (hash * 31 + character.charCodeAt(0)) | 0
    return `hsl(${Math.abs(hash) % 360} 42% 48%)`
}

function materialTextureUv(name: string) {
    if (materialTextureCache.has(name)) return materialTextureCache.get(name)
    const uv = resources.value
        ? resolveSchematicMaterialTexture(name, resources.value.workerResources)
        : undefined
    materialTextureCache.set(name, uv)
    return uv
}

function materialStates(name: string) {
    return manifest.value?.palette.filter((state) => state.name === name) ?? []
}

function useMaterialForReplacement(name: string) {
    lastReplacementName.value = name
    inspectorTab.value = 'edit'
    blockPicker.value?.show(name)
}

function openBlockPicker() {
    blockPicker.value?.show(lastReplacementName.value || undefined)
}

function replaceSelectedWith(state: SchematicBlockState) {
    lastReplacementName.value = state.name
    void commitSelectionEdit(-1, formatMessage(messages.replaceSelected), state)
}

async function exportMaterials() {
    if (!manifest.value) return
    const opened = manifest.value
    const session = captureSession(opened)
    try {
        const path = await save({
            defaultPath: `${opened.fileName.replace(/\.(litematic|schem)$/i, '')}-materials.csv`,
            filters: [{ name: 'CSV', extensions: ['csv'] }],
        })
        if (!path || !isCurrentSession(session)) return
        const rows = [
            ['block', 'count'],
            ...opened.materials.map((material) => [material.name, material.count]),
        ]
        await writeTextFile(
            path,
            rows.map((row) => row.map(escapeSchematicCsvCell).join(',')).join('\r\n'),
        )
        if (isCurrentSession(session))
            addNotification({ type: 'success', title: formatMessage(messages.exportComplete) })
    } catch (caught) {
        if (isCurrentSession(session)) handleError(caught)
    }
}

async function exportMaterialsJson() {
    if (!manifest.value) return
    const opened = manifest.value
    const session = captureSession(opened)
    try {
        const path = await save({
            defaultPath: `${opened.fileName.replace(/\.(litematic|schem)$/i, '')}-materials.json`,
            filters: [{ name: 'JSON', extensions: ['json'] }],
        })
        if (!path || !isCurrentSession(session)) return
        await writeTextFile(path, JSON.stringify(opened.materials, null, 2))
        if (isCurrentSession(session))
            addNotification({ type: 'success', title: formatMessage(messages.exportComplete) })
    } catch (caught) {
        if (isCurrentSession(session)) handleError(caught)
    }
}

async function exportSchematic(format: 'schem' | 'litematic') {
    if (!manifest.value) return
    const opened = manifest.value
    const session = captureSession(opened)
    try {
        const extension = format === 'schem' ? 'schem' : 'litematic'
        const path = await save({
            defaultPath: `${opened.fileName.replace(/\.(litematic|schem)$/i, '')}-edited.${extension}`,
            filters: [
                format === 'schem'
                    ? { name: 'Sponge schematic', extensions: ['schem'] }
                    : { name: 'Litematica schematic', extensions: ['litematic'] },
            ],
        })
        if (!path || !isCurrentSession(session)) return
        const bytes =
            format === 'schem'
                ? await exportSchematicSponge(session.sessionId)
                : await exportSchematicLitematic(session.sessionId)
        if (!isCurrentSession(session)) return
        await writeFile(path, new Uint8Array(bytes))
        if (isCurrentSession(session))
            addNotification({ type: 'success', title: formatMessage(messages.exportComplete) })
    } catch (caught) {
        if (isCurrentSession(session)) handleError(caught)
    }
}

async function exportPng() {
    if (!manifest.value || !scene) return
    const opened = manifest.value
    const session = captureSession(opened)
    try {
        const path = await save({
            defaultPath: `${opened.fileName.replace(/\.(litematic|schem)$/i, '')}-preview.png`,
            filters: [{ name: 'PNG', extensions: ['png'] }],
        })
        if (!path || !isCurrentSession(session)) return
        const bytes = new Uint8Array(await (await fetch(scene.toPngDataUrl())).arrayBuffer())
        if (!isCurrentSession(session)) return
        await writeFile(path, bytes)
        if (isCurrentSession(session))
            addNotification({ type: 'success', title: formatMessage(messages.exportComplete) })
    } catch (caught) {
        if (isCurrentSession(session)) handleError(caught)
    }
}

function toggleViewMode() {
    if (viewMode.value === 'orbit') {
        clearBlockSelection()
        workspaceTool.value = 'select'
        projection.value = 'perspective'
        scene?.setViewMode('walk')
    } else {
        scene?.setViewMode('orbit')
    }
}

async function toggleFullscreen() {
    try {
        if (document.fullscreenElement) await document.exitFullscreen()
        else await viewport.value?.requestFullscreen()
    } catch (error) {
        handleError(error)
    }
}

function onFullscreenChange() {
    isFullscreen.value = document.fullscreenElement === viewport.value
}

async function copySelectedCoordinates() {
    const first = selectedBlocks.value[0]
    if (!first) return
    await navigator.clipboard.writeText(first.position.join(' '))
}

function openRecent(record: RecentSchematic) {
    void openSource(record.source)
}

function formatSource(record: RecentSchematic) {
    if (record.source.kind === 'external') return record.source.path
    const instance = instances.value.find((item) => item.id === record.source.instanceId)
    return `${instance?.name ?? record.source.instanceId} · schematics/${record.source.relativePath}`
}

function formatNumber(value: number | bigint) {
    return new Intl.NumberFormat(locale.value).format(value)
}

function formatDecimal(value: number) {
    return new Intl.NumberFormat(locale.value, { maximumFractionDigits: 2 }).format(value)
}

function formatFormat(opened: SchematicPreviewManifest) {
    return opened.format === 'litematic'
        ? `Litematic v${opened.formatVersion}`
        : `Sponge v${opened.formatVersion}`
}

function pointInWorkspace(position: { x: number; y: number }) {
    const rect = workspace.value?.getBoundingClientRect()
    if (!rect) return false
    const scale = window.devicePixelRatio || 1
    const x = position.x / scale
    const y = position.y / scale
    return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

async function setupNativeDrop() {
    const unlisten = await getCurrentWebview().onDragDropEvent(
        (event: { payload: DragDropEvent }) => {
            const payload = event.payload
            if (payload.type === 'leave') {
                dragging.value = false
                return
            }
            const path = payload.paths?.find((item) => /\.(litematic|schem)$/i.test(item))
            const inside = pointInWorkspace(payload.position)
            if (payload.type === 'enter' || payload.type === 'over')
                dragging.value = Boolean(path && inside)
            if (payload.type === 'drop') {
                dragging.value = false
                if (path && inside) void openSource({ kind: 'external', path })
            }
        },
    )
    if (disposed) unlisten()
    else unlistenNativeDrop = unlisten
}

function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'F11') {
        event.preventDefault()
        void toggleFullscreen()
        return
    }
    const target = event.target as HTMLElement | null
    if (target?.matches('input, textarea, select') || target?.isContentEditable) {
        return
    }
    const modifier = event.metaKey || event.ctrlKey
    if (modifier && event.key.toLocaleLowerCase() === 'z') {
        event.preventDefault()
        if (event.shiftKey) void redoEdit()
        else void undoEdit()
        return
    }
    if (modifier && event.key.toLocaleLowerCase() === 'y') {
        event.preventDefault()
        void redoEdit()
        return
    }
    if (event.key === 'Delete' || event.key === 'Backspace') {
        event.preventDefault()
        void commitSelectionEdit(0, formatMessage(messages.deleteSelected))
        return
    }
    if (
        event.key.toLocaleLowerCase() === 'v' &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.altKey
    ) {
        event.preventDefault()
        toggleViewMode()
        return
    }
    if (event.key === 'Escape' && measurementStart.value) {
        clearMeasurement()
        return
    }
    if (event.key === 'Escape' && selectedBlocks.value.length) clearBlockSelection()
}

watch(layerMaximum, applyLayerRange)
watch(layerExplosion, applyLayerExplosion)
watch(workspaceTool, (tool) => {
    selectionAnchor.value = undefined
    if (tool === 'measure' || tool === 'layer-spacing') clearBlockSelection()
    if (tool !== 'measure') clearMeasurement()
})
watch(showGrid, (value) => scene?.setGridVisible(value))
watch(showBounds, (value) => scene?.setBoundsVisible(value))
watch(showTranslucent, (value) => scene?.setTranslucentVisible(value))
watch(seamlessGlass, rebuildMeshes)

onMounted(async () => {
    await loadInstances()
    if (disposed) return
    window.addEventListener('keydown', handleKeydown)
    document.addEventListener('fullscreenchange', onFullscreenChange)
    await setupNativeDrop().catch(() => undefined)
    if (disposed) return

    const instanceId = typeof route.query.instance === 'string' ? route.query.instance : ''
    const relativePath = typeof route.query.path === 'string' ? route.query.path : ''
    if (
        instanceId &&
        relativePath &&
        /\.(litematic|schem)$/i.test(relativePath) &&
        instances.value.some((instance) => instance.id === instanceId)
    ) {
        await openSource({ kind: 'instance_file', instanceId, relativePath })
    }
})

onBeforeUnmount(() => {
    disposed = true
    resourceController?.abort()
    requestEpoch += 1
    resourceEpoch++
    if (activeOpenRequestId) void cancelSchematicPreview(activeOpenRequestId)
    terminateWorkers()
    window.removeEventListener('keydown', handleKeydown)
    document.removeEventListener('fullscreenchange', onFullscreenChange)
    unlistenNativeDrop?.()
    scene?.dispose()
    resources.value?.texture.dispose()
    if (manifest.value) void closeSchematicPreview(manifest.value.sessionId)
})
</script>

<template>
    <main ref="workspace" class="schematic-page" data-onboarding-id="schematic-preview-workspace">
        <div
            v-if="loadingStage"
            class="absolute inset-x-0 top-0 z-40 h-1 overflow-hidden bg-surface-5"
            role="progressbar"
            :aria-label="loadingLabel"
            :aria-valuenow="Math.round(stageProgress(loadingStage))"
        >
            <div
                class="h-full bg-brand transition-[width] duration-200"
                :style="{ width: `${stageProgress(loadingStage)}%` }"
            ></div>
        </div>

        <template v-if="!manifest">
            <section class="schematic-empty">
                <EmptyState
                    type="no-documents"
                    :heading="formatMessage(messages.emptyTitle)"
                    :description="formatMessage(messages.emptyDescription)"
                >
                    <template #actions>
                        <Button type="colored" color="brand" @click="chooseLocalFile"
                            ><FileArchiveIcon />{{ formatMessage(messages.openFile) }}
                        </Button>
                        <Button type="outlined" @click="instancePicker?.show()"
                            ><FolderSearchIcon />{{ formatMessage(messages.fromInstance) }}
                        </Button>
                    </template>
                </EmptyState>
                <p v-if="error" class="m-0 max-w-2xl text-center text-sm text-brand-red">
                    {{ error }}
                </p>
            </section>

            <section
                v-if="recent.length"
                class="schematic-recent w-[min(52rem,calc(100%-3rem))] mx-auto mb-8"
            >
                <header class="flex items-center justify-between gap-3">
                    <h2 class="m-0 text-base text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.recent) }}
                    </h2>
                    <Button type="quiet" size="2xs" @click="recent = clearRecentSchematics()"
                        ><TrashIcon />{{ formatMessage(messages.clearRecent) }}
                    </Button>
                </header>
                <ul class="schematic-recent-list m-0 list-none p-0">
                    <li
                        v-for="record in recent"
                        :key="record.id"
                        class="schematic-recent-row flex min-w-0 items-center gap-3 py-2"
                    >
                        <FileArchiveIcon
                            class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                        />
                        <button
                            class="min-w-0 flex-1 cursor-pointer border-0 bg-transparent p-0 text-left"
                            @click="openRecent(record)"
                        >
                            <strong class="block truncate text-[var(--color-text-default)]">{{
                                record.fileName
                            }}</strong>
                            <span
                                class="block truncate text-xs"
                                :class="
                                    unavailableRecent.has(record.id)
                                        ? 'text-brand-red'
                                        : 'text-[var(--color-text-tertiary)]'
                                "
                            >
                                {{
                                    unavailableRecent.has(record.id)
                                        ? formatMessage(messages.missingFile)
                                        : formatSource(record)
                                }}
                            </span>
                        </button>
                        <Button
                            type="quiet"
                            size="2xs"
                            circular
                            icon-only
                            :aria-label="formatMessage(messages.removeRecent)"
                            :title="formatMessage(messages.removeRecent)"
                            @click="recent = removeRecentSchematic(record.id)"
                            ><XIcon />
                        </Button>
                    </li>
                </ul>
            </section>
        </template>

        <template v-else>
            <header class="schematic-toolbar">
                <div class="min-w-0 flex-1">
                    <div class="flex min-w-0 items-center gap-2">
                        <h1
                            class="m-0 truncate text-base font-semibold text-[var(--color-text-primary)]"
                        >
                            {{ manifest.fileName }}
                        </h1>
                        <TagItem>{{ formatFormat(manifest) }}</TagItem>
                    </div>
                    <p class="m-0 mt-0.5 truncate text-xs text-[var(--color-text-tertiary)]">
                        {{ manifest.size.join(' × ') }} · {{ formatNumber(manifest.blockCount) }}
                        {{ formatMessage(messages.blocks).toLocaleLowerCase(locale) }}
                    </p>
                </div>
                <div class="schematic-toolbar-actions flex shrink-0 items-center gap-2">
                    <div class="schematic-command-group">
                        <Button
                            type="quiet"
                            circular
                            icon-only
                            :disabled="
                                editHistory.length === 0 || Boolean(loadingStage) || applyingEdit
                            "
                            :aria-label="formatMessage(messages.undo)"
                            :title="`${formatMessage(messages.undo)} (Ctrl+Z)`"
                            @click="undoEdit"
                            ><UndoIcon />
                        </Button>
                        <Button
                            type="quiet"
                            circular
                            icon-only
                            :disabled="
                                redoHistory.length === 0 || Boolean(loadingStage) || applyingEdit
                            "
                            :aria-label="formatMessage(messages.redo)"
                            :title="`${formatMessage(messages.redo)} (Ctrl+Y)`"
                            @click="redoEdit"
                            ><RedoIcon />
                        </Button>
                    </div>
                    <OverflowMenu
                        class="schematic-command-button min-w-0"
                        :options="openMenuOptions"
                        :aria-label="formatMessage(messages.open)"
                    >
                        <FileArchiveIcon />
                        <span class="schematic-command-label">{{
                            formatMessage(messages.open)
                        }}</span>
                        <ChevronDownIcon class="schematic-command-chevron" />
                        <template #open-file>
                            <FileArchiveIcon />{{ formatMessage(messages.openFile) }}
                        </template>
                        <template #from-instance>
                            <FolderSearchIcon />{{ formatMessage(messages.fromInstance) }}
                        </template>
                    </OverflowMenu>
                    <OverflowMenu
                        class="schematic-command-button min-w-0"
                        :options="exportMenuOptions"
                        :aria-label="formatMessage(messages.exportSchematic)"
                    >
                        <SaveIcon />
                        <span class="schematic-command-label">{{
                            formatMessage(messages.exportSchematic)
                        }}</span>
                        <ChevronDownIcon class="schematic-command-chevron" />
                        <template #export-sponge>
                            <FileArchiveIcon />{{ formatMessage(messages.exportSponge) }}
                        </template>
                        <template #export-litematic>
                            <FileArchiveIcon />{{ formatMessage(messages.exportLitematic) }}
                        </template>
                    </OverflowMenu>
                    <OverflowMenu
                        class="schematic-more-menu"
                        dropdown-class="schematic-more-menu-dropdown"
                        :options="moreMenuOptions"
                        :aria-label="formatMessage(messages.moreActions)"
                        :tooltip="formatMessage(messages.moreActions)"
                    >
                        <MoreHorizontalIcon />
                        <template #schematic-info>
                            <InfoIcon />
                            <span class="schematic-menu-label">{{
                                formatMessage(messages.schematicInfo)
                            }}</span>
                        </template>
                        <template #seamless-glass>
                            <GridIcon />
                            <span class="schematic-menu-label">{{
                                formatMessage(messages.seamlessGlass)
                            }}</span>
                            <span class="schematic-menu-check"
                                ><CheckIcon v-if="seamlessGlass"
                            /></span>
                        </template>
                        <template #screenshot>
                            <ImageIcon />
                            <span class="schematic-menu-label">{{
                                formatMessage(messages.screenshot)
                            }}</span>
                        </template>
                        <template #materials-csv>
                            <DownloadIcon />
                            <span class="schematic-menu-label">{{
                                formatMessage(messages.materialsCsv)
                            }}</span>
                        </template>
                        <template #reload>
                            <RefreshCwIcon />
                            <span class="schematic-menu-label">{{
                                formatMessage(messages.reload)
                            }}</span>
                        </template>
                    </OverflowMenu>
                </div>
            </header>

            <div class="schematic-workbench">
                <section ref="viewport" class="schematic-viewport">
                    <canvas
                        ref="canvas"
                        class="block size-full"
                        :class="
                            viewMode === 'walk'
                                ? walkLocked
                                    ? 'cursor-none'
                                    : 'cursor-pointer'
                                : workspaceTool === 'layer-spacing'
                                  ? 'cursor-grab'
                                  : 'cursor-crosshair'
                        "
                        aria-label="Schematic 3D preview"
                    ></canvas>
                    <nav
                        v-if="viewMode === 'orbit'"
                        class="schematic-mode-toolbar"
                        :aria-label="formatMessage(messages.workspaceTools)"
                    >
                        <Button
                            size="2xs"
                            :color="workspaceTool === 'select' ? 'brand' : undefined"
                            :type="workspaceTool === 'select' ? 'chip-text' : 'quiet'"
                            native-type="button"
                            :aria-pressed="workspaceTool === 'select'"
                            @click="workspaceTool = 'select'"
                        >
                            <CubeIcon />{{ formatMessage(messages.singleSelect) }}
                        </Button>
                        <Button
                            size="2xs"
                            :color="workspaceTool === 'box' ? 'brand' : undefined"
                            :type="workspaceTool === 'box' ? 'chip-text' : 'quiet'"
                            native-type="button"
                            :aria-pressed="workspaceTool === 'box'"
                            @click="workspaceTool = 'box'"
                        >
                            <BoxesIcon />{{ formatMessage(messages.boxSelect) }}
                        </Button>
                        <Button
                            size="2xs"
                            :color="workspaceTool === 'measure' ? 'brand' : undefined"
                            :type="workspaceTool === 'measure' ? 'chip-text' : 'quiet'"
                            native-type="button"
                            :aria-pressed="workspaceTool === 'measure'"
                            @click="workspaceTool = 'measure'"
                        >
                            <ArrowLeftRightIcon />{{ formatMessage(messages.measure) }}
                        </Button>
                        <span class="schematic-mode-divider"></span>
                        <Button
                            size="2xs"
                            :color="workspaceTool === 'layer-spacing' ? 'brand' : undefined"
                            :type="workspaceTool === 'layer-spacing' ? 'chip-text' : 'quiet'"
                            native-type="button"
                            :aria-pressed="workspaceTool === 'layer-spacing'"
                            @click="workspaceTool = 'layer-spacing'"
                        >
                            <UnfoldVerticalIcon />{{ formatMessage(messages.layerSpacing) }}
                        </Button>
                    </nav>
                    <div
                        v-if="viewMode === 'orbit' && workspaceTool === 'layer-spacing'"
                        class="schematic-tool-context"
                    >
                        <span class="schematic-tool-context-label">
                            <UnfoldVerticalIcon />{{ formatMessage(messages.layerSpacing) }}
                        </span>
                        <Slider v-model="layerExplosion" :min="0" :max="100" :step="10" unit="%" />
                    </div>
                    <div
                        v-else-if="
                            viewMode === 'orbit' && workspaceTool === 'measure' && measurementStart
                        "
                        class="schematic-tool-context schematic-measurement-readout w-[min(42rem,calc(100%-1.5rem))]"
                    >
                        <div class="min-w-0 flex-1">
                            <div class="truncate text-xs text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.measurementStart) }}
                                <strong>{{ measurementStart.position.join(', ') }}</strong>
                                · {{ formatMessage(messages.measurementEnd) }}
                                <strong>{{ measurementEnd?.position.join(', ') ?? '—' }}</strong>
                            </div>
                            <div
                                v-if="measurement"
                                class="truncate text-sm text-[var(--color-text-primary)]"
                            >
                                ΔX {{ formatNumber(measurement.delta[0]) }} · ΔY
                                {{ formatNumber(measurement.delta[1]) }} · ΔZ
                                {{ formatNumber(measurement.delta[2]) }} ·
                                {{ formatMessage(messages.measurementDistance) }}
                                {{ formatDecimal(measurement.distance) }} ·
                                {{ formatMessage(messages.measurementSize) }}
                                {{ measurement.size.join(' × ') }}
                            </div>
                        </div>
                        <Button
                            type="quiet"
                            size="2xs"
                            circular
                            icon-only
                            :aria-label="formatMessage(messages.clearMeasurement)"
                            :title="formatMessage(messages.clearMeasurement)"
                            @click="clearMeasurement"
                            ><XIcon />
                        </Button>
                    </div>
                    <div v-if="viewMode === 'orbit'" class="schematic-layer-control">
                        <span class="schematic-layer-heading">
                            <LayersIcon />
                            <span>Y</span>
                        </span>
                        <input
                            v-model.number="layerMaximum"
                            class="schematic-layer-current"
                            type="number"
                            :min="layerFloor"
                            :max="layerCeiling"
                            step="1"
                        />
                        <span class="schematic-layer-limit">{{ layerCeiling }}</span>
                        <input
                            v-model.number="layerMaximum"
                            type="range"
                            class="schematic-layer-slider"
                            :min="layerFloor"
                            :max="layerCeiling"
                            step="1"
                            :aria-label="
                                formatMessage(messages.visibleLayers, {
                                    current: layerMaximum,
                                    visible: visibleLayerCount,
                                    total: totalLayerCount,
                                })
                            "
                        />
                        <span class="schematic-layer-limit">{{ layerFloor }}</span>
                        <span class="schematic-layer-count">
                            {{ visibleLayerCount }}/{{ totalLayerCount }}
                        </span>
                        <Button
                            type="quiet"
                            size="2xs"
                            circular
                            icon-only
                            :disabled="showingAllLayers"
                            :aria-label="formatMessage(messages.allLayers)"
                            :title="formatMessage(messages.allLayers)"
                            @click="showAllLayers"
                            ><RefreshCwIcon />
                        </Button>
                    </div>
                    <div class="schematic-walk-control">
                        <Button
                            size="2xs"
                            :color="viewMode === 'walk' ? 'brand' : undefined"
                            :type="viewMode === 'walk' ? 'chip-text' : 'quiet'"
                            native-type="button"
                            :aria-pressed="viewMode === 'walk'"
                            :title="`${formatMessage(viewMode === 'walk' ? messages.orbitView : messages.walkView)} (V)`"
                            @click="toggleViewMode"
                        >
                            <MoveIcon />
                            {{
                                formatMessage(
                                    viewMode === 'walk' ? messages.orbitView : messages.walkPreview,
                                )
                            }}
                        </Button>
                    </div>
                    <div
                        v-if="viewMode === 'walk' && walkLocked"
                        class="schematic-walk-crosshair"
                        aria-hidden="true"
                    >
                        <span></span><span></span><span></span><span></span><i></i>
                    </div>
                    <div v-if="viewMode === 'walk' && walkLocked" class="schematic-walk-speed">
                        <MoveIcon />
                        {{ formatMessage(messages.walkSpeed, { speed: walkSpeed }) }}
                    </div>
                    <div v-if="viewMode === 'orbit'" class="schematic-canvas-controls">
                        <Button
                            v-tooltip.top="formatMessage(messages.resetView)"
                            size="2xs"
                            circular
                            icon-only
                            :aria-label="formatMessage(messages.resetView)"
                            @click="scene?.fitView()"
                            ><RotateCounterClockwiseIcon
                        /></Button>
                        <Button
                            v-tooltip.top="formatMessage(messages.projection)"
                            :type="projection === 'orthographic' ? 'base' : 'outlined'"
                            size="2xs"
                            circular
                            icon-only
                            :aria-label="formatMessage(messages.projection)"
                            @click="toggleProjection"
                            ><ScanEyeIcon
                        /></Button>
                        <Button
                            v-tooltip.top="formatMessage(messages.grid)"
                            :type="showGrid ? 'base' : 'outlined'"
                            size="2xs"
                            circular
                            icon-only
                            :aria-label="formatMessage(messages.grid)"
                            @click="showGrid = !showGrid"
                            ><GridIcon
                        /></Button>
                        <Button
                            v-tooltip.top="formatMessage(messages.bounds)"
                            :type="showBounds ? 'base' : 'outlined'"
                            size="2xs"
                            circular
                            icon-only
                            :aria-label="formatMessage(messages.bounds)"
                            @click="showBounds = !showBounds"
                            ><BoxIcon
                        /></Button>
                        <Button
                            v-tooltip.top="formatMessage(messages.translucent)"
                            :type="showTranslucent ? 'base' : 'outlined'"
                            size="2xs"
                            circular
                            icon-only
                            :aria-label="formatMessage(messages.translucent)"
                            @click="showTranslucent = !showTranslucent"
                            ><EyeIcon
                        /></Button>
                        <Button
                            v-tooltip.top="
                                `${formatMessage(isFullscreen ? messages.exitFullscreen : messages.fullscreen)} (F11)`
                            "
                            :type="isFullscreen ? 'base' : 'outlined'"
                            size="2xs"
                            circular
                            icon-only
                            :aria-label="
                                formatMessage(
                                    isFullscreen ? messages.exitFullscreen : messages.fullscreen,
                                )
                            "
                            @click="toggleFullscreen"
                            ><ContractIcon v-if="isFullscreen" /><MaximizeIcon v-else
                        /></Button>
                    </div>
                    <div v-if="loadingStage" class="schematic-loading-status">
                        <SpinnerIcon class="size-4 animate-spin" />{{ loadingLabel
                        }}<span v-if="loadingStage === 'mesh'"
                            >{{ completedMeshes }} / {{ totalMeshes }}</span
                        >
                    </div>
                    <div v-if="contextLost" class="schematic-context-lost">
                        <TriangleAlertIcon />{{ formatMessage(messages.webglLost) }}
                    </div>
                    <div v-if="dragging" class="schematic-drop-overlay">
                        <FileArchiveIcon />{{ formatMessage(messages.drop) }}
                    </div>
                    <footer class="schematic-statusbar">
                        <span v-if="selected && selectedBlock" class="truncate">
                            <strong>{{ selected.position.join(', ') }}</strong> ·
                            {{ selectedStateText }} ·
                            {{
                                formatMessage(messages.selectedCount, {
                                    count: selectedBlocks.length,
                                })
                            }}
                        </span>
                        <span v-else-if="error" class="truncate text-brand-red">{{ error }}</span>
                        <span v-else class="truncate text-[var(--color-text-tertiary)]"
                            >{{ manifest.min.join(', ') }} → {{ manifest.max.join(', ') }}</span
                        >
                    </footer>
                </section>

                <aside
                    class="schematic-inspector flex min-h-0 flex-col border-l-0 bg-surface-2"
                    :class="{ 'schematic-inspector-collapsed': !inspectorOpen }"
                >
                    <button
                        type="button"
                        class="schematic-inspector-toggle hidden"
                        @click="inspectorOpen = !inspectorOpen"
                    >
                        <strong>{{ formatMessage(messages.inspector) }}</strong
                        ><ChevronDownIcon :class="{ 'rotate-180': inspectorOpen }" />
                    </button>
                    <div class="schematic-inspector-body flex min-h-0 flex-1 flex-col gap-3 p-3">
                        <Tabs
                            v-if="viewMode === 'orbit'"
                            v-model:value="inspectorTab"
                            :tabs="inspectorTabs"
                        />

                        <div
                            v-if="viewMode === 'orbit' && inspectorTab === 'edit'"
                            class="schematic-inspector-scroll"
                        >
                            <section
                                class="inspector-section flex flex-col gap-2 border-t border-surface-5 pt-3"
                            >
                                <h2><BoxesIcon />{{ formatMessage(messages.selectedBlock) }}</h2>
                                <div class="selection-summary">
                                    <strong>{{
                                        formatMessage(messages.selectedCount, {
                                            count: selectedBlocks.length,
                                        })
                                    }}</strong>
                                    <span
                                        v-if="selectedBlock"
                                        class="truncate text-xs text-[var(--color-text-tertiary)]"
                                        >{{ blockDisplayName(selectedBlock.name) }}</span
                                    >
                                </div>
                                <p
                                    v-if="workspaceTool === 'box' && selectionAnchor"
                                    class="m-0 text-xs text-brand"
                                >
                                    {{ formatMessage(messages.boxSelectPending) }}
                                </p>
                                <div class="editor-action-grid">
                                    <Button
                                        type="outlined"
                                        :disabled="!selectedBlock"
                                        @click="expandSelectionByMaterial()"
                                        ><ListIcon />{{ formatMessage(messages.selectMaterial) }}
                                    </Button>
                                    <Button
                                        type="outlined"
                                        :disabled="!selectedBlocks.length"
                                        @click="expandSelectionByLayer"
                                        ><LayersIcon />{{ formatMessage(messages.selectLayer) }}
                                    </Button>
                                    <Button
                                        type="outlined"
                                        :disabled="!selectedBlocks.length"
                                        @click="expandConnectedSelection"
                                        ><BoxesIcon />{{ formatMessage(messages.selectConnected) }}
                                    </Button>
                                    <Button
                                        type="quiet"
                                        :disabled="!selectedBlocks.length"
                                        @click="clearBlockSelection"
                                        ><XIcon />{{ formatMessage(messages.clearSelection) }}
                                    </Button>
                                </div>
                            </section>

                            <section
                                v-if="selected && selectedBlock"
                                class="inspector-section flex flex-col gap-2 border-t border-surface-5 pt-3"
                            >
                                <h2><SettingsIcon />{{ formatMessage(messages.blockState) }}</h2>
                                <SchematicStateCard
                                    :properties="selectedStateProperties"
                                    :disabled="!canEditSelection"
                                    @change="commitStateProperty"
                                />
                            </section>

                            <section
                                class="inspector-section flex flex-col gap-2 border-t border-surface-5 pt-3"
                            >
                                <h2><EyeIcon />{{ formatMessage(messages.visibility) }}</h2>
                                <div class="editor-action-grid">
                                    <Button
                                        type="outlined"
                                        :disabled="!selectedBlocks.length"
                                        @click="hideSelectedBlocks"
                                        ><EyeOffIcon />{{ formatMessage(messages.hideSelection) }}
                                    </Button>
                                    <Button
                                        type="outlined"
                                        :disabled="!selectedBlocks.length"
                                        @click="showOnlySelectedBlocks"
                                        ><ScanEyeIcon />{{
                                            formatMessage(messages.isolateSelection)
                                        }}
                                    </Button>
                                    <Button
                                        type="quiet"
                                        :disabled="hiddenBlocks.size === 0 && !isolateSelection"
                                        @click="showAllBlocks"
                                        ><EyeIcon />{{ formatMessage(messages.showAll) }}
                                    </Button>
                                    <Button
                                        type="quiet"
                                        :disabled="!selectedBlocks.length"
                                        @click="copySelectedCoordinates"
                                        ><CopyIcon />{{ formatMessage(messages.coordinates) }}
                                    </Button>
                                </div>
                            </section>

                            <section
                                class="inspector-section flex flex-col gap-2 border-t border-surface-5 pt-3"
                            >
                                <h2><EditIcon />{{ formatMessage(messages.edit) }}</h2>
                                <div class="editor-action-grid">
                                    <Button
                                        type="colored"
                                        color="brand"
                                        :disabled="!canEditSelection"
                                        @click="openBlockPicker"
                                        ><EditIcon />{{ formatMessage(messages.replaceSelected) }}
                                    </Button>
                                    <Button
                                        type="outlined"
                                        color="red"
                                        :disabled="!canEditSelection"
                                        @click="
                                            commitSelectionEdit(
                                                0,
                                                formatMessage(messages.deleteSelected),
                                            )
                                        "
                                        ><TrashIcon />{{ formatMessage(messages.deleteSelected) }}
                                    </Button>
                                </div>
                            </section>

                            <section
                                class="inspector-section flex flex-col gap-2 border-t border-surface-5 pt-3"
                            >
                                <h2>
                                    <RotateClockwiseIcon />{{ formatMessage(messages.transform) }}
                                </h2>
                                <div class="editor-action-grid">
                                    <Button
                                        type="outlined"
                                        :disabled="Boolean(loadingStage) || applyingEdit"
                                        @click="
                                            transformStructure(
                                                'rotate_counter_clockwise',
                                                formatMessage(messages.rotateCounterClockwise),
                                            )
                                        "
                                        ><RotateCounterClockwiseIcon />{{
                                            formatMessage(messages.rotateCounterClockwise)
                                        }}
                                    </Button>
                                    <Button
                                        type="outlined"
                                        :disabled="Boolean(loadingStage) || applyingEdit"
                                        @click="
                                            transformStructure(
                                                'rotate_clockwise',
                                                formatMessage(messages.rotateClockwise),
                                            )
                                        "
                                        ><RotateClockwiseIcon />{{
                                            formatMessage(messages.rotateClockwise)
                                        }}
                                    </Button>
                                    <Button
                                        type="outlined"
                                        :disabled="Boolean(loadingStage) || applyingEdit"
                                        @click="
                                            transformStructure(
                                                'mirror_x',
                                                formatMessage(messages.mirrorX),
                                            )
                                        "
                                        ><ArrowLeftRightIcon />{{ formatMessage(messages.mirrorX) }}
                                    </Button>
                                    <Button
                                        type="outlined"
                                        :disabled="Boolean(loadingStage) || applyingEdit"
                                        @click="
                                            transformStructure(
                                                'mirror_z',
                                                formatMessage(messages.mirrorZ),
                                            )
                                        "
                                        ><ArrowUpDownIcon />{{ formatMessage(messages.mirrorZ) }}
                                    </Button>
                                </div>
                            </section>
                        </div>

                        <div v-else class="schematic-inspector-scroll">
                            <div class="flex items-center gap-2 p-1">
                                <StyledInput
                                    v-model="materialSearch"
                                    class="min-w-0 flex-1"
                                    :icon="SearchIcon"
                                    type="search"
                                    :placeholder="formatMessage(messages.searchMaterials)"
                                    clearable
                                />
                                <Button
                                    type="outlined"
                                    size="2xs"
                                    circular
                                    icon-only
                                    :aria-label="formatMessage(messages.materialsJson)"
                                    :title="formatMessage(messages.materialsJson)"
                                    @click="exportMaterialsJson"
                                    ><DownloadIcon />
                                </Button>
                            </div>
                            <p
                                v-if="visibleMaterials.length === 0"
                                class="m-0 py-8 text-center text-sm text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.noMaterials) }}
                            </p>
                            <ul
                                v-else
                                class="material-list flex flex-col gap-0.5 list-none m-0 p-0"
                            >
                                <li v-for="material in visibleMaterials" :key="material.name">
                                    <button
                                        type="button"
                                        class="material-row"
                                        @click="
                                            expandedMaterial =
                                                expandedMaterial === material.name
                                                    ? ''
                                                    : material.name
                                        "
                                    >
                                        <SchematicMaterialSwatch
                                            v-if="resources"
                                            class="material-swatch"
                                            :atlas="resources.atlas"
                                            :uv="materialTextureUv(material.name)"
                                            :fallback-color="materialColor(material.name)"
                                            :state="materialStates(material.name)[0]"
                                            :resources="resources"
                                        />
                                        <span
                                            v-else
                                            class="material-swatch"
                                            :style="{ background: materialColor(material.name) }"
                                        ></span>
                                        <span class="min-w-0 flex-1 truncate text-left">{{
                                            blockDisplayName(material.name)
                                        }}</span>
                                        <strong class="tabular-nums">{{
                                            formatNumber(material.count)
                                        }}</strong>
                                    </button>
                                    <div
                                        v-if="expandedMaterial === material.name"
                                        class="material-states"
                                    >
                                        <code
                                            v-for="(state, index) in materialStates(material.name)"
                                            :key="index"
                                            >{{
                                                Object.entries(state.properties)
                                                    .map(([key, value]) => `${key}=${value}`)
                                                    .join(', ') || 'default'
                                            }}</code
                                        >
                                        <div
                                            v-if="viewMode === 'orbit'"
                                            class="flex flex-wrap gap-2 pt-1"
                                        >
                                            <Button
                                                type="outlined"
                                                size="2xs"
                                                @click="expandSelectionByMaterial(material.name)"
                                                ><BoxesIcon />{{
                                                    formatMessage(messages.selectAllMaterial)
                                                }}
                                            </Button>
                                            <Button
                                                type="quiet"
                                                size="2xs"
                                                @click="useMaterialForReplacement(material.name)"
                                                ><EditIcon />{{
                                                    formatMessage(messages.useForReplace)
                                                }}
                                            </Button>
                                        </div>
                                    </div>
                                </li>
                            </ul>
                        </div>
                    </div>
                </aside>
            </div>
        </template>

        <SchematicInstancePickerModal ref="instancePicker" @open="(source) => openSource(source)" />
        <SchematicBlockPickerModal
            v-if="manifest"
            ref="blockPicker"
            :blocks="replacementBlocks"
            :resources="resources"
            :selected-count="selectedBlocks.length"
            :display-name="blockDisplayName"
            @replace="replaceSelectedWith"
        />
        <SchematicInfoModal
            v-if="manifest"
            ref="infoModal"
            :manifest="manifest"
            :format="formatFormat(manifest)"
            :warnings="warnings"
            :region-visibility="regionVisibility"
            @region-visibility="setRegionVisibility"
            @focus-region="(region) => scene?.focusRegion(region)"
        />
    </main>
</template>

<style scoped>
.schematic-page {
    position: relative;
    display: flex;
    height: calc(100vh - 4rem);
    min-height: 36rem;
    width: 100%;
    flex-direction: column;
    overflow: hidden;
    background: var(--color-bg);
}

.schematic-empty {
    display: flex;
    min-height: 22rem;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 2rem;
}

.schematic-toolbar {
    display: flex;
    min-height: 4.25rem;
    flex-shrink: 0;
    align-items: center;
    gap: 1rem;
    border-bottom: 1px solid var(--surface-5);
    padding: 0.65rem 1rem;
    background: var(--surface-2);
}

.schematic-command-group {
    display: flex;
    align-items: center;
    gap: 0.125rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    padding: 0.125rem;
    background: var(--surface-2);
}

.schematic-command-chevron {
    width: 0.875rem !important;
    height: 0.875rem !important;
    min-width: 0.875rem !important;
    min-height: 0.875rem !important;
}

.schematic-menu-check {
    display: flex;
    width: 1rem;
    height: 1rem;
    grid-column: 3;
    align-items: center;
    justify-content: center;
}

.schematic-menu-check svg {
    width: 1rem;
    height: 1rem;
    color: var(--color-brand);
}

:global(.schematic-more-menu-dropdown .btn) {
    display: grid;
    width: 100%;
    grid-template-columns: 1.25rem minmax(0, 1fr) 1rem;
    justify-content: stretch;
    text-align: left;
}

:global(.schematic-more-menu-dropdown .btn > svg) {
    grid-column: 1;
}

:global(.schematic-more-menu-dropdown .schematic-menu-label) {
    min-width: 0;
    grid-column: 2;
    text-align: left;
}

.schematic-workbench {
    display: grid;
    min-height: 0;
    flex: 1;
    grid-template-columns: minmax(0, 1fr) 340px;
    background: var(--surface-1);
}

.schematic-viewport {
    position: relative;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border-right: 1px solid var(--color-divider);
    background: var(--surface-1);
    box-shadow: inset 0 0 0 1px var(--color-divider);
}

.schematic-viewport:fullscreen {
    height: 100vh;
    border-right: none;
    box-shadow: none;
}

.schematic-mode-toolbar,
.schematic-layer-control,
.schematic-walk-control,
.schematic-tool-context,
.schematic-canvas-controls {
    z-index: 10;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    padding: 0.25rem;
    background: color-mix(in srgb, var(--color-bg) 90%, transparent);
    box-shadow: var(--shadow-card);
    backdrop-filter: blur(12px);
}

.schematic-mode-toolbar {
    position: absolute;
    top: 3.75rem;
    left: 50%;
    display: flex;
    max-width: calc(100% - 1.5rem);
    align-items: center;
    gap: 0.15rem;
    transform: translateX(-50%);
}

.schematic-walk-control {
    position: absolute;
    top: 0.75rem;
    right: 0.75rem;
    display: flex;
}

.schematic-mode-divider {
    width: 1px;
    height: 1.25rem;
    margin: 0 0.15rem;
    flex: none;
    background: var(--surface-5);
}

.schematic-tool-context {
    position: absolute;
    z-index: 9;
    top: 6.75rem;
    left: 50%;
    display: flex;
    width: min(32rem, calc(100% - 1.5rem));
    align-items: center;
    gap: 0.75rem;
    transform: translateX(-50%);
    padding: 0.55rem 0.65rem;
}

.schematic-tool-context-label {
    display: flex;
    flex: none;
    align-items: center;
    gap: 0.35rem;
    color: var(--color-text-primary);
    font-size: 0.78rem;
    font-weight: 600;
}

.schematic-tool-context-label svg {
    width: 1rem;
    height: 1rem;
    color: var(--color-brand);
}

.schematic-canvas-controls {
    position: absolute;
    right: 0.75rem;
    bottom: 2.5rem;
    display: flex;
    gap: 0.35rem;
}

.schematic-walk-crosshair {
    position: absolute;
    z-index: 10;
    top: 50%;
    left: 50%;
    width: 1.5rem;
    height: 1.5rem;
    transform: translate(-50%, -50%);
    pointer-events: none;
}

.schematic-walk-crosshair span,
.schematic-walk-crosshair i {
    position: absolute;
    display: block;
    background: rgb(255 255 255 / 72%);
    box-shadow: 0 0 1px rgb(0 0 0 / 85%);
}

.schematic-walk-crosshair span:nth-child(1),
.schematic-walk-crosshair span:nth-child(2) {
    left: calc(50% - 1px);
    width: 2px;
    height: 0.5rem;
}

.schematic-walk-crosshair span:nth-child(1) {
    top: 0;
}

.schematic-walk-crosshair span:nth-child(2) {
    bottom: 0;
}

.schematic-walk-crosshair span:nth-child(3),
.schematic-walk-crosshair span:nth-child(4) {
    top: calc(50% - 1px);
    width: 0.5rem;
    height: 2px;
}

.schematic-walk-crosshair span:nth-child(3) {
    left: 0;
}

.schematic-walk-crosshair span:nth-child(4) {
    right: 0;
}

.schematic-walk-crosshair i {
    top: calc(50% - 2px);
    left: calc(50% - 2px);
    width: 4px;
    height: 4px;
    opacity: 0.7;
}

.schematic-walk-speed {
    position: absolute;
    z-index: 10;
    right: 0.75rem;
    bottom: 2.5rem;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    border: 1px solid var(--color-divider);
    border-radius: var(--radius-md);
    padding: 0.4rem 0.6rem;
    background: color-mix(in srgb, var(--surface-2) 88%, transparent);
    box-shadow: 0 0.5rem 1.5rem rgb(0 0 0 / 35%);
    color: rgb(255 255 255 / 72%);
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
    backdrop-filter: blur(12px);
}

.schematic-walk-speed svg {
    width: 1rem;
    height: 1rem;
    color: var(--color-brand);
}

.schematic-layer-control {
    position: absolute;
    z-index: 10;
    top: 50%;
    left: 0.75rem;
    display: grid;
    width: 3.25rem;
    height: min(24rem, calc(100% - 8rem));
    min-height: 12rem;
    grid-template-rows: auto auto auto minmax(4rem, 1fr) auto auto auto;
    justify-items: center;
    gap: 0.35rem;
    transform: translateY(-50%);
}

.schematic-layer-heading {
    display: flex;
    align-items: center;
    gap: 0.2rem;
    flex: none;
    color: var(--color-text-tertiary);
    font-size: 0.72rem;
    font-weight: 700;
}

.schematic-layer-heading svg {
    width: 1rem;
    height: 1rem;
    color: var(--color-brand);
}

.schematic-layer-current {
    min-height: 0.5rem;
    height: 1.4em;
    border-radius: 0.2rem;
    padding: 0.2rem;
    color: var(--color-text-primary);
    font-size: 0.7rem;
    font-weight: bold;
    font-variant-numeric: tabular-nums;
    text-align: center;
}

.schematic-layer-limit,
.schematic-layer-count {
    color: var(--color-text-tertiary);
    font-size: 0.62rem;
    font-variant-numeric: tabular-nums;
}

.schematic-layer-slider {
    width: 0.35rem;
    height: 100%;
    min-height: 4rem;
    cursor: pointer;
    appearance: none;
    writing-mode: vertical-lr;
    direction: rtl;
    border: 0;
    border-radius: 999px;
    padding: 0;
    background: var(--color-text-default);
    box-shadow: none;
}

.schematic-layer-slider::-webkit-slider-thumb {
    width: 0.75rem;
    height: 0.75rem;
    cursor: grab;
    appearance: none;
    border: 0;
    border-radius: 50%;
    background: var(--color-brand);
}

.schematic-layer-slider::-webkit-slider-thumb:active {
    cursor: grabbing;
}

.schematic-layer-control :deep(button) {
    width: 2rem;
    height: 2rem;
}

.schematic-loading-status,
.schematic-context-lost {
    position: absolute;
    top: 4rem;
    right: 0.75rem;
    display: flex;
    align-items: center;
    gap: 0.45rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    padding: 0.45rem 0.65rem;
    background: color-mix(in srgb, var(--color-bg) 90%, transparent);
    color: var(--color-text-tertiary);
    font-size: 0.75rem;
}

.schematic-context-lost {
    max-width: 28rem;
    color: var(--color-orange);
}

.schematic-drop-overlay {
    position: absolute;
    inset: 0.75rem;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    border: 2px dashed var(--color-brand);
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--color-bg) 88%, transparent);
    color: var(--color-brand);
    font-weight: 700;
    pointer-events: none;
}

.schematic-drop-overlay svg {
    width: 1.5rem;
    height: 1.5rem;
}

.schematic-statusbar {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    display: flex;
    height: 1.75rem;
    align-items: center;
    border-top: 1px solid var(--color-divider);
    padding: 0 0.65rem;
    background: color-mix(in srgb, var(--surface-2) 90%, transparent);
    color: var(--color-text-primary);
    font-size: 0.72rem;
    backdrop-filter: blur(8px);
}

.selection-summary {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    padding: 0.65rem 0.75rem;
    background: var(--surface-3);
    color: var(--color-text-primary);
}

.editor-action-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.5rem;
}

.editor-action-grid :deep(button) {
    width: 100%;
    min-width: 0;
}

.editor-action-grid :deep(button) {
    justify-content: flex-start;
}

.schematic-inspector-body > :deep([role='tablist']) {
    width: 100%;
}

.schematic-inspector-body > :deep([role='tablist'] button) {
    min-width: 0;
    flex: 1;
    padding-inline: 0.4rem;
}

.schematic-inspector-scroll {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    gap: 0.75rem;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding-right: 0.15rem;
}

.inspector-section:first-child {
    border-top: 0;
    padding-top: 0;
}

.inspector-section h2 {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0;
    color: var(--color-text-primary);
    font-size: 0.82rem;
}

.inspector-section h2 svg {
    width: 1rem;
    height: 1rem;
}

.material-list li {
    content-visibility: auto;
    contain-intrinsic-size: 2.5rem;
}

.material-row {
    display: flex;
    min-height: 2.5rem;
    width: 100%;
    cursor: pointer;
    align-items: center;
    gap: 0.5rem;
    border: 0;
    border-radius: var(--radius-sm);
    padding: 0.35rem 0.45rem;
    background: transparent;
    color: var(--color-text-primary);
    font: inherit;
    font-size: 0.72rem;
}

.material-row:hover {
    background: var(--surface-4);
}

.material-swatch {
    width: 1.4rem;
    height: 1.4rem;
    flex-shrink: 0;
    border: 1px solid var(--color-divider);
    border-radius: 0.2rem;
    image-rendering: pixelated;
}

.material-states {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.25rem 0.45rem 0.55rem 2.35rem;
}

.material-states code {
    overflow-wrap: anywhere;
    color: var(--color-text-tertiary);
    font-size: 0.65rem;
}

@media (max-width: 900px) {
    .schematic-page {
        height: auto;
        min-height: calc(100vh - 4rem);
        overflow-y: auto;
    }

    .schematic-toolbar {
        align-items: flex-start;
        flex-wrap: wrap;
    }

    .schematic-toolbar-actions {
        width: 100%;
        overflow-x: auto;
    }

    .schematic-workbench {
        display: flex;
        min-height: 0;
        flex: none;
        flex-direction: column;
    }

    .schematic-viewport {
        height: min(64vh, 34rem);
        min-height: 23rem;
    }

    .schematic-mode-toolbar {
        max-width: calc(100% - 1.5rem);
    }

    .schematic-inspector {
        min-height: 0;
        border-top: 1px solid var(--surface-5);
        border-left: 0;
    }

    .schematic-inspector-toggle {
        display: flex;
        width: 100%;
        cursor: pointer;
        align-items: center;
        justify-content: space-between;
        border: 0;
        padding: 0.75rem 1rem;
        background: transparent;
        color: var(--color-text-primary);
    }

    .schematic-inspector-toggle svg {
        width: 1rem;
        height: 1rem;
        transition: transform 150ms ease;
    }

    .schematic-inspector-collapsed .schematic-inspector-body {
        display: none;
    }

    .schematic-inspector-body {
        min-height: 26rem;
        padding: 0 1rem 1rem;
    }
}

@media (max-width: 700px) {
    .schematic-walk-speed {
        bottom: 5.5rem;
    }
}

@media (max-width: 520px) {
    .schematic-command-label {
        display: none;
    }

    .schematic-command-chevron {
        display: none;
    }

    .schematic-mode-toolbar {
        right: 0.75rem;
        left: 0.75rem;
        max-width: none;
        transform: none;
        overflow-x: auto;
    }

    .schematic-tool-context {
        right: 0.75rem;
        left: 0.75rem;
        width: auto;
        transform: none;
    }

    .schematic-tool-context:not(.schematic-measurement-readout) {
        align-items: stretch;
        flex-direction: column;
    }

    .schematic-canvas-controls {
        max-width: calc(100% - 1.5rem);
        overflow-x: auto;
    }

    .schematic-loading-status {
        top: auto;
        bottom: 8.5rem;
        left: 0.75rem;
        width: fit-content;
    }
}

@media (max-width: 520px) {
    .schematic-layer-control {
        left: 0.5rem;
        height: min(20rem, calc(100% - 8rem));
    }

    .schematic-walk-control {
        top: 0.75rem;
        right: 0.5rem;
    }
}
</style>
