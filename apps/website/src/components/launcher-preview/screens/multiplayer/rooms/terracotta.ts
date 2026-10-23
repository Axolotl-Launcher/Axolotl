import { invoke } from './rooms-backend.ts'

export type TerracottaStatus =
    | 'idle'
    | 'starting'
    | 'downloading'
    | 'waiting'
    | 'host_scanning'
    | 'host_starting'
    | 'host_ready'
    | 'guest_connecting'
    | 'guest_starting'
    | 'guest_ready'
    | 'error'
    | 'fatal'

export type TerracottaDownloadStage =
    'preparing' | 'downloading' | 'verifying' | 'extracting' | 'installing' | 'complete'

export type TerracottaErrorType = 'os' | 'network' | 'install' | 'terracotta' | 'unknown'

export interface TerracottaPlayer {
    machine_id: string
    name: string
    vendor: string
    kind: 'HOST' | 'GUEST' | 'UNKNOWN'
}

export interface TerracottaState {
    status: TerracottaStatus
    http_port: number | null
    room_code: string | null
    server_port: number | null
    players: TerracottaPlayer[]
    download_progress: number | null
    download_stage: TerracottaDownloadStage | null
    binary_installed: boolean
    installed_version: string | null
    error_type: TerracottaErrorType | null
    error_message: string | null
    profile_index: number | null
}

export interface TerracottaUpdate {
    installed_version: string | null
    latest_version: string
    update_available: boolean
}

const TERRACOTTA_ROOM_CODE_PATTERN = /^U\/[A-Z0-9]{4}(?:-[A-Z0-9]{4}){3}$/i
const TERRACOTTA_PUBLIC_NODE_SCHEMES = new Set([
    'http:',
    'https:',
    'tcp:',
    'tls:',
    'udp:',
    'ws:',
    'wss:',
])

const command = (name: string) => `plugin:terracotta|${name}`

export function isValidTerracottaRoomCode(roomCode: string): boolean {
    return TERRACOTTA_ROOM_CODE_PATTERN.test(roomCode.trim())
}

export function parseTerracottaPublicNodes(value: string): {
    nodes: string[]
    invalidNode: string | null
} {
    const nodes = value
        .split(/[\n,]+/)
        .map((node) => node.trim())
        .filter(Boolean)

    for (const node of nodes) {
        try {
            const url = new URL(node)
            if (!TERRACOTTA_PUBLIC_NODE_SCHEMES.has(url.protocol) || !url.hostname) {
                return { nodes, invalidNode: node }
            }
        } catch {
            return { nodes, invalidNode: node }
        }
    }

    return { nodes, invalidNode: null }
}

export const terracotta = {
    getState: () => invoke<TerracottaState>(command('terracotta_get_state')),
    getPlatformKey: () => invoke<string>(command('terracotta_get_platform_key')),
    checkForUpdate: () => invoke<TerracottaUpdate>(command('terracotta_check_for_update')),
    getPlayerName: () => invoke<string>(command('terracotta_get_player_name')),
    getDiagnosticReport: () => invoke<string>(command('terracotta_get_diagnostic_report')),
    start: () => invoke<void>(command('terracotta_start'), { autoDownload: true }),
    host: (playerName: string) =>
        invoke<void>(command('terracotta_host'), { playerName: playerName.trim() }),
    join: (playerName: string, roomCode: string) =>
        invoke<void>(command('terracotta_join'), {
            playerName: playerName.trim(),
            roomCode: roomCode.trim(),
        }),
    reset: () => invoke<void>(command('terracotta_reset')),
    download: () => invoke<void>(command('terracotta_download')),
    update: () => invoke<TerracottaUpdate>(command('terracotta_update')),
}
