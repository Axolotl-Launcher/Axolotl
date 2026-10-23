import { RefreshCwIcon } from '@modrinth/assets'
import {
    isServerTypeSupported,
    requiredJavaMajorVersion,
    SERVER_TYPES,
    type ServerTypeId,
    setEulaAccepted,
} from '@modrinth/server'
import {
    createContext,
    defineMessages,
    type MultiStageModal,
    type StageConfigInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, markRaw, type Ref, ref } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import {
    javaMajorFromVersion,
    toErrorMessage,
} from '@/components/multiplayer/servers/server-flow-utils'
import { getServerInstallStrategy, runServerInstall } from '@/composables/server-install'
import { refresh as refreshServerList } from '@/composables/useServers'
import { find_filtered_jres, get_java_default_versions, get_max_memory } from '@/helpers/jre'
import { get_game_versions, get_loader_versions } from '@/helpers/metadata'
import { type ServerManifestData, servers } from '@/helpers/servers'
import { injectDownloadManager } from '@/providers/download-manager'

import InstallStage from './stages/InstallStage.vue'
import SetupStage from './stages/SetupStage.vue'
import TypeStage from './stages/TypeStage.vue'

export type InstallPhase =
    'idle' | 'preparing' | 'downloading' | 'first-run' | 'eula' | 'error' | 'done'

export interface JavaSelection {
    path: string
    version: string
}

export interface LoaderVersionOption {
    id: string
    stable: boolean
}

export interface CreateServerFlowContext<TCtx extends CreateServerFlowContext<TCtx>> {
    modal: Ref<ComponentExposed<typeof MultiStageModal> | null>
    stageConfigs: StageConfigInput<TCtx>[]
    formatMessage: ReturnType<typeof useVIntl>['formatMessage']

    serverType: Ref<ServerTypeId>
    availableGameVersions: Ref<string[]>
    selectedGameVersion: Ref<string>
    showSnapshots: Ref<boolean>
    loaderVersions: Ref<LoaderVersionOption[]>
    selectedLoaderVersion: Ref<string>
    isVersionsLoading: Ref<boolean>
    versionsError: Ref<string | null>

    name: Ref<string>
    selectedJava: Ref<JavaSelection>
    memoryMb: Ref<number>
    maxMemoryMb: Ref<number>

    installPhase: Ref<InstallPhase>
    downloadProgress: Ref<{ downloaded: number; total: number | null } | null>
    installLog: Ref<string[]>
    installError: Ref<string | null>
    eulaText: Ref<string>
    createdServer: Ref<ServerManifestData | null>
    showEulaModal: Ref<boolean>

    /** Registered by the configure stage to persist server.properties before finishing. */
    saveServerProperties: Ref<(() => Promise<boolean>) | null>

    needsLoaderVersion: Ref<boolean>
    typeSupported: Ref<boolean>
    canContinueFromType: Ref<boolean>

    loadVersions: () => Promise<void>
    loadLoaderVersions: () => Promise<void>
    loadDefaultJava: () => Promise<void>
    beginInstall: () => Promise<void>
    retryInstall: () => Promise<void>
    acceptEula: () => Promise<void>
    declineEula: () => void
    reset: () => void
}

/** Concrete context used by the vanilla (non-modpack) server creation flow. */
export type CreateServerFlowContextValue = CreateServerFlowContext<CreateServerFlowContextValue>

export const [injectCreateServerFlow, provideCreateServerFlow] =
    createContext<CreateServerFlowContextValue>('CreateServerFlow')

