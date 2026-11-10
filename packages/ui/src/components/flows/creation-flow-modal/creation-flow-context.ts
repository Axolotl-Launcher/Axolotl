import type { LauncherMeta } from '@modrinth/api-client'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, type ComputedRef, type Ref, ref, type ShallowRef, watch } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { useDebugLogger } from '#ui/composables/debug-logger'
import {
    defineMessages,
    type MessageDescriptor,
    useVIntl,
    type VIntlFormatters,
} from '#ui/composables/i18n'
import { defaultInstanceName } from '#ui/utils/loaders'

import { createContext, injectModrinthClient } from '../../../providers'
import type { ImportableLauncher } from '../../../providers/instance-import'
import type { MultiStageModal, StageConfigInput } from '../../base'
import type { ComboboxOption } from '../../base/Combobox.vue'
import {
    loaderMetadataCacheKey,
    loaderMetadataQueryKey,
    type LoaderMetadataStatus,
    loaderSupportState,
} from './loader-metadata'
import { stageConfigs } from './stages'

export type FlowType = 'world' | 'instance'
export type SetupType = 'modpack' | 'custom' | 'vanilla'
export type Gamemode = 'survival' | 'creative' | 'hardcore'
export type Difficulty = 'peaceful' | 'easy' | 'normal' | 'hard'
export type LoaderVersionType = 'stable' | 'latest' | 'other'
export type AdjunctLoader = 'optifine' | 'lite_loader'
export type GeneratorSettingsMode = 'default' | 'flat' | 'custom'
export type GameDirOverrideMode = 'builtin' | 'isolated' | 'not-isolated'
export type LoaderManifest = LauncherMeta.Manifest.v0.Manifest
export type LoaderManifestResolver = (
    loader: string,
    gameVersion?: string,
) => Promise<LoaderManifest>
export interface LoaderVersionEntry {
    id: string
    stable: boolean
}

export interface ProjectVersionCompatibility {
    id: string
    game_versions?: string[]
    loaders?: string[]
}

export type OptiFabricCompatibilityResolver = (gameVersion: string) => Promise<boolean>

const LOADER_MANIFEST_STALE_TIME = 30 * 60 * 1000
const paperSupportedVersionsQueryKey = ['creation-flow', 'paper', 'supported-versions'] as const
const purpurSupportedVersionsQueryKey = ['creation-flow', 'purpur', 'supported-versions'] as const

export const creationFlowMessages = defineMessages({
    createWorldTitle: {
        id: 'creation-flow.title.create-world',
        defaultMessage: 'Create world',
    },
    createInstanceTitle: {
        id: 'creation-flow.title.create-instance',
        defaultMessage: 'Create instance',
    },
    createWorldButton: {
        id: 'creation-flow.button.create-world',
        defaultMessage: 'Create world',
    },
    createInstanceButton: {
        id: 'creation-flow.button.create-instance',
        defaultMessage: 'Create instance',
    },
    finishButton: {
        id: 'creation-flow.button.finish',
        defaultMessage: 'Finish',
    },
    importInstanceTitle: {
        id: 'creation-flow.title.import-instance',
        defaultMessage: 'Import instance',
    },
    importButton: {
        id: 'creation-flow.button.import',
        defaultMessage: 'Import',
    },
    importInstancesButton: {
        id: 'creation-flow.button.import-instances',
        defaultMessage: 'Import {count, plural, one {# instance} other {# instances}}',
    },
    chooseModpackTitle: {
        id: 'creation-flow.title.choose-modpack',
        defaultMessage: 'Choose modpack',
    },
})

export const flowTypeHeadingMessages: Record<FlowType, MessageDescriptor> = {
    world: creationFlowMessages.createWorldTitle,
    instance: creationFlowMessages.createInstanceTitle,
}

export interface ModpackSelection {
    projectId: string
    versionId: string
    name: string
    iconUrl?: string
}

export interface ModpackSearchHit {
    title: string
    iconUrl?: string
    latestVersion?: string
}

export interface ModpackSearchResult {
    hits: {
        project_id: string
        title: string
        icon_url: string
        latest_version?: string
    }[]
    total_hits: number
    offset: number
    limit: number
}

export interface CreationFlowContextValue {
    // Flow
    flowType: FlowType
    formatMessage: VIntlFormatters['formatMessage']

    // Configuration
    availableLoaders: string[]
    showSnapshotToggle: boolean
    disableClose: boolean

    // Initial values
    initialLoader: string | null
    initialGameVersion: string | null

