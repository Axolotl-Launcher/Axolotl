import type { MultiplayerState } from '@/helpers/multiplayer'
import type { TerracottaUpdate } from '@/helpers/terracotta'

import { desktopOnly } from './settings'
import type { Handlers } from './types'

/**
 * Rooms connect players through a tunnel the browser cannot open: the page shows
 * Terracotta ready and idle, and hosting or joining asks for the desktop app.
 */
const state: MultiplayerState = {
    active_provider: 'terracotta',
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
            supported: false,
            can_host: false,
            can_join: false,
            requires_local_port: true,
            unsupported_reason: null,
        },
    ],
    terracotta: {
        status: 'idle',
        http_port: null,
        room_code: null,
        server_port: null,
        players: [],
        download_progress: null,
        download_stage: null,
        binary_installed: true,
        installed_version: null,
        error_type: null,
        error_message: null,
        profile_index: null,
    },
    hongshi: {
        supported: false,
        status: 'unsupported',
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
}

const needsDesktop = () => {
    throw desktopOnly()
}

export const roomHandlers: Handlers = {
    'plugin:multiplayer|multiplayer_get_state': () => state,
    'plugin:multiplayer|multiplayer_get_detected_ports': () => [],
    'plugin:multiplayer|multiplayer_get_nodes': () => [],
    'plugin:multiplayer|multiplayer_get_player_name': () => 'Steve',
    'plugin:terracotta|terracotta_get_platform_key': () => 'macos-aarch64',
    'plugin:terracotta|terracotta_check_for_update': (): TerracottaUpdate => ({
        installed_version: null,
        latest_version: '',
        update_available: false,
    }),
    'plugin:multiplayer|multiplayer_host': needsDesktop,
    'plugin:multiplayer|multiplayer_join': needsDesktop,
    'plugin:multiplayer|multiplayer_prepare_terracotta': needsDesktop,
    'plugin:terracotta|terracotta_start': needsDesktop,
    'plugin:terracotta|terracotta_host': needsDesktop,
    'plugin:terracotta|terracotta_join': needsDesktop,
}
