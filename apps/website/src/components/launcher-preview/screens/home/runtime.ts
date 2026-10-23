import { createContext } from '@modrinth/ui/src/providers/create-context.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, type ComputedRef, type Ref, shallowRef, watch } from 'vue'

import { createShortcutNotice, revealInFolder } from '../../lib/platform/opener'
import { usePreview } from '../../lib/state'
import type { ProcessState } from '../../lib/state/processes'
import type { GameInstance } from '../../lib/types'
import { getActivePlayerAccount } from './home-utils'
import type { HomeState } from './state'

/** The native alert behind `window.confirm`, drawn inside the window. */
export type HomeSystemDialog = {
    kind: 'confirm'
    message: string
    resolve: (confirmed: boolean) => void
}

/**
 * The Home page's bridge to the launcher: the app's `helpers/*` calls
 * (`run`, `kill`, `start_join_*`, `showInstanceInFolder`, the servers API, …)
 * reproduced on the preview's in-memory launcher.
 */
export type HomeRuntime = HomeState & {
    playerName: ComputedRef<string | null>
    playerAccountId: ComputedRef<string | null>
    runningInstanceIds: ComputedRef<string[]>
    systemDialog: Ref<HomeSystemDialog | null>
    isLaunching: (instanceId: string) => boolean
    run: (instance: GameInstance) => Promise<void>
    kill: (instanceId: string) => Promise<void>
    startJoinSingleplayerWorld: (instance: GameInstance, path: string) => Promise<void>
    startJoinServer: (instance: GameInstance, address: string) => Promise<void>
    installExistingInstance: (instance: GameInstance) => Promise<void>
    showInstanceInFolder: (instanceId: string) => void
    copyToClipboard: (text: string) => Promise<void>
    createInstanceShortcut: (name: string, instanceId: string) => Promise<string | null>
    confirm: (message: string) => Promise<boolean>
    startServer: (serverId: string) => Promise<boolean>
    stopServer: (serverId: string) => Promise<boolean>
    killServer: (serverId: string) => Promise<boolean>
    waitUntilReady: (serverId: string) => Promise<boolean>
    setServerHomePinned: (serverId: string, pinned: boolean) => void
    activeOfflineAccountName: ComputedRef<string | null>
    handleError: (error: unknown) => void
}

export const [injectHomeRuntime, provideHomeRuntimeContext] = createContext<HomeRuntime>(
    'HomeRuntime',
    'launcher-preview-home-runtime',
)

/** How long app-lib waits for a linked server to accept connections. */
const READY_TIMEOUT_MS = 180_000

