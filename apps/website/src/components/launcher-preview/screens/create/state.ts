import { defaultInstanceName } from '@modrinth/ui/src/utils/loaders.ts'
import { computed, type ComputedRef, type Ref, ref, shallowRef, watch } from 'vue'

import type { PreviewState } from '../../lib/state'
import { usePreviewDomain } from '../../lib/state'
import type { GameInstance, InstanceLoader, LoaderComponent } from '../../lib/types'
import { getLoaderInstanceIconUrl } from './icons'
import { createImportFlow, type ImportFlow } from './import-flow'
import { GAME_FILES, runInstallJob } from './jobs'
import {
    clientInstallableLoaders,
    type GameVersionTag,
    type LoaderManifest,
    loaderMetadataCacheKey,
    type LoaderMetadataStatus,
    loadGameVersions,
    loadLoaderManifests,
    toApiLoaderName,
} from './metadata'

export type SetupType = 'modpack' | 'custom' | 'vanilla'
export type LoaderVersionType = 'stable' | 'latest' | 'other'
export type AdjunctLoader = 'optifine' | 'lite_loader'
export type GameDirOverrideMode = 'builtin' | 'isolated' | 'not-isolated'
export type CreationStageId = 'setup-type' | 'import-instance' | 'custom-setup'

/** `PickedFile` of the file-picker provider, as the icon picker returns it. */
export interface PickedIcon {
    file: Blob | null
    path: string | null
    previewUrl: string
    frameless?: boolean
}

/** The instance icon picker modal (`filePicker.pickInstanceIcon`). */
export interface IconPickerApi {
    show: () => Promise<PickedIcon | null>
}

export interface CreationModalApi {
    show: () => void
    hide: () => void
    setStage: (stage: number | CreationStageId) => void
}

export interface ShowCreationOptions {
    skipSetupType?: boolean
    initialMode?: 'custom' | 'import'
    onBack?: () => void
}

/** A `.minecraft` root configured for direct links (`axolotl-minecraft-directories`). */
export interface ConfiguredMinecraftDirectory {
    path: string
    mode: 'isolated' | 'shared' | 'automatic'
}

/**
 * The creation flow's context (`createCreationFlowContext` of
 * `packages/ui/src/components/flows/creation-flow-modal/creation-flow-context.ts`)
 * for the instance flow, which is the only flow the launcher's `/create` page opens.
 */
