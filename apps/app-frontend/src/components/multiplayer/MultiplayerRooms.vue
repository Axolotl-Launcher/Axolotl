<script setup lang="ts">
import {
    ArrowLeftIcon,
    BinaryIcon,
    CheckCircleIcon,
    DownloadIcon,
    DropdownIcon,
    GlobeIcon,
    LogInIcon,
    LogOutIcon,
    PlayIcon,
    RefreshCwIcon,
    ServerIcon,
    SpinnerIcon,
    UserIcon,
    UsersIcon,
} from '@modrinth/assets'
import {
    Admonition,
    Button,
    Card,
    Combobox,
    CopyCode,
    defineMessages,
    NavTabs,
    PopoutMenu,
    ProgressBar,
    StyledInput,
    TagItem,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import hongshiIcon from '@/assets/multiplayer/hongshi.png'
import terracottaIcon from '@/assets/multiplayer/terracotta.png'
import { useMultiplayerSession } from '@/composables/useMultiplayerSession'
import { hongshiErrorMessage } from '@/helpers/hongshi-messages'
import {
    type MultiplayerProvider,
    selectedDetectedInstance,
    selectedNodePreference,
    storedMultiplayerProvider,
    validLocalPort,
} from '@/helpers/multiplayer'
import {
    isValidTerracottaRoomCode,
    type TerracottaPlayer,
    type TerracottaStatus,
} from '@/helpers/terracotta'

const { formatMessage } = useVIntl()
const messages = defineMessages({
    host: { id: 'app.multiplayer.host', defaultMessage: 'Host' },
    join: { id: 'app.multiplayer.join', defaultMessage: 'Join' },
    hostDescription: {
        id: 'app.multiplayer.host-description',
        defaultMessage: 'Create a virtual LAN room so friends can connect directly to your game.',
    },
    lanHint: {
        id: 'app.multiplayer.lan-hint',
        defaultMessage:
            'Open your Minecraft world, then press Esc → Open to LAN → choose a port. Terracotta will detect it automatically.',
    },
    joinDescription: {
        id: 'app.multiplayer.join-description',
        defaultMessage: "Enter a room code to join a friend's virtual LAN room.",
    },
    playerName: {
        id: 'app.multiplayer.player-name',
        defaultMessage: 'Player name',
    },
    roomCode: {
        id: 'app.multiplayer.room-code',
        defaultMessage: 'Room code',
    },
    roomCodePlaceholder: {
        id: 'app.multiplayer.room-code-placeholder',
        defaultMessage: 'e.g. U/ABCD-EFGH-IJKL-MNOP',
    },
    roomCodeInvalid: {
        id: 'app.multiplayer.room-code-invalid',
        defaultMessage: 'Enter a room code in the format U/XXXX-XXXX-XXXX-XXXX.',
    },
    startHosting: {
        id: 'app.multiplayer.start-hosting',
        defaultMessage: 'Start hosting',
    },
    joinRoom: {
        id: 'app.multiplayer.join-room',
        defaultMessage: 'Join room',
    },
    copyRoomCode: {
        id: 'app.multiplayer.copy-room-code',
        defaultMessage: 'Copy room code',
    },
    back: {
        id: 'app.multiplayer.back',
        defaultMessage: 'Back',
    },
    disconnect: {
        id: 'app.multiplayer.disconnect',
        defaultMessage: 'Disconnect',
    },
    statusIdle: {
        id: 'app.multiplayer.status.idle',
        defaultMessage: 'Not connected',
    },
    statusStarting: {
        id: 'app.multiplayer.status.starting',
        defaultMessage: 'Starting...',
    },
    statusWaiting: {
        id: 'app.multiplayer.status.waiting',
        defaultMessage: 'Waiting...',
    },
    statusHostScanning: {
        id: 'app.multiplayer.status.host-scanning',
        defaultMessage: 'Creating room...',
    },
    statusHostStarting: {
        id: 'app.multiplayer.status.host-starting',
        defaultMessage: 'Starting host...',
    },
    statusHostReady: {
        id: 'app.multiplayer.status.host-ready',
        defaultMessage: 'Room ready',
    },
    statusGuestConnecting: {
        id: 'app.multiplayer.status.guest-connecting',
        defaultMessage: 'Joining room...',
    },
    statusGuestStarting: {
        id: 'app.multiplayer.status.guest-starting',
        defaultMessage: 'Connecting as guest...',
    },
    statusGuestReady: {
        id: 'app.multiplayer.status.guest-ready',
        defaultMessage: 'Connected to room',
    },
    statusError: {
        id: 'app.multiplayer.status.error',
        defaultMessage: 'Error',
    },
    statusFatal: {
        id: 'app.multiplayer.status.fatal',
        defaultMessage: 'Fatal error',
    },
    statusDownloading: {
        id: 'app.multiplayer.status.downloading',
        defaultMessage: 'Downloading...',
    },
    players: {
        id: 'app.multiplayer.players',
        defaultMessage: 'Players',
    },
    playersInRoom: {
        id: 'app.multiplayer.players-in-room',
        defaultMessage: '{count} player(s) in room',
    },
    notRunning: {
        id: 'app.multiplayer.not-running',
        defaultMessage:
            'Multiplayer service is not running. Start hosting or join a room to begin.',
    },
    notRunningTitle: {
        id: 'app.multiplayer.not-running-title',
        defaultMessage: 'Start a multiplayer session',
    },
    shareCode: {
        id: 'app.multiplayer.share-code',
        defaultMessage: 'Share this code with friends to let them join:',
    },
    serverAddress: {
        id: 'app.multiplayer.server-address',
        defaultMessage: 'Backup connection address',
    },
    hostLabel: {
        id: 'app.multiplayer.host-label',
        defaultMessage: 'Host',
    },
    guestLabel: {
        id: 'app.multiplayer.guest-label',
        defaultMessage: 'Guest',
    },
    unknownPlayerRole: {
        id: 'app.multiplayer.unknown-player-role',
        defaultMessage: 'Unknown role',
    },
    platformInfo: {
        id: 'app.multiplayer.platform-info',
        defaultMessage: 'Current platform: {platform}',
    },
    binaryNotFound: {
        id: 'app.multiplayer.binary-not-found',
        defaultMessage: 'Terracotta binary not found. Please download it and place it at:',
    },
    downloadTerracotta: {
        id: 'app.multiplayer.download-terracotta',
        defaultMessage: 'Download Terracotta',
    },
    updateTerracotta: {
        id: 'app.multiplayer.terracotta.update',
        defaultMessage: 'Update Terracotta',
    },
    terracottaUpdateAvailable: {
        id: 'app.multiplayer.terracotta.update-available',
        defaultMessage: 'Terracotta {version} is ready to install.',
    },
    retry: {
        id: 'app.multiplayer.retry',
        defaultMessage: 'Retry',
    },
    exportErrorReport: {
        id: 'app.multiplayer.export-error-report',
        defaultMessage: 'Export error report',
    },
    checkNetwork: {
        id: 'app.multiplayer.check-network',
        defaultMessage: 'Check your network connection',
    },
    downloadProgress: {
        id: 'app.multiplayer.download-progress',
        defaultMessage: 'Download progress',
    },
    verifying: {
        id: 'app.multiplayer.verifying',
        defaultMessage: 'Verifying...',
    },
    extracting: {
        id: 'app.multiplayer.extracting',
        defaultMessage: 'Extracting...',
    },
    installing: {
        id: 'app.multiplayer.installing',
        defaultMessage: 'Installing...',
    },
    connecting: {
        id: 'app.multiplayer.connecting',
        defaultMessage: 'Connecting...',
    },
    errorNetwork: {
        id: 'app.multiplayer.error.network',
        defaultMessage: 'Network error',
    },
    errorInstall: {
        id: 'app.multiplayer.error.install',
        defaultMessage: 'Installation error',
    },
    errorTerracotta: {
        id: 'app.multiplayer.error.terracotta',
        defaultMessage: 'Terracotta error',
    },
    errorUnknown: {
        id: 'app.multiplayer.error.unknown',
        defaultMessage: 'Unknown error',
    },
    errorOs: {
        id: 'app.multiplayer.error.os',
        defaultMessage: 'System error',
    },
    poweredByTerracotta: {
        id: 'app.multiplayer.powered-by-terracotta',
        defaultMessage: 'Powered by Terracotta',
    },
    startTerracotta: {
        id: 'app.multiplayer.start-terracotta',
        defaultMessage: 'Start Terracotta',
    },
    startDescription: {
        id: 'app.multiplayer.start-description',
        defaultMessage: "Start the multiplayer service to host games or join friends' rooms.",
    },
    loading: {
        id: 'app.multiplayer.loading',
        defaultMessage: 'Initializing...',
    },
    noPlayers: {
        id: 'app.multiplayer.no-players',
        defaultMessage: 'No players in room',
    },
    terracottaProvider: {
        id: 'app.multiplayer.provider.terracotta',
        defaultMessage: 'Terracotta',
    },
    hongshiProvider: {
        id: 'app.multiplayer.provider.hongshi',
        defaultMessage: 'RedStone Online 2',
    },
    hongshiUnsupported: {
        id: 'app.multiplayer.hongshi.unsupported',
        defaultMessage:
            'RedStone Online 2 is not available for this operating system or architecture.',
    },
    hongshiBinaryMissing: {
        id: 'app.multiplayer.hongshi.binary-missing',
        defaultMessage:
            'The RedStone Online 2 kernel will be installed when you create your first room.',
    },
    switchWarning: {
        id: 'app.multiplayer.switch-warning',
        defaultMessage:
            'Switching services will disconnect the current multiplayer session. Continue?',
    },
    localPort: { id: 'app.multiplayer.hongshi.local-port', defaultMessage: 'Local Minecraft port' },
    detectedPort: {
        id: 'app.multiplayer.hongshi.detected-port',
        defaultMessage: '{instance} — port {port}',
    },
    manualPort: {
        id: 'app.multiplayer.hongshi.manual-port',
        defaultMessage: 'Enter port manually',
    },
    portHint: {
        id: 'app.multiplayer.hongshi.port-hint',
        defaultMessage:
            'Open a world to LAN. Axolotl will detect the port automatically; external games can use a manual port.',
    },
    node: { id: 'app.multiplayer.hongshi.node', defaultMessage: 'Relay node' },
    autoNode: { id: 'app.multiplayer.hongshi.node-auto', defaultMessage: 'Auto — lowest latency' },
    nodeLabel: {
        id: 'app.multiplayer.hongshi.node-label',
        defaultMessage: '{name} — {latency} ms{cached}',
    },
    cachedNode: { id: 'app.multiplayer.hongshi.cached', defaultMessage: ' (cached)' },
    unreachableNode: { id: 'app.multiplayer.hongshi.unreachable', defaultMessage: 'unreachable' },
    refreshNodes: { id: 'app.multiplayer.hongshi.refresh-nodes', defaultMessage: 'Refresh nodes' },
    createTunnel: { id: 'app.multiplayer.hongshi.create', defaultMessage: 'Create public room' },
    creatingTunnel: {
        id: 'app.multiplayer.hongshi.creating',
        defaultMessage: 'Creating tunnel...',
    },
    publicAddress: {
        id: 'app.multiplayer.hongshi.public-address',
        defaultMessage: 'Public address',
    },
    publicAddressHint: {
        id: 'app.multiplayer.hongshi.public-address-hint',
        defaultMessage:
            'Friends can enter this address directly in Minecraft. They do not need RedStone Online 2.',
    },
    noNodes: {
        id: 'app.multiplayer.hongshi.no-nodes',
        defaultMessage: 'No relay nodes are listed right now. Refresh the list to try again.',
    },
    cancel: {
        id: 'app.multiplayer.hongshi.cancel',
        defaultMessage: 'Cancel',
    },
    portChanged: {
        id: 'app.multiplayer.hongshi.port-changed',
        defaultMessage:
            'Minecraft opened a different LAN port. Restart the tunnel before sharing it.',
    },
    restartTunnel: { id: 'app.multiplayer.hongshi.restart', defaultMessage: 'Restart tunnel' },
    openLogs: {
        id: 'app.multiplayer.hongshi.open-logs',
        defaultMessage: 'Open RedStone Online 2 logs',
    },
    closedTunnel: {
        id: 'app.multiplayer.hongshi.closed',
        defaultMessage:
            'The RedStone Online 2 room has closed. Create a new room to receive a new address.',
    },
    selectingNode: {
        id: 'app.multiplayer.hongshi.selecting-node',
        defaultMessage: 'Selecting the best relay node...',
    },
})

const tabIndex = ref(0)
const roomCodeTouched = ref(false)
const {
    activeProvider,
    detectedPorts,
    downloadTerracotta,
    exportTerracottaReport,
    hongshiState,
    hostHongshi,
    hostTerracotta: hostGame,
    isActionPending,
    isStopping,
    isNodesLoading,
    nodesLoaded,
    nodesError,
    isExportingReport,
    joinTerracotta: joinGame,
    nodes,
    openHongshiLogs,
    platformKey,
    playerName,
    refreshNodes,
    reset: resetState,
    roomCodeInput,
    startTerracotta,
    state,
    stop: stopMultiplayer,
    switchProvider,
    terracottaUpdate,
    updateTerracotta,
} = useMultiplayerSession()

const providerStorageKey = 'axolotl-multiplayer-provider'
const nodeStorageKey = 'axolotl-hongshi-node'
const route = useRoute()
const storedProvider =
    typeof route.query.provider === 'string'
        ? route.query.provider
        : localStorage.getItem(providerStorageKey)
const selectedProvider = ref<MultiplayerProvider>(storedMultiplayerProvider(storedProvider))
const selectedNodeName = ref(localStorage.getItem(nodeStorageKey) ?? 'auto')
const selectedInstanceId = ref('manual')
const initialPort = typeof route.query.port === 'string' ? route.query.port : ''
const manualPort = ref(validLocalPort(initialPort) ? initialPort : '25565')
const portSelectionTouched = ref(!!validLocalPort(initialPort))
const hongshiFailure = computed(() =>
    formatMessage(hongshiErrorMessage(hongshiState.value?.error_type)),
)

const hongshiSupported = computed(() => hongshiState.value?.supported ?? false)
const detectedPortOptions = computed(() => [
    'manual',
    ...detectedPorts.value.map((entry) => entry.instance_id),
])
const nodeOptions = computed(() => ['auto', ...nodes.value.map((node) => node.name)])
const selectedDetectedPort = computed(() =>
    detectedPorts.value.find((entry) => entry.instance_id === selectedInstanceId.value),
)
const effectiveLocalPort = computed(() => {
    if (selectedDetectedPort.value) return selectedDetectedPort.value.port
    return validLocalPort(manualPort.value)
})
const isHongshiBusy = computed(() =>
    ['downloading', 'selecting_node', 'starting'].includes(hongshiState.value?.status ?? ''),
)
const selectedNode = computed(() =>
    selectedNodeName.value === 'auto'
        ? null
        : (nodes.value.find((node) => node.name === selectedNodeName.value) ?? null),
)
const hasLoadedNodes = ref(false)
const providerOptions = computed(() => [
    {
        id: 'terracotta' as const,
        label: messages.terracottaProvider,
        image: terracottaIcon,
        disabled: false,
    },
    {
        id: 'hongshi' as const,
        label: messages.hongshiProvider,
        image: hongshiIcon,
        disabled: !hongshiSupported.value,
    },
])
const selectedProviderOption = computed(
    () =>
        providerOptions.value.find((option) => option.id === selectedProvider.value) ??
        providerOptions.value[0],
)

watch(
    detectedPorts,
    (ports) => {
        if (portSelectionTouched.value && selectedInstanceId.value === 'manual') return
        selectedInstanceId.value = selectedDetectedInstance(selectedInstanceId.value, ports)
    },
    { immediate: true },
)

watch(nodes, (value) => {
    selectedNodeName.value = selectedNodePreference(selectedNodeName.value, value)
    if (value.length > 0) hasLoadedNodes.value = true
})

watch(activeProvider, (provider) => {
    if (!provider) return
    selectedProvider.value = provider
    localStorage.setItem(providerStorageKey, provider)
})

watch(hongshiState, (state) => {
    if (state && !state.supported && selectedProvider.value === 'hongshi') {
        selectedProvider.value = 'terracotta'
        localStorage.setItem(providerStorageKey, 'terracotta')
    }
})

watch(
    [selectedProvider, hongshiSupported],
    ([provider, supported]) => {
        if (provider === 'hongshi' && supported && !hasLoadedNodes.value) {
            void refreshNodes()
        }
    },
    { immediate: true },
)

watch(selectedNodeName, (value) => localStorage.setItem(nodeStorageKey, value))

async function selectProvider(provider: MultiplayerProvider) {
    if (provider === selectedProvider.value || (provider === 'hongshi' && !hongshiSupported.value))
        return
    if (activeProvider.value && activeProvider.value !== provider) {
        if (!window.confirm(formatMessage(messages.switchWarning))) return
        if (!(await switchProvider(provider))) return
    }
    selectedProvider.value = provider
    localStorage.setItem(providerStorageKey, provider)
}

function detectedPortLabel(value: string) {
    if (value === 'manual') return formatMessage(messages.manualPort)
    const entry = detectedPorts.value.find((port) => port.instance_id === value)
    return entry
        ? formatMessage(messages.detectedPort, { instance: entry.instance_name, port: entry.port })
        : value
}

function nodeOptionLabel(value: string) {
    if (value === 'auto') return formatMessage(messages.autoNode)
    const node = nodes.value.find((entry) => entry.name === value)
    if (!node) return value
    if (!node.reachable) {
        return `${node.name} — ${formatMessage(messages.unreachableNode)}${
            node.cached ? formatMessage(messages.cachedNode) : ''
        }`
    }
    return formatMessage(messages.nodeLabel, {
        name: node.name,
        latency: node.latency_ms,
        cached: node.cached ? formatMessage(messages.cachedNode) : '',
    })
}

async function startHongshiTunnel() {
    if (!effectiveLocalPort.value) return
    await hostHongshi(
        effectiveLocalPort.value,
        selectedNodeName.value === 'auto' ? null : selectedNodeName.value,
        selectedInstanceId.value === 'manual' ? null : selectedInstanceId.value,
    )
}

async function restartHongshiTunnel() {
    if (!(await stopMultiplayer())) return
    await startHongshiTunnel()
}

const tabLinks = computed(() => [
    { label: formatMessage(messages.host), href: 'host', icon: UsersIcon },
    { label: formatMessage(messages.join), href: 'join', icon: LogInIcon },
])

const isRunning = computed(() => !!state.value?.http_port)
const isSessionReady = computed(
    () => state.value?.status === 'host_ready' || state.value?.status === 'guest_ready',
)
const isHostSession = computed(() => state.value?.status === 'host_ready')
const canSubmitSession = computed(
    () =>
        playerName.value.trim().length > 0 &&
        (tabIndex.value === 0 || roomCodeInput.value.trim().length > 0),
)
const isRoomCodeValid = computed(() => isValidTerracottaRoomCode(roomCodeInput.value))
const showRoomCodeError = computed(
    () => tabIndex.value === 1 && roomCodeTouched.value && !isRoomCodeValid.value,
)
const guestServerAddress = computed(() =>
    state.value?.server_port ? `127.0.0.1:${state.value.server_port}` : '',
)

const statusText = computed(() => {
    if (!state.value) return ''
    const statusMap = {
        idle: messages.statusIdle,
        starting: messages.statusStarting,
        waiting: messages.statusWaiting,
        host_scanning: messages.statusHostScanning,
        host_starting: messages.statusHostStarting,
        host_ready: messages.statusHostReady,
        guest_connecting: messages.statusGuestConnecting,
        guest_starting: messages.statusGuestStarting,
        guest_ready: messages.statusGuestReady,
        error: messages.statusError,
        fatal: messages.statusFatal,
        downloading: messages.statusDownloading,
    } satisfies Record<TerracottaStatus, (typeof messages)[keyof typeof messages]>
    return formatMessage(statusMap[state.value.status])
})

const playerCount = computed(() => state.value?.players?.length ?? 0)

function playerRoleMessage(kind: TerracottaPlayer['kind']) {
    if (kind === 'HOST') return messages.hostLabel
    if (kind === 'GUEST') return messages.guestLabel
    return messages.unknownPlayerRole
}

const binaryPathHint = computed(() => {
    const name = platformKey.value?.includes('windows') ? 'terracotta.exe' : 'terracotta'
    return `<launcher_dir>/terracotta/${name}`
})

const downloadStageText = computed(() => {
    if (state.value?.download_stage) {
        if (state.value.download_stage === 'downloading')
            return formatMessage(messages.downloadProgress)
        if (state.value.download_stage === 'verifying') return formatMessage(messages.verifying)
        if (state.value.download_stage === 'extracting') return formatMessage(messages.extracting)
        if (state.value.download_stage === 'installing') return formatMessage(messages.installing)
        if (state.value.download_stage === 'complete') return ''
        if (state.value.download_stage === 'preparing') return formatMessage(messages.connecting)
    }
    if (state.value?.status === 'downloading') {
        if (state.value.download_progress === null || state.value.download_progress === 0)
            return formatMessage(messages.connecting)
        if (state.value.download_progress! < 100) return formatMessage(messages.downloadProgress)
        return formatMessage(messages.verifying)
    }
    return ''
})

const errorTypeLabel = computed(() => {
    const et = state.value?.error_type
    switch (et) {
        case 'network':
            return formatMessage(messages.errorNetwork)
        case 'install':
            return formatMessage(messages.errorInstall)
        case 'terracotta':
            return formatMessage(messages.errorTerracotta)
        case 'os':
            return formatMessage(messages.errorOs)
        default:
            return formatMessage(messages.errorUnknown)
    }
})

const isRecoverable = computed(() => {
    const et = state.value?.error_type
    if (!et) return state.value?.status === 'error'
    return et !== 'os'
})

function submitJoin() {
    roomCodeTouched.value = true
    if (!isRoomCodeValid.value) return
    void joinGame()
}
</script>

<template>
    <div class="flex min-h-0 w-full flex-1 flex-col gap-3">
        <div class="flex min-w-0 flex-wrap items-center justify-end gap-3">
            <PopoutMenu placement="bottom-end">
                <Button size="md" class="flex min-w-36 items-center gap-2"
                    ><img
                        :src="selectedProviderOption.image"
                        class="size-5 shrink-0 object-contain"
                        alt=""
                    />
                    <span class="flex-1 text-left">{{
                        formatMessage(selectedProviderOption.label)
                    }}</span>
                    <DropdownIcon class="size-4 shrink-0" />
                </Button>
                <template #menu>
                    <div class="flex w-44 flex-col gap-1 p-1">
                        <Button
                            v-for="option in providerOptions"
                            :key="option.id"
                            :type="selectedProvider === option.id ? 'base' : 'quiet'"
                            class="flex w-full items-center gap-2 !justify-start text-left"
                            :disabled="option.disabled"
                            @click="selectProvider(option.id)"
                            ><img
                                :src="option.image"
                                class="size-4 shrink-0 object-contain"
                                alt=""
                            />
                            {{ formatMessage(option.label) }}
                        </Button>
                    </div>
                </template>
            </PopoutMenu>
        </div>

        <template v-if="selectedProvider === 'terracotta'">
            <Card v-if="!state" class="!m-0">
                <div class="flex items-center gap-3">
                    <SpinnerIcon class="size-8 animate-spin text-brand" />
                    <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.loading) }}
                    </h2>
                </div>
            </Card>

            <Card v-else-if="!state.binary_installed" class="!m-0">
                <div class="flex flex-col gap-5">
                    <div class="flex items-start gap-3">
                        <div
                            class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-highlight-orange text-orange"
                        >
                            <BinaryIcon class="size-5" />
                        </div>
                        <div class="min-w-0">
                            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.downloadTerracotta) }}
                            </h2>
                            <p class="mb-0 mt-1 text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.notRunning) }}
                            </p>
                        </div>
                    </div>

                    <Admonition type="warning" :header="formatMessage(messages.binaryNotFound)">
                        <div class="flex flex-col gap-2">
                            <span>{{
                                formatMessage(messages.platformInfo, { platform: platformKey })
                            }}</span>
                            <code
                                class="w-fit max-w-full select-all break-all rounded-lg bg-surface-3 px-2 py-1"
                            >
                                {{ binaryPathHint }}
                            </code>
                        </div>
                    </Admonition>

                    <ProgressBar
                        v-if="state.status === 'downloading'"
                        full-width
                        :progress="state.download_progress ?? 0"
                        :max="100"
                        :waiting="state.download_progress === null || state.download_progress === 0"
                        :label="downloadStageText || statusText"
                        show-progress
                    />

                    <div v-else class="flex flex-wrap gap-2">
                        <Button
                            type="colored"
                            color="brand"
                            :disabled="isActionPending"
                            @click="downloadTerracotta"
                            ><DownloadIcon />
                            {{ formatMessage(messages.downloadTerracotta) }}
                        </Button>
                    </div>
                </div>
            </Card>

            <Card
                v-else-if="state.status === 'starting' || state.status === 'downloading'"
                class="!m-0"
            >
                <div class="flex flex-col gap-5">
                    <div class="flex items-center gap-3">
                        <SpinnerIcon class="size-6 shrink-0 animate-spin text-orange" />
                        <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                            {{ statusText }}
                        </h2>
                    </div>
                    <ProgressBar
                        v-if="state.status === 'downloading'"
                        full-width
                        :progress="state.download_progress ?? 0"
                        :max="100"
                        :waiting="state.download_progress === null"
                        :label="downloadStageText"
                        show-progress
                    />
                </div>
            </Card>

            <Card
                v-else-if="isRunning && (state.status === 'idle' || state.status === 'waiting')"
                class="!m-0"
            >
                <div class="flex flex-col gap-5">
                    <NavTabs
                        mode="local"
                        :active-index="tabIndex"
                        :links="tabLinks"
                        @tab-click="tabIndex = $event"
                    />

                    <div>
                        <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(tabIndex === 0 ? messages.host : messages.join) }}
                        </h2>
                        <p class="mb-0 mt-1 text-[var(--color-text-tertiary)]">
                            {{
                                formatMessage(
                                    tabIndex === 0
                                        ? messages.hostDescription
                                        : messages.joinDescription,
                                )
                            }}
                        </p>
                    </div>

                    <div class="grid gap-4 md:grid-cols-2">
                        <label class="flex min-w-0 flex-col gap-2" for="multiplayer-player-name">
                            <span class="font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.playerName) }}
                            </span>
                            <StyledInput
                                id="multiplayer-player-name"
                                v-model="playerName"
                                :icon="UserIcon"
                                :placeholder="formatMessage(messages.playerName)"
                                autocomplete="off"
                            />
                        </label>

                        <label
                            v-if="tabIndex === 1"
                            class="flex min-w-0 flex-col gap-2"
                            for="multiplayer-room-code"
                        >
                            <span class="font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.roomCode) }}
                            </span>
                            <StyledInput
                                id="multiplayer-room-code"
                                v-model="roomCodeInput"
                                :icon="UsersIcon"
                                :placeholder="formatMessage(messages.roomCodePlaceholder)"
                                :error="showRoomCodeError"
                                :input-attrs="{
                                    'aria-invalid': showRoomCodeError,
                                    'aria-describedby': showRoomCodeError
                                        ? 'multiplayer-room-code-error'
                                        : undefined,
                                }"
                                autocomplete="off"
                                :spellcheck="false"
                                @focusout="roomCodeTouched = true"
                            />
                            <span
                                v-if="showRoomCodeError"
                                id="multiplayer-room-code-error"
                                class="text-xs text-red"
                            >
                                {{ formatMessage(messages.roomCodeInvalid) }}
                            </span>
                        </label>
                    </div>

                    <div class="flex flex-wrap gap-2">
                        <Button
                            type="colored"
                            color="brand"
                            native-type="button"
                            :disabled="!canSubmitSession || isActionPending"
                            @click="tabIndex === 0 ? hostGame() : submitJoin()"
                        >
                            <PlayIcon v-if="tabIndex === 0" />
                            <LogInIcon v-else />
                            {{
                                formatMessage(
                                    tabIndex === 0 ? messages.startHosting : messages.joinRoom,
                                )
                            }}
                        </Button>
                    </div>
                </div>
            </Card>

            <Card
                v-else-if="state.status === 'host_scanning' || state.status === 'host_starting'"
                class="!m-0"
            >
                <div class="flex flex-col gap-5">
                    <div class="flex items-center gap-3">
                        <SpinnerIcon class="size-6 shrink-0 animate-spin text-orange" />
                        <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                            {{ statusText }}
                        </h2>
                    </div>
                    <Admonition type="info" :header="formatMessage(messages.host)">
                        {{ formatMessage(messages.lanHint) }}
                    </Admonition>
                    <div class="flex flex-wrap gap-2">
                        <Button type="outlined" :disabled="isActionPending" @click="resetState"
                            ><ArrowLeftIcon />
                            {{ formatMessage(messages.back) }}
                        </Button>
                    </div>
                </div>
            </Card>

            <Card v-else-if="isSessionReady" class="!m-0">
                <div class="flex flex-col gap-5">
                    <div class="flex flex-wrap items-start justify-between gap-3">
                        <div class="flex items-center gap-3">
                            <CheckCircleIcon class="size-7 shrink-0 text-green" />
                            <div>
                                <h2
                                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                                >
                                    {{ statusText }}
                                </h2>
                                <p class="mb-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                                    {{
                                        formatMessage(messages.playersInRoom, {
                                            count: playerCount,
                                        })
                                    }}
                                </p>
                            </div>
                        </div>
                        <TagItem>
                            <UsersIcon v-if="isHostSession" />
                            <LogInIcon v-else />
                            {{
                                formatMessage(
                                    isHostSession ? messages.hostLabel : messages.guestLabel,
                                )
                            }}
                        </TagItem>
                    </div>

                    <div
                        v-if="isHostSession && state.room_code"
                        class="flex flex-wrap items-center justify-between gap-3 rounded-xl bg-surface-2 p-4"
                    >
                        <div class="min-w-0">
                            <div class="font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.roomCode) }}
                            </div>
                            <div class="mt-1 text-sm text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.shareCode) }}
                            </div>
                        </div>
                        <CopyCode :text="state.room_code" />
                    </div>

                    <div
                        v-if="!isHostSession && guestServerAddress"
                        class="flex flex-wrap items-center justify-between gap-3 rounded-xl bg-surface-2 p-4"
                    >
                        <div class="min-w-0">
                            <div class="font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.serverAddress) }}
                            </div>
                        </div>
                        <CopyCode :text="guestServerAddress" />
                    </div>

                    <section class="flex flex-col gap-3">
                        <div class="flex items-center justify-between gap-3">
                            <h3
                                class="m-0 text-base font-semibold text-[var(--color-text-primary)]"
                            >
                                {{ formatMessage(messages.players) }}
                            </h3>
                            <TagItem>
                                <UsersIcon />
                                {{ playerCount }}
                            </TagItem>
                        </div>

                        <div
                            v-if="state.players.length > 0"
                            class="overflow-hidden rounded-xl border border-solid border-surface-5"
                        >
                            <div
                                v-for="(player, index) in state.players"
                                :key="player.machine_id || index"
                                class="flex min-w-0 items-center gap-3 border-0 border-b border-solid border-divider bg-surface-2 px-4 py-3 last:border-b-0"
                            >
                                <div
                                    class="flex size-9 shrink-0 items-center justify-center rounded-full bg-highlight-green text-green"
                                >
                                    <UserIcon class="size-4" />
                                </div>
                                <span
                                    class="min-w-0 flex-1 truncate font-medium text-[var(--color-text-primary)]"
                                >
                                    {{ player.name }}
                                </span>
                                <TagItem>
                                    {{ formatMessage(playerRoleMessage(player.kind)) }}
                                </TagItem>
                            </div>
                        </div>
                        <div
                            v-else
                            class="flex items-center gap-2 rounded-xl bg-surface-2 px-4 py-5 text-[var(--color-text-tertiary)]"
                        >
                            <UsersIcon class="size-5" />
                            {{ formatMessage(messages.noPlayers) }}
                        </div>
                    </section>

                    <div class="flex flex-wrap gap-2">
                        <Button
                            type="outlined"
                            color="red"
                            :disabled="isActionPending"
                            @click="resetState"
                            ><LogOutIcon />
                            {{ formatMessage(messages.disconnect) }}
                        </Button>
                    </div>
                </div>
            </Card>

            <Card
                v-else-if="state.status === 'guest_connecting' || state.status === 'guest_starting'"
                class="!m-0"
            >
                <div class="flex flex-col gap-5">
                    <div class="flex items-center gap-3">
                        <SpinnerIcon class="size-6 shrink-0 animate-spin text-orange" />
                        <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                            {{ statusText }}
                        </h2>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button type="outlined" :disabled="isActionPending" @click="resetState"
                            ><ArrowLeftIcon />
                            {{ formatMessage(messages.back) }}
                        </Button>
                    </div>
                </div>
            </Card>

            <Card v-else-if="state.status === 'error' || state.status === 'fatal'" class="!m-0">
                <Admonition type="critical" :header="errorTypeLabel">
                    {{ state.error_message || formatMessage(messages.checkNetwork) }}
                    <template #actions>
                        <div class="flex flex-wrap gap-2">
                            <Button
                                v-if="isRecoverable"
                                type="outlined"
                                color="red"
                                :disabled="isActionPending || isExportingReport"
                                @click="resetState"
                                ><RefreshCwIcon />
                                {{ formatMessage(messages.retry) }}
                            </Button>
                            <Button
                                type="colored"
                                color="brand"
                                :disabled="isActionPending || isExportingReport"
                                @click="exportTerracottaReport"
                                ><SpinnerIcon v-if="isExportingReport" class="animate-spin" />
                                <DownloadIcon v-else />
                                {{ formatMessage(messages.exportErrorReport) }}
                            </Button>
                        </div>
                    </template>
                </Admonition>
            </Card>

            <Card v-else-if="!isRunning" class="!m-0">
                <div class="flex flex-col gap-5">
                    <Admonition
                        v-if="terracottaUpdate?.update_available"
                        type="info"
                        :header="
                            formatMessage(messages.terracottaUpdateAvailable, {
                                version: terracottaUpdate.latest_version,
                            })
                        "
                    >
                        <template #actions>
                            <Button
                                type="colored"
                                color="brand"
                                :disabled="isActionPending"
                                @click="updateTerracotta"
                                ><DownloadIcon />
                                {{ formatMessage(messages.updateTerracotta) }}
                            </Button>
                        </template>
                    </Admonition>

                    <div class="flex items-start gap-3">
                        <div
                            class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-brand-highlight text-brand"
                        >
                            <UsersIcon class="size-5" />
                        </div>
                        <div class="min-w-0">
                            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.notRunningTitle) }}
                            </h2>
                            <p class="mb-0 mt-1 text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.notRunning) }}
                            </p>
                        </div>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button
                            type="colored"
                            color="brand"
                            :disabled="isActionPending"
                            @click="startTerracotta"
                            ><PlayIcon />
                            {{ formatMessage(messages.startTerracotta) }}
                        </Button>
                    </div>
                </div>
            </Card>

            <div class="mt-auto pt-6 text-center text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.poweredByTerracotta) }}
            </div>
        </template>

        <template v-else>
            <Card v-if="!hongshiState" class="!m-0">
                <div class="flex items-center gap-3">
                    <SpinnerIcon class="size-8 animate-spin text-brand" />
                    <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.loading) }}
                    </h2>
                </div>
            </Card>

            <Card v-else-if="hongshiState.status === 'open'" class="!m-0">
                <div class="flex flex-col gap-5">
                    <div class="flex flex-wrap items-start justify-between gap-3">
                        <div class="flex items-center gap-3">
                            <CheckCircleIcon class="size-7 shrink-0 text-green" />
                            <div>
                                <h2
                                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                                >
                                    {{ formatMessage(messages.statusHostReady) }}
                                </h2>
                                <p class="mb-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                                    {{ formatMessage(messages.publicAddressHint) }}
                                </p>
                            </div>
                        </div>
                        <TagItem>
                            <ServerIcon />
                            {{ hongshiState.node?.name }}
                        </TagItem>
                    </div>

                    <div
                        v-if="hongshiState.public_address"
                        class="flex flex-wrap items-center justify-between gap-3 rounded-xl bg-surface-2 p-4"
                    >
                        <div class="min-w-0">
                            <div class="font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.publicAddress) }}
                            </div>
                            <div class="mt-1 text-sm text-[var(--color-text-tertiary)]">
                                {{ hongshiState.node?.name }} · 127.0.0.1:{{
                                    hongshiState.local_port
                                }}
                            </div>
                        </div>
                        <CopyCode :text="hongshiState.public_address" />
                    </div>

                    <Admonition
                        v-if="hongshiState.port_changed"
                        type="warning"
                        :header="formatMessage(messages.localPort)"
                    >
                        {{ formatMessage(messages.portChanged) }}
                        <template #actions>
                            <Button
                                type="colored"
                                color="orange"
                                :disabled="isActionPending"
                                @click="restartHongshiTunnel"
                                ><RefreshCwIcon />
                                {{ formatMessage(messages.restartTunnel) }}
                            </Button>
                        </template>
                    </Admonition>

                    <div class="flex flex-wrap gap-2">
                        <Button
                            type="outlined"
                            color="red"
                            :disabled="isStopping"
                            @click="stopMultiplayer"
                            ><LogOutIcon />
                            {{ formatMessage(messages.disconnect) }}
                        </Button>
                        <Button type="outlined" :disabled="isActionPending" @click="openHongshiLogs"
                            ><BinaryIcon />
                            {{ formatMessage(messages.openLogs) }}
                        </Button>
                    </div>
                </div>
            </Card>

            <Card v-else-if="isHongshiBusy" class="!m-0">
                <div class="flex flex-col gap-5">
                    <div class="flex items-center gap-3">
                        <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                            {{
                                formatMessage(
                                    hongshiState.status === 'selecting_node'
                                        ? messages.selectingNode
                                        : hongshiState.status === 'downloading'
                                          ? messages.statusDownloading
                                          : messages.creatingTunnel,
                                )
                            }}
                        </h2>
                    </div>
                    <ProgressBar
                        v-if="hongshiState.status === 'downloading'"
                        full-width
                        :progress="hongshiState.download_progress ?? 0"
                        :max="100"
                        :waiting="!hongshiState.download_progress"
                        :label="formatMessage(messages.statusDownloading)"
                        show-progress
                    />
                    <div class="flex flex-wrap gap-2">
                        <Button
                            type="outlined"
                            color="red"
                            :disabled="isStopping"
                            @click="stopMultiplayer"
                            ><LogOutIcon />
                            {{ formatMessage(messages.cancel) }}
                        </Button>
                    </div>
                </div>
            </Card>

            <Card v-else class="!m-0">
                <div class="flex flex-col gap-5">
                    <Admonition
                        v-if="hongshiState.status === 'error'"
                        type="critical"
                        :header="formatMessage(messages.hongshiProvider)"
                    >
                        {{ hongshiFailure }}
                        <template #actions>
                            <Button type="outlined" @click="openHongshiLogs"
                                ><BinaryIcon />
                                {{ formatMessage(messages.openLogs) }}
                            </Button>
                        </template>
                    </Admonition>

                    <Admonition
                        v-else-if="hongshiState.status === 'closed'"
                        type="warning"
                        :header="formatMessage(messages.statusIdle)"
                    >
                        {{ formatMessage(messages.closedTunnel) }}
                    </Admonition>

                    <div>
                        <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.hongshiProvider) }}
                        </h2>
                        <p class="mb-0 mt-1 text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.portHint) }}
                        </p>
                    </div>

                    <div class="grid gap-4 md:grid-cols-2">
                        <div class="flex min-w-0 flex-col gap-2">
                            <span class="font-semibold text-[var(--color-text-primary)]">{{
                                formatMessage(messages.localPort)
                            }}</span>
                            <Combobox
                                v-model="selectedInstanceId"
                                width="full"
                                :options="
                                    detectedPortOptions.map((value) => ({
                                        value,
                                        label: detectedPortLabel(value),
                                    }))
                                "
                                @update:model-value="portSelectionTouched = true"
                            />
                        </div>

                        <div class="flex min-w-0 flex-col gap-2">
                            <span class="font-semibold text-[var(--color-text-primary)]">{{
                                formatMessage(messages.node)
                            }}</span>
                            <Combobox
                                v-model="selectedNodeName"
                                width="full"
                                :options="
                                    nodeOptions.map((value) => ({
                                        value,
                                        label: nodeOptionLabel(value),
                                    }))
                                "
                            />
                        </div>

                        <label
                            v-if="selectedInstanceId === 'manual'"
                            class="flex min-w-0 flex-col gap-2"
                            for="hongshi-local-port"
                        >
                            <span class="font-semibold text-[var(--color-text-primary)]">{{
                                formatMessage(messages.manualPort)
                            }}</span>
                            <StyledInput
                                id="hongshi-local-port"
                                v-model="manualPort"
                                :icon="ServerIcon"
                                inputmode="numeric"
                                placeholder="25565"
                            />
                        </label>
                    </div>

                    <Admonition
                        v-if="!hongshiState.binary_installed"
                        type="info"
                        :header="formatMessage(messages.hongshiProvider)"
                    >
                        {{ formatMessage(messages.hongshiBinaryMissing) }}
                    </Admonition>
                    <Admonition
                        v-if="nodesError || (nodesLoaded && nodes.length === 0)"
                        type="warning"
                        :header="formatMessage(messages.node)"
                    >
                        {{
                            nodesError
                                ? formatMessage(hongshiErrorMessage('node_list'))
                                : formatMessage(messages.noNodes)
                        }}
                    </Admonition>

                    <div class="flex flex-wrap gap-2">
                        <Button
                            type="colored"
                            color="brand"
                            :disabled="
                                !effectiveLocalPort ||
                                isActionPending ||
                                isStopping ||
                                isNodesLoading ||
                                !!(selectedNode && !selectedNode.reachable)
                            "
                            @click="startHongshiTunnel"
                            ><GlobeIcon />
                            {{ formatMessage(messages.createTunnel) }}
                        </Button>
                        <Button
                            type="outlined"
                            :disabled="isNodesLoading"
                            @click="refreshNodes(true)"
                            ><RefreshCwIcon :class="{ 'animate-spin': isNodesLoading }" />
                            {{ formatMessage(messages.refreshNodes) }}
                        </Button>
                    </div>
                </div>
            </Card>

            <div class="mt-auto pt-6 text-center text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.hongshiProvider) }}
            </div>
        </template>
    </div>
</template>
