import { invoke } from './rooms-backend.ts'
import type { TerracottaState } from './terracotta.ts'

export type MultiplayerProvider = 'terracotta' | 'hongshi'
export type HongshiStatus =
    | 'unsupported'
    | 'idle'
    | 'downloading'
    | 'selecting_node'
    | 'starting'
    | 'open'
    | 'closed'
    | 'error'

export type HongshiErrorType =
    | 'unsupported'
    | 'build_unavailable'
    | 'node_list'
    | 'node_unavailable'
    | 'invalid_port'
    | 'install'
    | 'kernel_start'
    | 'kernel_exit'
    | 'kernel_output'
    | 'unknown'

export interface HongshiNode {
    name: string
    address: string
    latency_ms: number | null
    reachable: boolean
    cached: boolean
}

export interface DetectedLanPort {
    instance_id: string
    instance_name: string
    process_id: string
    port: number
    detected_at: string
}

export interface HongshiState {
    supported: boolean
    status: HongshiStatus
    local_port: number | null
    node: HongshiNode | null
    public_address: string | null
    created_at: string | null
    last_exit_code: number | null
    error_type: HongshiErrorType | null
    error_message: string | null
    bound_instance_id: string | null
    port_changed: boolean
    binary_installed: boolean
    download_progress: number | null
}

export interface ProviderCapabilities {
    provider: MultiplayerProvider
    supported: boolean
    can_host: boolean
    can_join: boolean
    requires_local_port: boolean
    unsupported_reason: string | null
}

export interface MultiplayerState {
    active_provider: MultiplayerProvider | null
    providers: ProviderCapabilities[]
    terracotta: TerracottaState
    hongshi: HongshiState
}

export function storedMultiplayerProvider(value: string | null): MultiplayerProvider {
    return value === 'hongshi' ? 'hongshi' : 'terracotta'
}

export function validLocalPort(value: string): number | null {
    if (!/^\d+$/.test(value)) return null
    const port = Number(value)
    return Number.isInteger(port) && port >= 1 && port <= 65535 ? port : null
}

export function selectedDetectedInstance(current: string, ports: DetectedLanPort[]): string {
    if (current !== 'manual' && ports.some((entry) => entry.instance_id === current)) {
        return current
    }
    return ports.length === 1 ? ports[0].instance_id : 'manual'
}

export function selectedNodePreference(current: string, nodes: HongshiNode[]): string {
    return current === 'auto' || nodes.some((node) => node.name === current) ? current : 'auto'
}

export function multiplayerErrorText(error: unknown): string {
    if (
        error &&
        typeof error === 'object' &&
        'message' in error &&
        typeof error.message === 'string'
    ) {
        return error.message
    }
    return String(error)
}

const command = (name: string) => `plugin:multiplayer|${name}`

export const multiplayer = {
    getState: () => invoke<MultiplayerState>(command('multiplayer_get_state')),
    getNodes: (forceRefresh = false) =>
        invoke<HongshiNode[]>(command('multiplayer_get_nodes'), { forceRefresh }),
    getDetectedPorts: () => invoke<DetectedLanPort[]>(command('multiplayer_get_detected_ports')),
    downloadHongshi: () => invoke<void>(command('multiplayer_download_hongshi')),
    switchProvider: (provider: MultiplayerProvider) =>
        invoke<void>(command('multiplayer_switch_provider'), { provider }),
    prepareTerracotta: () => invoke<void>(command('multiplayer_prepare_terracotta')),
    hostTerracotta: (playerName: string) =>
        invoke<void>(command('multiplayer_host'), {
            request: {
                provider: 'terracotta',
                player_name: playerName.trim(),
                room_code: null,
            },
        }),
    joinTerracotta: (playerName: string, roomCode: string) =>
        invoke<void>(command('multiplayer_join'), {
            request: {
                provider: 'terracotta',
                player_name: playerName.trim(),
                room_code: roomCode.trim(),
            },
        }),
    hostHongshi: (localPort: number, nodeName: string | null, instanceId: string | null) =>
        invoke<void>(command('multiplayer_host'), {
            request: {
                provider: 'hongshi',
                local_port: localPort,
                node_name: nodeName,
                instance_id: instanceId,
            },
        }),
    stop: () => invoke<void>(command('multiplayer_stop')),
    reset: () => invoke<void>(command('multiplayer_reset')),
    getPlayerName: () => invoke<string>(command('multiplayer_get_player_name')),
    openHongshiLogs: () => invoke<void>(command('multiplayer_open_hongshi_logs')),
}
