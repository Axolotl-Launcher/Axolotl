import { formatPreviewMessage } from '../../../lib/i18n'
import { currentPreview, previewDomain, type PreviewState } from '../../../lib/state'
import { serverPreviewMessages } from '../server-preview-messages.ts'
import type { MultiplayerProvider, MultiplayerState } from './multiplayer.ts'
import type { TerracottaState } from './terracotta.ts'

/**
 * The `multiplayer` and `terracotta` plugin commands for the preview. Rooms
 * connect players through the Terracotta and RedStone Online 2 programs, which
 * the desktop app downloads and runs; a page can do neither, so the state is
 * the app's state before those programs are installed and every action that
 * would download or run them answers with the desktop app's notice. Relay
 * nodes and LAN worlds are probed on the player's computer, so their lists
 * stay empty here.
 */

/** Raised for what only the desktop app can do; the session shows its message as is. */
export class DesktopOnlyRoomsError extends Error {}

function desktopOnly(): never {
    throw new DesktopOnlyRoomsError(formatPreviewMessage(serverPreviewMessages.roomsDesktopOnly))
}

/** `get_latest_terracotta_version` (gitee mirror of burningtnt/Terracotta), fetched 2026-10-23. */
const TERRACOTTA_LATEST = '0.4.2'

function idleTerracotta(): TerracottaState {
    return {
        status: 'idle',
        http_port: null,
        room_code: null,
        server_port: null,
        players: [],
        download_progress: null,
        download_stage: null,
        binary_installed: false,
        installed_version: null,
        error_type: null,
        error_message: null,
        profile_index: null,
    }
}

function createRooms(_state: PreviewState) {
    return {
        value: { activeProvider: null as MultiplayerProvider | null },
    }
}

function rooms() {
    return previewDomain(currentPreview(), 'multiplayer:rooms', createRooms)
}

function state(): MultiplayerState {
    return {
        active_provider: rooms().activeProvider,
        providers: [
            {
                provider: 'terracotta',
                supported: true,
                can_host: true,
                can_join: true,
                requires_local_port: false,
                unsupported_reason: null,
            },
            {
                provider: 'hongshi',
                supported: true,
                can_host: true,
                can_join: false,
                requires_local_port: true,
                unsupported_reason: null,
            },
        ],
        terracotta: idleTerracotta(),
        hongshi: {
            supported: true,
            status: 'idle',
            local_port: null,
            node: null,
            public_address: null,
            created_at: null,
            last_exit_code: null,
            error_type: null,
            error_message: null,
            bound_instance_id: null,
            port_changed: false,
            binary_installed: false,
            download_progress: null,
        },
    } as MultiplayerState
}

function playerName() {
    const accounts = currentPreview().accounts
    return accounts.selected.value?.profile.name ?? ''
}

/** `invoke` of `@tauri-apps/api/core` for the rooms helpers. */
export async function invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
    const name = command.replace(/^plugin:(multiplayer|terracotta)\|/, '')
    switch (name) {
        case 'multiplayer_get_state':
            return state() as T
        case 'terracotta_get_state':
            return idleTerracotta() as T
        case 'multiplayer_get_nodes':
        case 'multiplayer_get_detected_ports':
            return [] as T
        case 'multiplayer_switch_provider':
            rooms().activeProvider = args.provider as MultiplayerProvider
            return undefined as T
        case 'multiplayer_stop':
        case 'multiplayer_reset':
        case 'terracotta_reset':
            rooms().activeProvider = null
            return undefined as T
        case 'multiplayer_get_player_name':
        case 'terracotta_get_player_name':
            return playerName() as T
        case 'terracotta_get_platform_key':
            return 'macos-arm64' as T
        case 'terracotta_check_for_update':
            return {
                installed_version: null,
                latest_version: TERRACOTTA_LATEST,
                update_available: true,
            } as T
        default:
            return desktopOnly()
    }
}
