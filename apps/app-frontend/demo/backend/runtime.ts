import type { Handlers } from './types'

const ignore = () => null

/** Processes, downloads, window chrome and other desktop runtime state. */
export const runtimeHandlers: Handlers = {
    is_dev: () => false,
    'plugin:utils|get_os': () => 'MacOS',
    'plugin:utils|progress_bars_list': () => ({}),
    'plugin:utils|get_opening_command': ignore,
    'plugin:process|process_get_all': () => [],
    'plugin:install|download_job_list': () => ({ jobs: [], nextCursor: null }),
    'plugin:ai|ai_get_state': () => ({
        settings: { enabled: false },
        catalog_source: 'builtin',
        providers: [],
    }),
    'plugin:mcp|mcp_get_state': () => ({
        revision: 0,
        enabled: false,
        running: false,
        full_access: false,
        require_auth: false,
        endpoint: null,
        connection: null,
        token: null,
        error: null,
    }),
    'plugin:settings|proxy_get': () => ({ mode: 'none', url: '', username: '', password: '' }),
    'plugin:system-accent|system_accent_color': ignore,
    set_discord_activity: ignore,
    set_transparent_window_frame: ignore,
    'plugin:window|set_decorations': ignore,
    'plugin:window|set_effects': ignore,
    'plugin:window|is_maximized': () => false,
    'plugin:window|is_fullscreen': () => false,
}