    // State
    setupType: Ref<SetupType | null>
    isImportMode: Ref<boolean>
    worldName: Ref<string>
    gamemode: Ref<Gamemode>
    difficulty: Ref<Difficulty>
    worldSeed: Ref<string>
    worldTypeOption: Ref<string>
    generateStructures: Ref<boolean>
    generatorSettingsMode: Ref<GeneratorSettingsMode>
    generatorSettingsCustom: Ref<string>

    // Instance-specific state
    instanceName: Ref<string>
    autoInstanceName: ComputedRef<string>
    instanceIcon: Ref<File | null>
    instanceIconUrl: Ref<string | null>
    instanceIconPath: Ref<string | null>
    gameDirOverrideMode: Ref<GameDirOverrideMode>
    gameDirOverride: Ref<string | null>

    // Loader/version state (custom setup)
    selectedLoader: Ref<string | null>
    selectedGameVersion: Ref<string | null>
    loaderVersionType: Ref<LoaderVersionType>
    selectedLoaderVersion: Ref<string | null>
    selectedAdjuncts: Ref<AdjunctLoader[]>
    hideLoaderChips: ComputedRef<boolean>
    hideLoaderVersion: ComputedRef<boolean>
    showSnapshots: Ref<boolean>
    loaderVersionsCache: Ref<Record<string, LoaderManifest>>
    loaderMetadataStatus: Ref<Record<string, LoaderMetadataStatus>>
    paperSupportedVersions: Ref<Set<string> | null>
    purpurSupportedVersions: Ref<Set<string> | null>

    // Modpack state
    modpackSelection: Ref<ModpackSelection | null>
    modpackFile: Ref<File | null>
    modpackFilePath: Ref<string | null>

    // Modpack search state (persisted across stage navigation)
    modpackSearchProjectId: Ref<string | undefined>
    modpackSearchVersionId: Ref<string | undefined>
    modpackSearchOptions: Ref<ComboboxOption<string>[]>
    modpackVersionOptions: Ref<ComboboxOption<string>[]>
    modpackSearchHits: Ref<Record<string, ModpackSearchHit>>

    // Import state (instance flow only)
    importLaunchers: Ref<ImportableLauncher[]>
    importSelectedInstances: Ref<Record<string, Set<string>>>
    importSearchQuery: Ref<string>
    importAsSymlink: Ref<boolean>

    // Loading state (set when finish() is called, cleared on reset)
    loading: Ref<boolean>
    finishDisabled: ComputedRef<boolean>
    finishDisabledTooltip: ComputedRef<string | undefined>

    // Skip setup type (used when creation flow is entered from /create page)
    skipSetupType: Ref<boolean>

    // Modal
    modal: ShallowRef<ComponentExposed<typeof MultiStageModal> | null>
    stageConfigs: StageConfigInput<CreationFlowContextValue>[]

    // Callbacks
    onBack: (() => void) | null

    /** @see ImportInstanceStage.vue — ImportFilePayload documentation */
    onImportFileReceived?: (payload: {
        file: File | null
        filePath: string | null
        source: 'file-picker' | 'drag-drop'
    }) => void

    // Methods
    reset: (instanceCount?: number) => Promise<void>
    setSetupType: (type: SetupType) => void
    setImportMode: () => void
    browseModpacks: () => void
    finish: () => void
    fetchLoaderMetadata: (loader?: string | null, gameVersion?: string) => Promise<void>
    prefetchLoaderMetadata: (gameVersion?: string) => Promise<void>

    // Platform-provided search
    searchModpacks: (query: string, limit?: number) => Promise<ModpackSearchResult>
    getProjectVersions: (projectId: string) => Promise<ProjectVersionCompatibility[]>
    hasCompatibleOptiFabric: OptiFabricCompatibilityResolver
    getLoaderManifest: LoaderManifestResolver | null
}

export const [injectCreationFlowContext, provideCreationFlowContext] =
    createContext<CreationFlowContextValue>('CreationFlowModal')

// TODO: replace with actual world count from the world list once available
let worldCounter = 0

export interface CreationFlowOptions {
    availableLoaders?: string[]
    showSnapshotToggle?: boolean
    disableClose?: boolean
    initialLoader?: string
    initialGameVersion?: string
    fetchExistingInstanceNames?: () => Promise<string[]>
    onBack?: () => void
    searchModpacks?: (query: string, limit?: number) => Promise<ModpackSearchResult>
    getProjectVersions?: (projectId: string) => Promise<ProjectVersionCompatibility[]>
    hasCompatibleOptiFabric?: OptiFabricCompatibilityResolver
    getLoaderManifest?: LoaderManifestResolver
    finishDisabled?: ComputedRef<boolean>
    finishDisabledTooltip?: ComputedRef<string | undefined>

