import type { Labrinth } from '@modrinth/api-client'
import { RefreshCwIcon } from '@modrinth/assets'
import { type ServerTypeId, setEulaAccepted } from '@modrinth/server'
import {
    createContext,
    defineMessages,
    type MultiStageModal,
    type StageConfigInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, markRaw, type Ref, ref } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { startModpackServerInstall } from '@/composables/useServerInstalls'
import { refresh as refreshServerList } from '@/composables/useServers'
import { find_filtered_jres, get_java_default_versions, get_max_memory } from '@/helpers/jre'
import { get_loader_versions } from '@/helpers/metadata'
import { serverEventListener, type ServerManifestData, servers } from '@/helpers/servers'
import { injectDownloadManager } from '@/providers/download-manager'

import type { CreateServerFlowContext, JavaSelection } from '../create-server-flow'
import {
    javaMajorFromVersion,
    resolveServerLauncher,
    toErrorMessage,
    waitForServerStop,
} from '../server-flow-utils'
import ModpackInstallStage from './stages/ModpackInstallStage.vue'
import ModpackSetupStage from './stages/ModpackSetupStage.vue'

export type ModpackInstallPhase =
    'idle' | 'preparing' | 'downloading' | 'first-run' | 'eula' | 'error' | 'done'

export interface ModpackServerOptions {
    project: Labrinth.Projects.v2.Project
    version: Labrinth.Versions.v2.Version
}

export interface ModpackServerFlowContext extends CreateServerFlowContext<ModpackServerFlowContext> {
    modpackTitle: Ref<string>
    modpackVersionNumber: Ref<string>
    modpackIconUrl: Ref<string | undefined>
    loaderLabel: Ref<string>
    loaderSupported: Ref<boolean>
    gameVersionLabel: Ref<string>
    setPack: (project: Labrinth.Projects.v2.Project, version: Labrinth.Versions.v2.Version) => void
}

export const [injectModpackServerFlow, provideModpackServerFlow] =
    createContext<ModpackServerFlowContext>('ModpackServerFlow')

const MODPACK_SERVER_TYPES: Record<string, { type: ServerTypeId; label: string }> = {
    fabric: { type: 'fabric', label: 'Fabric' },
    quilt: { type: 'quilt', label: 'Quilt' },
    neoforge: { type: 'neoforge', label: 'NeoForge' },
    forge: { type: 'forge', label: 'Forge' },
}

/** Loaders whose server launcher the app can download and boot directly. */
const SUPPORTED_MODPACK_LOADERS: ServerTypeId[] = ['vanilla', 'fabric', 'quilt', 'forge']

export function resolveModpackLoader(loaders: string[]): { type: ServerTypeId; label: string } {
    for (const loader of loaders) {
        const entry = MODPACK_SERVER_TYPES[loader.toLowerCase()]
        if (entry) return entry
    }
    return { type: 'vanilla', label: 'Vanilla' }
}