export function provideHomeRuntime(state: HomeState): HomeRuntime {
    const preview = usePreview()
    const { handleError } = injectNotificationManager()
    const systemDialog = shallowRef<HomeSystemDialog | null>(null)

    const activeAccount = computed(() =>
        getActivePlayerAccount(
            preview.accounts.selected.value?.account_id ?? null,
            preview.accounts.accounts.value,
        ),
    )
    const playerName = computed(() => activeAccount.value?.name ?? null)
    const playerAccountId = computed(() => activeAccount.value?.id ?? null)
    const activeOfflineAccountName = computed(() => {
        const account = preview.accounts.selected.value
        if (account?.account_type !== 'offline') return null
        return account.profile?.name || 'offline'
    })

    /**
     * The processes the launcher reports (`process.get_all`): a game counts once
     * its process is up, not while the launch is still preparing it.
     */
    function liveProcesses(): ProcessState[] {
        // Launch progress is not reactive in the shared process list; reading
        // the pending launch makes this re-run when a launch completes.
        void preview.processes.pendingInstanceId.value
        return preview.processes.processes.value
    }

    const runningInstanceIds = computed(() =>
        liveProcesses()
            .filter((process) => process.stage === 'running' || process.stage === 'stopping')
            .map((process) => process.instance.id),
    )

    function isLaunching(instanceId: string) {
        return liveProcesses().some(
            (process) =>
                process.instance.id === instanceId &&
                process.stage !== 'running' &&
                process.stage !== 'stopping',
        )
    }

    function waitFor(condition: () => boolean, source: () => unknown): Promise<void> {
        if (condition()) return Promise.resolve()
        return new Promise((resolve) => {
            const stop = watch(source, () => {
                if (!condition()) return
                stop()
                resolve()
            })
        })
    }

    function launchSource() {
        return [preview.processes.processes.value, preview.processes.pendingInstanceId.value]
    }

    /**
     * `run(instanceId)`: resolves once the game process has started, which is
     * how long the app's play buttons spin. A launch that is already preparing
     * the same instance is joined instead of started twice.
     */
    async function run(instance: GameInstance): Promise<void> {
        for (;;) {
            if (runningInstanceIds.value.includes(instance.id)) return
            const existing = preview.processes.processes.value.find(
                (process) => process.instance.id === instance.id,
            )
            if (!existing) {
                const process = preview.processes.launch(instance)
                if (process) {
                    await waitFor(
                        () =>
                            process.stage === 'running' ||
                            !preview.processes.processes.value.some(
                                (entry) => entry.uuid === process.uuid,
                            ),
                        launchSource,
                    )
                    // The shared list advances a launch's stage in place, which
                    // its watchers (the titlebar's running pill) do not see;
                    // republish the list so they pick up the started game.
                    preview.processes.processes.value = [...preview.processes.processes.value]
                    return
                }
            }
            // Another launch is still preparing its instance: start once it is up.
            await waitFor(() => !preview.processes.pendingInstanceId.value, launchSource)
        }
    }

    /** `kill(instanceId)`: stops the instance's game and resolves once the process has exited. */
    async function kill(instanceId: string): Promise<void> {
        const processes = preview.processes.processes.value.filter(
            (process) => process.instance.id === instanceId,
        )
        if (!processes.length) return
        for (const process of processes) preview.processes.stop(process.uuid)
        await waitFor(
            () =>
                !preview.processes.processes.value.some((process) =>
                    processes.some((entry) => entry.uuid === process.uuid),
                ),
            launchSource,
        )
    }

    /** `start_join_singleplayer_world`: launches straight into the world (Quick Play). */
    async function startJoinSingleplayerWorld(instance: GameInstance, path: string) {
        state.pendingJoins.set(instance.id, {
            instanceId: instance.id,
            type: 'singleplayer',
            identifier: path,
        })
        await run(instance)
    }

    /** `start_join_server`: launches straight onto the server (Quick Play). */
    async function startJoinServer(instance: GameInstance, address: string) {
        state.pendingJoins.set(instance.id, {
            instanceId: instance.id,
            type: 'server',
            identifier: address,
        })
        await run(instance)
    }

    /** `install_existing_instance`: downloads the game files of an instance that lacks them. */
    async function installExistingInstance(instance: GameInstance) {
        const target = preview.instances.get(instance.id)
        if (!target) return
        target.install_stage = 'minecraft_installing'
        state.instanceRevision.value += 1
        await state.delay(3200)
        target.install_stage = 'installed'
        state.instanceRevision.value += 1
    }

    /** `showInstanceInFolder`: Finder belongs to the desktop app. */
    function showInstanceInFolder(instanceId: string) {
        revealInFolder(instanceId)
    }

    async function copyToClipboard(text: string) {
        await navigator.clipboard?.writeText(text)
    }

    /** Shows a system dialog; one that is still open is dismissed as cancelled. */
    function openSystemDialog(dialog: HomeSystemDialog) {
        systemDialog.value?.resolve(false)
        systemDialog.value = dialog
    }

    /** `window.confirm`, which the desktop app shows as a system alert. */
    function confirm(message: string) {
        return new Promise<boolean>((resolve) => {
            const dialog: HomeSystemDialog = {
                kind: 'confirm',
                message,
                resolve: (confirmed) => {
                    if (systemDialog.value === dialog) systemDialog.value = null
                    resolve(confirmed)
                },
            }
            openSystemDialog(dialog)
        })
    }

    /** `createInstanceShortcut`: shortcut files are written by the desktop app. */
    async function createInstanceShortcut(_name: string, _instanceId: string) {
        createShortcutNotice()
        return null
    }

    /* -- local servers ------------------------------------------------------ */

    const serversBackend = () =>
        import('../multiplayer/backend.ts').then((module) => module.servers)

    async function startServer(serverId: string) {
        if (!preview.servers.get(serverId)) return false
        await (await serversBackend()).start(serverId)
        return true
    }

    async function stopServer(serverId: string) {
        if (!preview.servers.get(serverId)) return false
        await (await serversBackend()).stop(serverId)
        return true
    }

    async function killServer(serverId: string) {
        if (!preview.servers.get(serverId)) return false
        await (await serversBackend()).kill(serverId)
        return true
    }

    /** `servers.waitUntilReady`: resolves once the server accepts connections, or gives up. */
    async function waitUntilReady(serverId: string) {
        return (await serversBackend()).waitUntilReady(serverId, READY_TIMEOUT_MS)
    }

    async function setServerHomePinned(serverId: string, pinned: boolean) {
        await (await serversBackend()).updateSettings(serverId, { homePinned: pinned })
    }

    const runtime: HomeRuntime = {
        ...state,
        playerName,
        playerAccountId,
        runningInstanceIds,
        systemDialog,
        isLaunching,
        run,
        kill,
        startJoinSingleplayerWorld,
        startJoinServer,
        installExistingInstance,
        showInstanceInFolder,
        copyToClipboard,
        createInstanceShortcut,
        confirm,
        startServer,
        stopServer,
        killServer,
        waitUntilReady,
        setServerHomePinned,
        activeOfflineAccountName,
        handleError,
    }

    provideHomeRuntimeContext(runtime)
    return runtime
}

export function useHomeRuntime(): HomeRuntime {
    return injectHomeRuntime()
}