function createCreationFlow(preview: PreviewState, onCreate: () => void) {
    const availableLoaders: string[] = [...clientInstallableLoaders]

    const setupType = ref<SetupType | null>(null)
    const isImportMode = ref(false)

    // Instance-specific state
    const instanceName = ref('')
    const existingInstanceNames = ref<string[]>([])
    const instanceIcon = shallowRef<Blob | null>(null)
    const instanceIconUrl = ref<string | null>(null)
    const instanceIconPath = ref<string | null>(null)
    const gameDirOverrideMode = ref<GameDirOverrideMode>('builtin')
    const gameDirOverride = ref<string | null>(null)

    // Revoke old object URL when the icon is cleared.
    watch(instanceIconUrl, (_next, previous) => {
        if (previous && previous.startsWith('blob:')) URL.revokeObjectURL(previous)
    })

    const selectedLoader = ref<string | null>(null)
    const selectedGameVersion = ref<string | null>(null)
    const loaderVersionType = ref<LoaderVersionType>('stable')
    const selectedLoaderVersion = ref<string | null>(null)
    const selectedAdjuncts = ref<AdjunctLoader[]>([])
    const loaderVersionsCache = ref<Record<string, LoaderManifest>>({})
    const loaderMetadataStatus = ref<Record<string, LoaderMetadataStatus>>({})

    const skipSetupType = ref(false)
    const loading = ref(false)
    let onBack: (() => void) | null = null

    const modal = shallowRef<CreationModalApi | null>(null)

    const autoInstanceName = computed(() => {
        const version = selectedGameVersion.value
        if (!version) return ''
        const baseName = defaultInstanceName(
            selectedLoader.value,
            version,
            selectedLoaderVersion.value,
        )
        const names = new Set(existingInstanceNames.value)
        if (!names.has(baseName)) return baseName
        let counter = 1
        while (names.has(`${baseName} (${counter})`)) counter++
        return `${baseName} (${counter})`
    })

    const hideLoaderChips = computed(() => setupType.value === 'vanilla')
    const hideLoaderVersion = computed(
        () => setupType.value === 'vanilla' || selectedLoader.value === 'vanilla',
    )

    // `queryClient.fetchQuery` with a 30 min stale time: the first request of a
    // key waits for the metadata endpoint, later requests resolve from cache.
    const fetchedKeys = new Set<string>()

    function delay(ms: number) {
        return new Promise<void>((resolve) => preview.clock.after(ms, resolve))
    }

    async function fetchLoaderManifest(loader: string, gameVersion?: string) {
        const apiLoader = toApiLoaderName(loader)
        const cacheKey = loaderMetadataCacheKey(apiLoader, gameVersion)
        loaderMetadataStatus.value[cacheKey] = 'loading'
        try {
            const manifests = await loadLoaderManifests()
            if (!fetchedKeys.has(cacheKey)) {
                await delay(240 + ((cacheKey.length * 37) % 180))
                fetchedKeys.add(cacheKey)
            }
            loaderVersionsCache.value[cacheKey] = manifests[apiLoader] ?? { gameVersions: [] }
            loaderMetadataStatus.value[cacheKey] = 'success'
        } catch {
            loaderMetadataStatus.value[cacheKey] = 'error'
        }
    }

    async function fetchLoaderMetadata(loader?: string | null, gameVersion?: string) {
        if (!loader || loader === 'vanilla') return
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
        existingInstanceNames.value = preview.instances.instances.value.map((entry) => entry.name)
        setupType.value = null
        isImportMode.value = false

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
        loading.value = false
    }

    function setSetupType(type: SetupType) {
        isImportMode.value = false
        setupType.value = type
        if (type === 'modpack') {
            selectedLoader.value = null
            selectedLoaderVersion.value = null
            loaderVersionType.value = 'stable'
            modal.value?.setStage('import-instance')
        } else {
            if (type === 'vanilla') {
                selectedLoader.value = null
                selectedLoaderVersion.value = null
                loaderVersionType.value = 'stable'
            }
            modal.value?.setStage('custom-setup')
        }
    }

    function setImportMode() {
        isImportMode.value = true
        setupType.value = null
        modal.value?.setStage('import-instance')
    }

    function finish() {
        loading.value = true
        onCreate()
    }

    /** `CreationFlowModal.show` (creation-flow-modal/index.vue). */
    async function show(options?: ShowCreationOptions) {
        skipSetupType.value = options?.skipSetupType ?? false
        onBack = options?.onBack ?? null
        await reset()
        void prefetchLoaderMetadata()
        if (options?.skipSetupType) {
            if (options.initialMode === 'import') setImportMode()
            else setSetupType('custom')
        } else {
            modal.value?.setStage(0)
        }
        modal.value?.show()
    }

    function hide() {
        modal.value?.hide()
    }

    function back() {
        modal.value?.hide()
        onBack?.()
    }

    return {
        availableLoaders,
        setupType,
        isImportMode,
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
        loaderVersionsCache,
        loaderMetadataStatus,
        skipSetupType,
        loading,
        modal,
        reset,
        setSetupType,
        setImportMode,
        finish,
        fetchLoaderMetadata,
        prefetchLoaderMetadata,
        /** OptiFabric compatibility comes from CurseForge, which the snapshot does not hold. */
        hasCompatibleOptiFabric: async (_gameVersion: string) => false,
        show,
        hide,
        back,
    }
}

export type CreationFlow = ReturnType<typeof createCreationFlow>

/** The Mac's path join (`@tauri-apps/api/path` join). */
function joinPath(...parts: string[]) {
    return parts.join('/').replace(/\/{2,}/g, '/')
}