export function createModpackServerFlowContext(
    modal: Ref<ComponentExposed<typeof MultiStageModal> | null>,
): ModpackServerFlowContext {
    const { formatMessage } = useVIntl()

    // [SERVER-DOWNLOAD-BRIDGE] Capture the download manager once during Vue
    // setup context.  Vue's inject() only works in the synchronous setup
    // scope — after any `await` the injection context is lost.  We store
    // the reference here and pass it explicitly to `startModpackServerInstall`.
    let downloadManager: ReturnType<typeof injectDownloadManager> | null = null
    try {
        downloadManager = injectDownloadManager()
    } catch {
        // Not inside a provider tree — server downloads will not appear in sidebar.
    }

    const wizardMessages = defineMessages({
        setupTitle: { id: 'app.servers.wizard.setup-title', defaultMessage: 'Setup' },
        installTitle: { id: 'app.servers.wizard.install-title', defaultMessage: 'Install' },
        configureTitle: { id: 'app.servers.wizard.configure-title', defaultMessage: 'Configure' },
        next: { id: 'app.servers.wizard.next', defaultMessage: 'Next' },
        retry: { id: 'app.servers.wizard.retry', defaultMessage: 'Retry' },
        finish: { id: 'app.servers.wizard.finish', defaultMessage: 'Finish' },
        javaTooOld: {
            id: 'app.servers.wizard.java-too-old',
            defaultMessage:
                'Java {selected} cannot run this game version; Java {required} or newer is required.',
        },
        firstRunCrashed: {
            id: 'app.servers.modpack.first-run-crashed',
            defaultMessage:
                'The server crashed during its first start. Check that your selected Java version is compatible, then try again.',
        },
    })

    const project = ref<Labrinth.Projects.v2.Project | null>(null)
    const version = ref<Labrinth.Versions.v2.Version | null>(null)

    const serverType = ref<ServerTypeId>('vanilla')
    const availableGameVersions = ref<string[]>([])
    const selectedGameVersion = ref('')
    const showSnapshots = ref(false)
    const loaderVersions = ref<{ id: string; stable: boolean }[]>([])
    const selectedLoaderVersion = ref('')
    const isVersionsLoading = ref(false)
    const versionsError = ref<string | null>(null)

    const name = ref('')
    const selectedJava = ref<JavaSelection>({ path: '', version: '' })
    const memoryMb = ref(2048)
    const maxMemoryMb = ref(8192)

    const installPhase = ref<ModpackInstallPhase>('idle')
    const downloadProgress = ref<{ downloaded: number; total: number | null } | null>(null)
    const installLog = ref<string[]>([])
    const installError = ref<string | null>(null)
    const eulaText = ref('')
    const createdServer = ref<ServerManifestData | null>(null)
    const showEulaModal = ref(false)
    const saveServerProperties = ref<(() => Promise<boolean>) | null>(null)
    let installSession = 0

    const modpackTitle = ref('')
    const modpackVersionNumber = ref('')
    const modpackIconUrl = ref<string | undefined>(undefined)
    const loaderLabel = ref('')
    const loaderSupported = ref(false)
    const gameVersionLabel = ref('')

    const needsLoaderVersion = computed(
        () => serverType.value === 'fabric' || serverType.value === 'quilt',
    )
    const typeSupported = computed(() => loaderSupported.value)
    const canContinueFromType = computed(() => loaderSupported.value)

    function setPack(
        packProject: Labrinth.Projects.v2.Project,
        packVersion: Labrinth.Versions.v2.Version,
    ) {
        project.value = packProject
        version.value = packVersion
        modpackTitle.value = packProject.title
        modpackVersionNumber.value = packVersion.version_number ?? ''
        modpackIconUrl.value = packProject.icon_url ?? undefined

        const gameVersion = packVersion.game_versions?.[0] ?? packProject.game_versions?.[0] ?? ''
        // Merge both the project-level and version-level loader declarations.
        // Modpack versions frequently leave `version.loaders` empty (the project
        // field is the reliable source); the authoritative source is the mrpack's
        // `modrinth.index.json` dependencies, but that is only available after
        // download. See resolveModpackLoader's fallback note.
        const loaderCandidates = [...(packProject.loaders ?? []), ...(packVersion.loaders ?? [])]
        const loader = resolveModpackLoader(loaderCandidates)
        serverType.value = loader.type
        loaderLabel.value = loader.label
        gameVersionLabel.value = gameVersion
        selectedGameVersion.value = gameVersion
        availableGameVersions.value = gameVersion ? [gameVersion] : []
        loaderSupported.value = SUPPORTED_MODPACK_LOADERS.includes(loader.type)

        // Default the server name to `<modpack title> <version number>` so different
        // versions of the same modpack produce distinct server names instead of
        // colliding.  A short uid is appended only if a name collision remains
        // (see beginInstall), mirroring the direct-server id style.
        name.value = `${packProject.title} ${packVersion.version_number ?? ''}`.trim()
        selectedLoaderVersion.value = ''
        loaderVersions.value = []
    }

    async function loadVersions() {
        // The modpack fixes the game version; nothing to load.
    }

    async function loadLoaderVersions() {
        selectedLoaderVersion.value = ''
        loaderVersions.value = []
        if (!needsLoaderVersion.value || !selectedGameVersion.value) return
        try {
            const manifest = (await get_loader_versions(
                serverType.value,
                selectedGameVersion.value,
            )) as {
                gameVersions: Array<{ id: string; loaders: { id: string; stable: boolean }[] }>
            }
            const entry = manifest.gameVersions.find(
                (game) => game.id === selectedGameVersion.value,
            )
            loaderVersions.value = entry?.loaders ?? []
            const stable =
                loaderVersions.value.find((option) => option.stable) ?? loaderVersions.value[0]
            selectedLoaderVersion.value = stable?.id ?? ''
        } catch {
            loaderVersions.value = []
        }
    }

    async function loadDefaultJava() {
        if (selectedJava.value.path !== '') return
        const major = javaMajorFromVersion(selectedGameVersion.value || '1.21') ?? 21
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
        if (!project.value || !version.value) return
        if (!loaderSupported.value) return

        // A closed wizard leaves its install promise running in the background.
        // Reopening the wizard starts a fresh session; stale sessions must stop
        // touching the shared state once their token is superseded.
        const session = ++installSession
        const isStale = () => installSession !== session

        installPhase.value = 'preparing'
        installError.value = null
        installLog.value = []
        downloadProgress.value = null
        try {
            await loadLoaderVersions()
            if (isStale()) return

            const requiredJava = javaMajorFromVersion(selectedGameVersion.value) ?? 21
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

            if (!createdServer.value) {
                // Ensure the chosen name is unique among existing servers.  When it
                // collides we append a short uid (mirroring the direct-server id
                // style) so duplicate modpack versions stay distinguishable; if the
                // name is free, no suffix is added.
                let finalName = name.value.trim()
                try {
                    const existing = await servers.list()
                    const taken = new Set(
                        existing.map((server) => server.name.trim().toLowerCase()),
                    )
                    if (taken.has(finalName.toLowerCase())) {
                        const uid = Math.random().toString(36).slice(2, 6)
                        finalName = `${finalName} ${uid}`
                    }
                } catch {
                    // Best-effort uniqueness; the backend id already disambiguates.
                }

                const manifest = await servers.create({
                    name: finalName,
                    serverType: serverType.value,
                    gameVersion: selectedGameVersion.value,
                    loaderVersion: needsLoaderVersion.value
                        ? selectedLoaderVersion.value
                        : undefined,
                    javaPath: selectedJava.value.path || undefined,
                    memoryMb: memoryMb.value,
                })
                if (isStale()) {
                    await servers.delete(manifest.id).catch(() => {})
                    void refreshServerList()
                    return
                }
                createdServer.value = manifest
            }
            const serverId = createdServer.value.id

            // Past this point the server directory exists and the backend tracks
            // install state on its manifest, so failures leave a retryable entry
            // instead of being cleaned up. Only pre-install resolution errors
            // (no launcher, no pack file) still remove the stub.
            let dispatched = false
            const unlistenEvents = await serverEventListener((id, payload) => {
                if (id !== serverId || isStale()) return
                if (payload.event === 'download_progress') {
                    downloadProgress.value = {
                        downloaded: payload.downloaded,
                        total: payload.total ?? null,
                    }
                } else if (payload.event === 'log') {
                    installLog.value.push(payload.line)
                    if (installLog.value.length > 500) {
                        installLog.value.splice(0, installLog.value.length - 500)
                    }
                }
            })
            try {
                const jar = await resolveServerLauncher(
                    serverType.value,
                    selectedGameVersion.value,
                    selectedLoaderVersion.value,
                )
                if (!jar) {
                    throw new Error(
                        `No server launcher available for ${loaderLabel.value} on ${selectedGameVersion.value}`,
                    )
                }

                const primaryFile =
                    version.value.files.find((file) => file.primary) ?? version.value.files[0]
                if (!primaryFile?.url) {
                    throw new Error('Modpack has no downloadable file')
                }

                // The download runs through the shared background runner, so closing
                // the wizard keeps it going; progress renders from the shared registry.
                dispatched = true
                installPhase.value = 'downloading'
                // [SERVER-DOWNLOAD-BRIDGE] Pass the download manager reference
                // captured during setup so the synthetic job appears in sidebar.
                await startModpackServerInstall(
                    serverId,
                    {
                        mrpackUrl: primaryFile.url,
                        mrpackSha1: primaryFile.hashes?.sha1,
                        jarUrl: jar.url,
                        jarFilename: jar.filename,
                        jarSha1: jar.sha1,
                        modpackProjectId: project.value.id,
                        modpackVersionId: version.value.id,
                        modpackTitle: `${modpackTitle.value} ${modpackVersionNumber.value}`.trim(),
                        modpackIconUrl: modpackIconUrl.value,
                    },
                    downloadManager,
                )
                if (isStale()) return

                // Modpack installation complete, no auto-start.
                // User will click "Start" later, which will handle EULA check via tryStartServer.
                // A code-created `eula.txt` (eula=false) is written so the manual start
                // gate can offer the EULA without booting the jar.
                const eula = setEulaAccepted('', false)
                await servers.writeFile(serverId, 'eula.txt', eula).catch(() => {})
                installPhase.value = 'done'
            } catch (error) {
                if (!dispatched && createdServer.value) {
                    const failed = createdServer.value
                    createdServer.value = null
                    await servers.delete(failed.id).catch(() => {})
                    void refreshServerList()
                }
                throw error
            } finally {
                unlistenEvents()
            }
        } catch (error) {
            installPhase.value = 'error'
            installError.value = toErrorMessage(error)
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
            // Start the server after accepting EULA
            await servers.start(createdServer.value.id)
            // Wait for server to stop (crash or normal)
            const stopped = await waitForServerStop(createdServer.value.id)
            if (stopped?.event === 'stopped' && stopped.crashed) {
                throw new Error(formatMessage(wizardMessages.firstRunCrashed))
            }
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
        installSession++
        installPhase.value = 'idle'
        installLog.value = []
        installError.value = null
        eulaText.value = ''
        createdServer.value = null
        showEulaModal.value = false
        saveServerProperties.value = null
        selectedJava.value = { path: '', version: '' }
        memoryMb.value = 2048
        void loadMaxMemory()
    }

    const stageConfigs: StageConfigInput<ModpackServerFlowContext>[] = [
        {
            id: 'setup',
            stageContent: markRaw(ModpackSetupStage),
            title: (ctx: ModpackServerFlowContext) => ctx.formatMessage(wizardMessages.setupTitle),
            cannotNavigateForward: (ctx: ModpackServerFlowContext) =>
                ctx.name.value.trim() === '' || !ctx.canContinueFromType.value,
            leftButtonConfig: () => null,
            rightButtonConfig: (ctx: ModpackServerFlowContext) => ({
                label: ctx.formatMessage(wizardMessages.next),
                color: 'brand',
                disabled: ctx.name.value.trim() === '' || !ctx.canContinueFromType.value,
                onClick: async () => {
                    await ctx.loadDefaultJava()
                    ctx.modal.value?.nextStage()
                },
            }),
        },
        {
            id: 'install',
            stageContent: markRaw(ModpackInstallStage),
            title: (ctx: ModpackServerFlowContext) =>
                ctx.formatMessage(wizardMessages.installTitle),
            cannotNavigateForward: (ctx: ModpackServerFlowContext) =>
                ctx.installPhase.value !== 'done',
            // Downloads continue in the background once the wizard closes; only
            // the first-run boot locks closing.
            disableClose: (ctx: ModpackServerFlowContext) => ctx.installPhase.value === 'first-run',
            leftButtonConfig: () => null,
            rightButtonConfig: (ctx: ModpackServerFlowContext) => ({
                label: ctx.formatMessage(
                    ctx.installPhase.value === 'error'
                        ? wizardMessages.retry
                        : ctx.installPhase.value === 'done'
                          ? wizardMessages.finish
                          : wizardMessages.next,
                ),
                color: 'brand',
                icon: ctx.installPhase.value === 'error' ? RefreshCwIcon : null,
                iconPosition: 'after',
                disabled: ctx.installPhase.value !== 'done' && ctx.installPhase.value !== 'error',
                onClick: () => {
                    if (ctx.installPhase.value === 'error') {
                        void ctx.retryInstall()
                        return
                    }
                    if (ctx.installPhase.value === 'done') {
                        ctx.modal.value?.hide()
                        return
                    }
                    ctx.modal.value?.nextStage()
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
        modpackTitle,
        modpackVersionNumber,
        modpackIconUrl,
        loaderLabel,
        loaderSupported,
        gameVersionLabel,
        loadVersions,
        loadLoaderVersions,
        loadDefaultJava,
        beginInstall,
        retryInstall,
        acceptEula,
        declineEula,
        reset,
        setPack,
    }
}
