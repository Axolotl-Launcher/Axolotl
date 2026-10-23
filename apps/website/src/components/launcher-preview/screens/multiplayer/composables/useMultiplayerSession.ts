import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, onMounted, onUnmounted, ref } from 'vue'

import { exportErrorLogs } from '../app-helpers.ts'
import { hongshiErrorMessage } from '../rooms/hongshi-messages.ts'
import {
    type DetectedLanPort,
    type HongshiNode,
    multiplayer,
    multiplayerErrorText,
    type MultiplayerProvider,
    type MultiplayerState,
} from '../rooms/multiplayer.ts'
import { DesktopOnlyRoomsError } from '../rooms/rooms-backend.ts'
import { terracotta, type TerracottaUpdate } from '../rooms/terracotta.ts'

const ACTIVE_POLL_INTERVAL = 500
const IDLE_POLL_INTERVAL = 2000

export function useMultiplayerSession() {
    const { handleError } = injectNotificationManager()
    const { formatMessage } = useVIntl()
    const multiplayerState = ref<MultiplayerState | null>(null)
    const nodes = ref<HongshiNode[]>([])
    const detectedPorts = ref<DetectedLanPort[]>([])
    const playerName = ref('')
    const roomCodeInput = ref('')
    const platformKey = ref('unknown')
    const isActionPending = ref(false)
    const isStopping = ref(false)
    const isNodesLoading = ref(false)
    const nodesLoaded = ref(false)
    const nodesError = ref(false)
    const isExportingReport = ref(false)
    const terracottaUpdate = ref<TerracottaUpdate | null>(null)

    let mounted = false
    let pollTimer: ReturnType<typeof setTimeout> | undefined
    let pollPromise: Promise<void> | undefined

    const state = computed(() => multiplayerState.value?.terracotta ?? null)
    const hongshiState = computed(() => multiplayerState.value?.hongshi ?? null)
    const activeProvider = computed(() => multiplayerState.value?.active_provider ?? null)

    function pollInterval() {
        const status = hongshiState.value?.status
        return isActionPending.value ||
            activeProvider.value ||
            ['selecting_node', 'starting', 'open'].includes(status ?? '')
            ? ACTIVE_POLL_INTERVAL
            : IDLE_POLL_INTERVAL
    }

    function schedulePoll() {
        if (!mounted) return
        clearTimeout(pollTimer)
        pollTimer = setTimeout(() => void pollState(), pollInterval())
    }

    async function pollState() {
        if (!mounted) return
        if (pollPromise) return pollPromise
        pollPromise = Promise.all([multiplayer.getState(), multiplayer.getDetectedPorts()])
            .then(([nextState, ports]) => {
                if (!mounted) return
                multiplayerState.value = nextState
                detectedPorts.value = ports
            })
            .catch((error: unknown) => {
                if (mounted) console.error(error)
            })
            .finally(() => {
                pollPromise = undefined
                schedulePoll()
            })
        return pollPromise
    }

    async function runAction(action: () => Promise<void>, hongshi = false) {
        if (isActionPending.value || isStopping.value) return false
        isActionPending.value = true
        schedulePoll()
        try {
            await action()
            return true
        } catch (error: unknown) {
            // The preview cannot run the room programs; its notice replaces the app's error text.
            if (error instanceof DesktopOnlyRoomsError) handleError(error)
            else if (hongshi) {
                if (multiplayerErrorText(error).includes('RedStone operation cancelled'))
                    return false
                const snapshot = await multiplayer.getState().catch(() => null)
                handleError(formatMessage(hongshiErrorMessage(snapshot?.hongshi.error_type)))
            } else handleError(error)
            return false
        } finally {
            isActionPending.value = false
            await pollState()
        }
    }

    async function refreshNodes(forceRefresh = false) {
        if (isNodesLoading.value) return
        isNodesLoading.value = true
        try {
            nodes.value = await multiplayer.getNodes(forceRefresh)
            nodesLoaded.value = true
            nodesError.value = false
        } catch {
            nodesError.value = true
            handleError(formatMessage(hongshiErrorMessage('node_list')))
        } finally {
            isNodesLoading.value = false
        }
    }

    async function runControlAction(action: () => Promise<void>) {
        if (isStopping.value) return false
        isStopping.value = true
        try {
            await action()
            return true
        } catch (error: unknown) {
            handleError(error)
            return false
        } finally {
            isStopping.value = false
            await pollState()
        }
    }
    const switchProvider = (provider: MultiplayerProvider) =>
        runControlAction(() => multiplayer.switchProvider(provider))
    const startTerracotta = () => runAction(multiplayer.prepareTerracotta)
    const hostTerracotta = () => runAction(() => multiplayer.hostTerracotta(playerName.value))
    const joinTerracotta = () =>
        runAction(() => multiplayer.joinTerracotta(playerName.value, roomCodeInput.value))
    const hostHongshi = (localPort: number, nodeName: string | null, instanceId: string | null) =>
        runAction(() => multiplayer.hostHongshi(localPort, nodeName, instanceId), true)
    const stop = () => runControlAction(multiplayer.stop)
    const reset = () => runControlAction(multiplayer.reset)
    const downloadTerracotta = () => runAction(terracotta.download)
    const updateTerracotta = () =>
        runAction(async () => {
            terracottaUpdate.value = await terracotta.update()
        })
    const downloadHongshi = () => runAction(multiplayer.downloadHongshi, true)
    async function exportTerracottaReport() {
        if (isExportingReport.value) return
        isExportingReport.value = true
        try {
            const report = await terracotta.getDiagnosticReport()
            await exportErrorLogs(report, 'Axolotl multiplayer error report')
        } catch (error: unknown) {
            handleError(error)
        } finally {
            isExportingReport.value = false
        }
    }
    const openHongshiLogs = () => runAction(multiplayer.openHongshiLogs)

    onMounted(() => {
        mounted = true
        void pollState()
        void terracotta
            .getPlatformKey()
            .then((value) => {
                if (mounted) platformKey.value = value
            })
            .catch(() => undefined)
        void terracotta
            .checkForUpdate()
            .then((update) => {
                if (mounted) terracottaUpdate.value = update
            })
            .catch(() => undefined)
        void multiplayer
            .getPlayerName()
            .then((value) => {
                if (mounted && value) playerName.value = value
            })
            .catch(() => undefined)
    })

    onUnmounted(() => {
        mounted = false
        clearTimeout(pollTimer)
    })

    return {
        activeProvider,
        detectedPorts,
        downloadTerracotta,
        downloadHongshi,
        exportTerracottaReport,
        hongshiState,
        hostHongshi,
        hostTerracotta,
        isActionPending,
        isStopping,
        isNodesLoading,
        nodesLoaded,
        nodesError,
        isExportingReport,
        joinTerracotta,
        multiplayerState,
        nodes,
        openHongshiLogs,
        platformKey,
        playerName,
        refreshNodes,
        reset,
        roomCodeInput,
        startTerracotta,
        state,
        stop,
        switchProvider,
        terracottaUpdate,
        updateTerracotta,
    }
}