function createCreateArea(preview: PreviewState) {
    const gameVersions = ref<GameVersionTag[]>([])
    const gameVersionsLoaded = ref(false)
    let dataPromise: Promise<void> | null = null

    /** The tag cache the app preloads at startup (`injectTags().gameVersions`). */
    function ensureData() {
        if (!dataPromise) {
            dataPromise = Promise.all([loadGameVersions(), loadLoaderManifests()]).then(
                ([versions]) => {
                    gameVersions.value = versions
                    gameVersionsLoaded.value = true
                },
            )
        }
        return dataPromise
    }

    const iconPicker = shallowRef<IconPickerApi | null>(null)
    /** The `skip_unknown_pack_warning` feature flag ("Don't show this warning again"). */
    const skipUnknownPackWarning = ref(false)

    /** `.minecraft` roots added by the gamedir import flow this session. */
    const importedMinecraftDirectories = ref<ConfiguredMinecraftDirectory[]>([])

    /**
     * Configured Minecraft directories (`loadMinecraftDirectories` of
     * `CustomSetupStage.vue`): Settings → Storage keeps them; roots imported through
     * the gamedir flow are added to the same list.
     */
    const configuredMinecraftDirectories = computed<ConfiguredMinecraftDirectory[]>(() => {
        const settingsDomain = preview.domains.get('settings')?.value as
            { extras?: { minecraft_directories?: { path: string; mode: string }[] } } | undefined
        const fromSettings = Array.isArray(settingsDomain?.extras?.minecraft_directories)
            ? settingsDomain.extras.minecraft_directories
            : []
        const merged = [
            ...fromSettings.map((entry) => ({
                path: entry.path,
                mode: (entry.mode === 'shared' ? 'shared' : 'isolated') as 'shared' | 'isolated',
            })),
            ...importedMinecraftDirectories.value,
        ].filter((entry) => typeof entry.path === 'string' && entry.path.trim())
        return [...new Map(merged.map((entry) => [entry.path, entry])).values()]
    })

    let importFlow: ImportFlow | null = null

    const flow = createCreationFlow(preview, () => {
        void handleCreate()
    })

    /** `handleCreate` of `providers/setup/creation-modal.ts`, custom-setup branch. */
    async function handleCreate() {
        flow.hide()

        if (flow.isImportMode.value) return

        const loader = flow.hideLoaderChips.value
            ? 'vanilla'
            : (flow.selectedLoader.value ?? 'vanilla')
        const concreteLoaderVersion = flow.hideLoaderVersion.value
            ? null
            : flow.selectedLoaderVersion.value
        const iconPath = flow.instanceIconPath.value ?? null
        const name = flow.instanceName.value.trim() || flow.autoInstanceName.value
        const mode = flow.gameDirOverrideMode.value
        const gameRoot = flow.gameDirOverride.value ?? null
        const gameDirOverride =
            mode === 'builtin'
                ? null
                : gameRoot
                  ? mode === 'isolated'
                      ? joinPath(gameRoot, 'versions', name)
                      : gameRoot
                  : null
        const gameVersion = flow.selectedGameVersion.value
        if (!gameVersion) return

        installCreateInstance({
            name,
            gameVersion,
            loader: loader as InstanceLoader,
            loaderVersion: concreteLoaderVersion,
            adjuncts: [...flow.selectedAdjuncts.value],
            iconPath,
            gameDirOverride,
        })
    }

    /**
     * `install_create_instance`: the instance record exists as soon as the job starts
     * (installing), the job downloads the game and installs the loader, and the record
     * becomes playable when the job completes.
     */
    function installCreateInstance(request: {
        name: string
        gameVersion: string
        loader: InstanceLoader
        loaderVersion: string | null
        adjuncts: AdjunctLoader[]
        iconPath: string | null
        gameDirOverride: string | null
    }): GameInstance {
        const created = preview.instances.create({
            name: request.name,
            gameVersion: request.gameVersion,
            loader: request.loader,
            loaderVersion: request.loaderVersion,
            iconPath: request.iconPath,
        })
        const instance = preview.instances.get(created.id) ?? created
        instance.install_stage = 'minecraft_installing'
        if (request.gameDirOverride) instance.path = request.gameDirOverride
        if (request.adjuncts.length > 0) {
            instance.loader_components = [
                ...instance.loader_components,
                ...request.adjuncts.map<LoaderComponent>((kind) => ({
                    instanceId: instance.id,
                    kind,
                    version: null,
                    role: 'adjunct',
                })),
            ]
        }
        preview.downloads.contents.value = {
            ...preview.downloads.contents.value,
            [instance.id]: [],
        }

        void runInstallJob(preview, {
            title: request.name,
            icon: request.iconPath ?? getLoaderInstanceIconUrl(request.loader),
            projectType: 'minecraft',
            instanceId: instance.id,
            instanceName: request.name,
            versionName: request.gameVersion,
            fileName: `${request.gameVersion}.jar`,
            downloadBytes: GAME_FILES.bytes,
        }).then((result) => {
            const current = preview.instances.get(instance.id)
            if (!current) return
            current.install_stage = result === 'completed' ? 'installed' : 'not_installed'
        })
        return instance
    }

    function getImportFlow(): ImportFlow {
        if (!importFlow) {
            importFlow = createImportFlow(preview, {
                ensureData,
                gameVersions,
                addMinecraftDirectories(roots) {
                    const merged = [...importedMinecraftDirectories.value, ...roots]
                    importedMinecraftDirectories.value = [
                        ...new Map(merged.map((entry) => [entry.path, entry])).values(),
                    ]
                },
                hideCreationModal: () => flow.hide(),
            })
        }
        return importFlow
    }

    return {
        flow,
        iconPicker,
        skipUnknownPackWarning,
        gameVersions,
        gameVersionsLoaded,
        ensureData,
        configuredMinecraftDirectories,
        installCreateInstance,
        get importFlow() {
            return getImportFlow()
        },
        dispose() {
            importFlow?.dispose()
        },
    }
}

export type CreateArea = ReturnType<typeof createCreateArea>

/** The area's state, created once per preview window and kept across navigation. */
export function useCreateArea(): CreateArea {
    return usePreviewDomain('create', (state) => {
        const area = createCreateArea(state)
        return { value: area, dispose: area.dispose }
    })
}

export type { ComputedRef, Ref }