    /** @see ImportInstanceStage.vue — ImportFilePayload documentation */
    onImportFileReceived?: (payload: {
        file: File | null
        filePath: string | null
        source: 'file-picker' | 'drag-drop'
    }) => void
}

export function createCreationFlowContext(
    modal: ShallowRef<ComponentExposed<typeof MultiStageModal> | null>,
    flowType: FlowType,
    emit: {
        browseModpacks: () => void
        create: (config: CreationFlowContextValue) => void
    },
    options: CreationFlowOptions = {},
): CreationFlowContextValue {
    const debug = useDebugLogger('CreationFlow')
    const client = injectModrinthClient()
    const queryClient = useQueryClient()
    const { formatMessage } = useVIntl()
    const availableLoaders = options.availableLoaders ?? ['fabric', 'neoforge', 'forge', 'quilt']
    const showSnapshotToggle = options.showSnapshotToggle ?? false
    const disableClose = options.disableClose ?? false
    const initialLoader = options.initialLoader ?? null
    const initialGameVersion = options.initialGameVersion ?? null
    const onBack = options.onBack ?? null
    const onImportFileReceived = options.onImportFileReceived
    const searchModpacks = options.searchModpacks!
    const getProjectVersions =
        options.getProjectVersions ??
        ((projectId: string) =>
            client.labrinth.versions_v2.getProjectVersions(projectId, {
                include_changelog: false,
            }))
    const hasCompatibleOptiFabric = options.hasCompatibleOptiFabric ?? (async () => false)
    const getLoaderManifest = options.getLoaderManifest ?? null
    const finishDisabled = options.finishDisabled ?? computed(() => false)
    const finishDisabledTooltip = options.finishDisabledTooltip ?? computed(() => undefined)

    const setupType = ref<SetupType | null>(null)
    const isImportMode = ref(false)
    const worldName = ref('')
    const gamemode = ref<Gamemode>('survival')
    const difficulty = ref<Difficulty>('normal')
    const worldSeed = ref('')
    const worldTypeOption = ref('minecraft:normal')
    const generateStructures = ref(true)
    const generatorSettingsMode = ref<GeneratorSettingsMode>('default')
    const generatorSettingsCustom = ref('')

    // Instance-specific state
    const instanceName = ref('')
    const existingInstanceNames = ref<string[]>([])
    const fetchExistingInstanceNames = options.fetchExistingInstanceNames ?? null
    const instanceIcon = ref<File | null>(null)
    const instanceIconUrl = ref<string | null>(null)
    const instanceIconPath = ref<string | null>(null)

    // Game directory isolation (instance flow only): how the instance's game
    // working directory is laid out relative to a .minecraft root.
    // `gameDirOverride` holds the picked `.minecraft` root; the finished path
    // (root vs root/versions/<name>) is resolved at create time by mode.
    const gameDirOverrideMode = ref<GameDirOverrideMode>('builtin')
    const gameDirOverride = ref<string | null>(null)

    // Revoke old object URL when icon is cleared to avoid memory leaks
    watch(instanceIconUrl, (_newUrl, oldUrl) => {
        if (oldUrl && oldUrl.startsWith('blob:')) {
            URL.revokeObjectURL(oldUrl)
        }
    })

    const selectedLoader = ref<string | null>(null)
    const selectedGameVersion = ref<string | null>(null)
    const loaderVersionType = ref<LoaderVersionType>('stable')
    const selectedLoaderVersion = ref<string | null>(null)
    const selectedAdjuncts = ref<AdjunctLoader[]>([])
    const showSnapshots = ref(false)
    const loaderVersionsCache = ref<Record<string, LoaderManifest>>({})
    const loaderMetadataStatus = ref<Record<string, LoaderMetadataStatus>>({})
    const paperSupportedVersions = ref<Set<string> | null>(null)
    const purpurSupportedVersions = ref<Set<string> | null>(null)

    const autoInstanceName = computed(() => {
        const loader = selectedLoader.value
        const version = selectedGameVersion.value
        if (!version) return ''

        const baseName = defaultInstanceName(loader, version, selectedLoaderVersion.value)

        const names = new Set(existingInstanceNames.value)
        if (!names.has(baseName)) return baseName

        let counter = 1
        while (names.has(`${baseName} (${counter})`)) {
            counter++
        }
        return `${baseName} (${counter})`
    })

    const modpackSelection = ref<ModpackSelection | null>(null)
    const modpackFile = ref<File | null>(null)
    const modpackFilePath = ref<string | null>(null)

    // Modpack search state (persisted across stage navigation)
    const modpackSearchProjectId = ref<string | undefined>()
    const modpackSearchVersionId = ref<string | undefined>()
    const modpackSearchOptions = ref<ComboboxOption<string>[]>([])
    const modpackVersionOptions = ref<ComboboxOption<string>[]>([])
    const modpackSearchHits = ref<Record<string, ModpackSearchHit>>({})

    // Import state (instance flow only)
    const importLaunchers = ref<ImportableLauncher[]>([])
    const importSelectedInstances = ref<Record<string, Set<string>>>({})
    const importSearchQuery = ref('')
    const importAsSymlink = ref(false)

    const skipSetupType = ref(false)
    const loading = ref(false)

    // hideLoaderChips: hides the entire loader chips section (only for vanilla world type in world/server flows)
    const hideLoaderChips = computed(() => setupType.value === 'vanilla')

    // hideLoaderVersion: hides the loader version section (vanilla world type OR vanilla selected as loader chip)
    const hideLoaderVersion = computed(
        () => setupType.value === 'vanilla' || selectedLoader.value === 'vanilla',
    )

    function toApiLoaderName(loader: string): string {
        return loader === 'neoforge' ? 'neo' : loader
    }

    async function fetchLoaderManifest(loader: string, gameVersion?: string) {
        const apiLoader = toApiLoaderName(loader)
        const cacheKey = loaderMetadataCacheKey(apiLoader, gameVersion)
        loaderMetadataStatus.value[cacheKey] = 'loading'

        try {
            const data = await queryClient.fetchQuery({
                queryKey: loaderMetadataQueryKey(apiLoader, gameVersion),
                queryFn: async () =>
                    (await getLoaderManifest?.(apiLoader, gameVersion)) ??
                    (await client.launchermeta.manifest_v0.getManifest(apiLoader)),
                staleTime: LOADER_MANIFEST_STALE_TIME,
            })
            loaderVersionsCache.value[cacheKey] = data
            loaderMetadataStatus.value[cacheKey] = 'success'
            if (gameVersion) {
                debug(
                    'fetchLoaderManifest: support',
                    cacheKey,
                    loaderSupportState('success', data, gameVersion),
                )
            }
            debug(
                'fetchLoaderManifest: loaded',
                cacheKey,
                'gameVersions:',
                data.gameVersions.length,
            )
        } catch (error) {
            loaderMetadataStatus.value[cacheKey] = 'error'
            debug('fetchLoaderManifest: failed', cacheKey, error)
        }
    }

    async function fetchPaperSupportedVersions() {
        if (paperSupportedVersions.value) return
        loaderMetadataStatus.value.paper = 'loading'
        try {
            paperSupportedVersions.value = await queryClient.fetchQuery({
                queryKey: paperSupportedVersionsQueryKey,
                queryFn: async () => {
                    const project = await client.paper.versions_v3.getProject()
                    return new Set(Object.values(project.versions).flat())
                },
                staleTime: Infinity,
            })
            loaderMetadataStatus.value.paper = 'success'
        } catch {
            loaderMetadataStatus.value.paper = 'error'
        }
    }

    async function fetchPurpurSupportedVersions() {
        if (purpurSupportedVersions.value) return
        loaderMetadataStatus.value.purpur = 'loading'
        try {
            purpurSupportedVersions.value = await queryClient.fetchQuery({
                queryKey: purpurSupportedVersionsQueryKey,
                queryFn: async () => {
                    const project = await client.purpur.versions_v2.getProject()
                    return new Set(project.versions)
                },
                staleTime: Infinity,
            })
            loaderMetadataStatus.value.purpur = 'success'
        } catch {
            loaderMetadataStatus.value.purpur = 'error'
        }
    }

    async function fetchLoaderMetadata(loader?: string | null, gameVersion?: string) {
        if (!loader || loader === 'vanilla') return
        if (loader === 'paper') {
            await fetchPaperSupportedVersions()
            return
        }
        if (loader === 'purpur') {
            await fetchPurpurSupportedVersions()
            return
        }
        await fetchLoaderManifest(loader, gameVersion)
    }

    async function prefetchLoaderMetadata(gameVersion?: string) {
        await Promise.allSettled(
            availableLoaders
                .filter((loader) => loader !== 'vanilla')
                .map((loader) => fetchLoaderMetadata(loader, gameVersion)),
        )
    }

    async function reset() {
        if (fetchExistingInstanceNames) {
            existingInstanceNames.value = await fetchExistingInstanceNames()
        }
        setupType.value = null
        isImportMode.value = false
        worldCounter++
        worldName.value = flowType === 'world' ? `World ${worldCounter}` : ''
        gamemode.value = 'survival'
        difficulty.value = 'normal'
        worldSeed.value = ''
        worldTypeOption.value = 'minecraft:normal'
        generateStructures.value = true
        generatorSettingsMode.value = 'default'
        generatorSettingsCustom.value = ''

        // Instance-specific
        instanceName.value = ''
        instanceIconUrl.value = null
        instanceIcon.value = null
        instanceIconPath.value = null
        gameDirOverrideMode.value = 'builtin'
        gameDirOverride.value = null

        selectedLoader.value = null
        selectedGameVersion.value = null
        loaderVersionType.value = 'stable'
        selectedLoaderVersion.value = null
        selectedAdjuncts.value = []
        showSnapshots.value = false
        modpackSelection.value = null
        modpackFile.value = null
        modpackFilePath.value = null
        modpackSearchProjectId.value = undefined
        modpackSearchVersionId.value = undefined
        modpackSearchOptions.value = []
        modpackVersionOptions.value = []
        modpackSearchHits.value = {}

        // Import state
        importLaunchers.value = []
        importSelectedInstances.value = {}
        importSearchQuery.value = ''
        importAsSymlink.value = false

        loading.value = false
    }

    function setSetupType(type: SetupType) {
        debug('setSetupType:', type)
        isImportMode.value = false
        setupType.value = type
        if (type === 'modpack') {
            selectedLoader.value = null
            selectedLoaderVersion.value = null
            loaderVersionType.value = 'stable'
            modal.value?.setStage('import-instance')
        } else {
            modpackSelection.value = null
            modpackFile.value = null
            modpackFilePath.value = null
            if (type === 'vanilla') {
                selectedLoader.value = null
                selectedLoaderVersion.value = null
                loaderVersionType.value = 'stable'
            }
            // both custom and vanilla go to custom-setup
            // vanilla just hides loader chips via hideLoaderChips computed
            modal.value?.setStage('custom-setup')
        }
    }

    function setImportMode() {
        isImportMode.value = true
        setupType.value = null
        modal.value?.setStage('import-instance')
    }

    function browseModpacks() {
        modal.value?.hide()
        emit.browseModpacks()
    }

    function finish() {
        if (finishDisabled.value) return

        debug('finish() called, state:', {
            setupType: setupType.value,
            selectedLoader: selectedLoader.value,
            selectedGameVersion: selectedGameVersion.value,
            selectedLoaderVersion: selectedLoaderVersion.value,
            modpackSelection: modpackSelection.value,
            hasModpackFile: !!modpackFile.value,
        })
        loading.value = true
        emit.create(contextValue)
    }

    const resolvedStageConfigs = disableClose
        ? stageConfigs.map((stage) => ({ ...stage, disableClose: true }))
        : stageConfigs

    const contextValue: CreationFlowContextValue = {
        flowType,
        formatMessage,
        availableLoaders,
        showSnapshotToggle,
        disableClose,
        initialLoader,
        initialGameVersion,
        setupType,
        isImportMode,
        worldName,
        gamemode,
        difficulty,
        worldSeed,
        worldTypeOption,
        generateStructures,
        generatorSettingsMode,
        generatorSettingsCustom,
        instanceName,
        autoInstanceName,
        instanceIcon,
        instanceIconUrl,
        instanceIconPath,
        gameDirOverrideMode,
        gameDirOverride,
        selectedLoader,
        selectedGameVersion,
        loaderVersionType,
        selectedLoaderVersion,
        selectedAdjuncts,
        hideLoaderChips,
        hideLoaderVersion,
        showSnapshots,
        loaderVersionsCache,
        loaderMetadataStatus,
        paperSupportedVersions,
        purpurSupportedVersions,
        modpackSelection,
        modpackFile,
        modpackFilePath,
        modpackSearchProjectId,
        modpackSearchVersionId,
        modpackSearchOptions,
        modpackVersionOptions,
        modpackSearchHits,
        importLaunchers,
        importSelectedInstances,
        importSearchQuery,
        importAsSymlink,
        skipSetupType,
        loading,
        finishDisabled,
        finishDisabledTooltip,
        modal,
        stageConfigs: resolvedStageConfigs,
        onBack,
        onImportFileReceived,
        reset,
        setSetupType,
        setImportMode,
        browseModpacks,
        finish,
        fetchLoaderMetadata,
        prefetchLoaderMetadata,
        searchModpacks,
        getProjectVersions,
        hasCompatibleOptiFabric,
        getLoaderManifest,
    }

    return contextValue
}