export function createCreateServerFlowContext(
    modal: Ref<ComponentExposed<typeof MultiStageModal> | null>,
): CreateServerFlowContextValue {
    const { formatMessage } = useVIntl()

    // [SERVER-DOWNLOAD-BRIDGE] Capture the download manager once during Vue
    // setup context.  Vue's inject() only works in the synchronous setup
    // scope — after any `await` the injection context is lost.  We store the
    // reference here and pass it explicitly to the shared download bridge so
    // the vanilla server download appears in the sidebar like the modpack flow.
    let downloadManager: ReturnType<typeof injectDownloadManager> | null = null
    try {
        downloadManager = injectDownloadManager()
    } catch {
        // Not inside a provider tree — server downloads will not appear in sidebar.
    }

    const wizardMessages = defineMessages({
        typeStageTitle: { id: 'app.servers.wizard.type-title', defaultMessage: 'Server type' },
        setupStageTitle: { id: 'app.servers.wizard.setup-title', defaultMessage: 'Setup' },
        installStageTitle: { id: 'app.servers.wizard.install-title', defaultMessage: 'Install' },
        configureStageTitle: {
            id: 'app.servers.wizard.configure-title',
            defaultMessage: 'Configure',
        },
        next: { id: 'app.servers.wizard.next', defaultMessage: 'Next' },
        retry: { id: 'app.servers.wizard.retry', defaultMessage: 'Retry' },
        finish: { id: 'app.servers.wizard.finish', defaultMessage: 'Finish' },
        javaTooOld: {
            id: 'app.servers.wizard.java-too-old',
            defaultMessage:
                'Java {selected} cannot run this game version; Java {required} or newer is required.',
        },
    })

    const serverType = ref<ServerTypeId>('vanilla')
    const availableGameVersions = ref<string[]>([])
    const selectedGameVersion = ref('')
    const showSnapshots = ref(false)
    const loaderVersions = ref<LoaderVersionOption[]>([])
    const selectedLoaderVersion = ref('')
    const isVersionsLoading = ref(false)
    const versionsError = ref<string | null>(null)

    const name = ref('')
    const selectedJava = ref<JavaSelection>({ path: '', version: '' })
    const memoryMb = ref(2048)
    const maxMemoryMb = ref(8192)

    const installPhase = ref<InstallPhase>('idle')
    const downloadProgress = ref<{ downloaded: number; total: number | null } | null>(null)
    const installLog = ref<string[]>([])
    const installError = ref<string | null>(null)
    const eulaText = ref('')
    const createdServer = ref<ServerManifestData | null>(null)
    const showEulaModal = ref(false)
    const saveServerProperties = ref<(() => Promise<boolean>) | null>(null)

    const needsLoaderVersion = computed(
        () => SERVER_TYPES[serverType.value]?.needsLoaderVersion ?? false,
    )
    const typeSupported = computed(() => isServerTypeSupported(serverType.value))

    async function loadVersions() {
        isVersionsLoading.value = true
        versionsError.value = null
        try {
            const manifest = (await get_game_versions()) as {
                latest: { release: string }
                versions: { id: string; type: string; url: string }[]
            }
            const all = manifest.versions
            availableGameVersions.value = all
                .filter((entry) => (showSnapshots.value ? true : entry.type === 'release'))
                .map((entry) => entry.id)
            if (!availableGameVersions.value.includes(selectedGameVersion.value)) {
                selectedGameVersion.value =
                    manifest.latest.release &&
                    availableGameVersions.value.includes(manifest.latest.release)
                        ? manifest.latest.release
                        : availableGameVersions.value[0]
            }
            await loadLoaderVersions()
        } catch (error) {
            versionsError.value = toErrorMessage(error)
        } finally {
            isVersionsLoading.value = false
        }
    }

    async function loadLoaderVersions() {
        selectedLoaderVersion.value = ''
        loaderVersions.value = []
        if (serverType.value !== 'fabric' || !selectedGameVersion.value) return
        try {
            const manifest = (await get_loader_versions('fabric', selectedGameVersion.value)) as {
                gameVersions: Array<{ id: string; loaders: LoaderVersionOption[] }>
            }
            const entry = manifest.gameVersions.find(
                (game) => game.id === selectedGameVersion.value,
            )
            loaderVersions.value = entry?.loaders ?? []
            selectedLoaderVersion.value = loaderVersions.value[0]?.id ?? ''
        } catch {
            loaderVersions.value = []
        }
    }

    /** Prefills the Java path from the instance-level defaults, falling back to a scan. */
    async function loadDefaultJava() {
        if (selectedJava.value.path !== '') return
        const major = requiredJavaMajorVersion(selectedGameVersion.value || '1.21')
        try {
            const defaults = (await get_java_default_versions()) as Array<{
                parsed_version: number
                version: string
                path: string
            }>
            const match =
                defaults.find((entry) => entry.parsed_version === major) ??
                defaults.find((entry) => entry.parsed_version >= major)
            if (match) {
                selectedJava.value = { path: match.path, version: match.version }
                return
            }
        } catch {
            // Fall through to a filtered scan
        }
        try {
            const javas = (await find_filtered_jres(major)) as JavaSelection[]
            if (javas.length > 0) selectedJava.value = javas[0]
        } catch {
            // Leave empty; the user picks manually in the setup stage
        }
    }

    async function loadMaxMemory() {
        try {
            const maxKiB = (await get_max_memory()) as number
            maxMemoryMb.value = Math.max(1024, Math.floor(maxKiB / 1024))
        } catch {
            maxMemoryMb.value = 8192
        }
    }

    async function beginInstall() {
        if (installPhase.value === 'downloading' || installPhase.value === 'first-run') return
        installPhase.value = 'preparing'
        installError.value = null
        installLog.value = []
        downloadProgress.value = null
        try {
            const requiredJava = requiredJavaMajorVersion(selectedGameVersion.value)
            const selectedMajor = javaMajorFromVersion(selectedJava.value.version)
            if (
                selectedJava.value.path !== '' &&
                selectedMajor !== null &&
                selectedMajor < requiredJava
            ) {
                throw new Error(
                    formatMessage(wizardMessages.javaTooOld, {
                        selected: selectedMajor,
                        required: requiredJava,
                    }),
                )
            }
            const manifest = await servers.create({
                name: name.value,
                serverType: serverType.value,
                gameVersion: selectedGameVersion.value,
                loaderVersion:
                    serverType.value === 'fabric' ? selectedLoaderVersion.value : undefined,
                javaPath: selectedJava.value.path || undefined,
                memoryMb: memoryMb.value,
            })
            createdServer.value = manifest

            // [SERVER-INSTALL] The shared orchestrator owns the sidebar download
            // job, progress/log event forwarding, and cancellation. Each server
            // type supplies a `ServerInstallStrategy` that knows how to obtain its
            // launcher files; vanilla/Fabric/Paper download a jar, Forge runs its
            // installer. This is the single reuse point for every server type.
            const strategy = getServerInstallStrategy(serverType.value)
            await runServerInstall({
                serverId: manifest.id,
                name: name.value,
                inputs: {
                    gameVersion: selectedGameVersion.value,
                    loaderVersion: selectedLoaderVersion.value || undefined,
                    javaPath: selectedJava.value.path || undefined,
                    memoryMb: memoryMb.value,
                },
                strategy,
                downloadManager,
                onProgress: (progress) => {
                    downloadProgress.value = progress
                },
                onLog: (line) => {
                    installLog.value.push(line)
                    if (installLog.value.length > 500) {
                        installLog.value.splice(0, installLog.value.length - 500)
                    }
                },
            })

            // [SERVER-EULA] Like the modpack flow, the server is not auto-started.
            // A code-created `eula.txt` (eula=false) is written so the manual start
            // gate (useServerLifecycle) can offer the EULA without booting the jar.
            const eula = setEulaAccepted('', false)
            await servers.writeFile(manifest.id, 'eula.txt', eula).catch(() => {})
            installPhase.value = 'done'
        } catch (error) {
            installPhase.value = 'error'
            installError.value = toErrorMessage(error)
            // A half-installed server must not linger in the list; retrying starts over.
            if (createdServer.value) {
                const failed = createdServer.value
                createdServer.value = null
                await servers.delete(failed.id).catch(() => {})
                void refreshServerList()
            }
        }
    }

    function retryInstall(): Promise<void> {
        installPhase.value = 'idle'
        return beginInstall()
    }

    async function acceptEula() {
        if (!createdServer.value) return
        try {
            const updated = setEulaAccepted(eulaText.value, true)
            await servers.writeFile(createdServer.value.id, 'eula.txt', updated)
            showEulaModal.value = false
            installPhase.value = 'done'
        } catch (error) {
            installError.value = toErrorMessage(error)
            installPhase.value = 'error'
            showEulaModal.value = false
        }
    }

    function declineEula() {
        showEulaModal.value = false
        modal.value?.hide()
    }

    function reset() {
        serverType.value = 'vanilla'
        selectedGameVersion.value = ''
        selectedLoaderVersion.value = ''
        loaderVersions.value = []
        name.value = ''
        selectedJava.value = { path: '', version: '' }
        memoryMb.value = 2048
        installPhase.value = 'idle'
        installLog.value = []
        installError.value = null
        downloadProgress.value = null
        eulaText.value = ''
        createdServer.value = null
        showEulaModal.value = false
        saveServerProperties.value = null
        void loadVersions()
        void loadMaxMemory()
    }

    const canContinueFromType = computed(
        () =>
            typeSupported.value &&
            selectedGameVersion.value !== '' &&
            (!needsLoaderVersion.value || selectedLoaderVersion.value !== ''),
    )

    const stageConfigs: StageConfigInput<CreateServerFlowContextValue>[] = [
        {
            id: 'type',
            stageContent: markRaw(TypeStage),
            title: (ctx) => ctx.formatMessage(wizardMessages.typeStageTitle),
            cannotNavigateForward: (ctx) => !ctx.canContinueFromType.value,
            leftButtonConfig: () => null,
            rightButtonConfig: (ctx) => ({
                label: ctx.formatMessage(wizardMessages.next),
                color: 'brand',
                disabled: !ctx.canContinueFromType.value,
                onClick: () => ctx.modal.value?.nextStage(),
            }),
        },
        {
            id: 'setup',
            stageContent: markRaw(SetupStage),
            title: (ctx) => ctx.formatMessage(wizardMessages.setupStageTitle),
            cannotNavigateForward: (ctx) => ctx.name.value.trim() === '',
            leftButtonConfig: () => null,
            rightButtonConfig: (ctx) => ({
                label: ctx.formatMessage(wizardMessages.next),
                color: 'brand',
                disabled: ctx.name.value.trim() === '',
                onClick: async () => {
                    await ctx.loadDefaultJava()
                    ctx.modal.value?.nextStage()
                },
            }),
        },
        {
            id: 'install',
            stageContent: markRaw(InstallStage),
            title: (ctx) => ctx.formatMessage(wizardMessages.installStageTitle),
            cannotNavigateForward: (ctx) => ctx.installPhase.value !== 'done',
            // Downloads continue in the background once the wizard closes; only
            // the first-run boot locks closing until the server reaches its EULA gate.
            disableClose: (ctx) => ctx.installPhase.value === 'first-run',
            leftButtonConfig: () => null,
            rightButtonConfig: (ctx) => ({
                label: ctx.formatMessage(
                    ctx.installPhase.value === 'error'
                        ? wizardMessages.retry
                        : wizardMessages.finish,
                ),
                color: 'brand',
                icon: ctx.installPhase.value === 'error' ? RefreshCwIcon : null,
                iconPosition: 'after',
                disabled: ctx.installPhase.value !== 'done' && ctx.installPhase.value !== 'error',
                onClick: () => {
                    if (ctx.installPhase.value === 'error') {
                        ctx.retryInstall()
                        return
                    }
                    // Server is ready — close the wizard so the host can navigate to it.
                    ctx.modal.value?.hide()
                },
            }),
        },
    ]

    return {
        modal,
        stageConfigs,
        formatMessage,
        serverType,
        availableGameVersions,
        selectedGameVersion,
        showSnapshots,
        loaderVersions,
        selectedLoaderVersion,
        isVersionsLoading,
        versionsError,
        name,
        selectedJava,
        memoryMb,
        maxMemoryMb,
        installPhase,
        downloadProgress,
        installLog,
        installError,
        eulaText,
        createdServer,
        showEulaModal,
        saveServerProperties,
        needsLoaderVersion,
        typeSupported,
        canContinueFromType,
        loadVersions,
        loadLoaderVersions,
        loadDefaultJava,
        beginInstall,
        retryInstall,
        acceptEula,
        declineEula,
        reset,
    }
}
