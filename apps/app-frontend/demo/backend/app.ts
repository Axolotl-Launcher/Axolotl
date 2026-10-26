import tauriConf from '../../../app/tauri.conf.json'
import type { Handlers } from './types'

const ignore = () => null

/** Process, window and update commands of the desktop shell. */
export const appHandlers: Handlers = {
    'plugin:app|version': () => tauriConf.version,
    'plugin:app|name': () => tauriConf.productName,
    initialize_state: ignore,
    show_window: ignore,
    is_elevated: () => false,
    lightweight_mode_frontend_ready: () => ({ pending_crashes: [], pending_commands: [] }),
    lightweight_mode_set_route: ignore,
    update_traffic_lights_anchor: ignore,
    'plugin:utils|should_disable_mouseover': () => false,
    // The demo never updates itself: it is always the version it was built from.
    get_update_channel: () => 'release',
    are_updates_enabled: () => false,
    'plugin:curseforge|curseforge_configure_manual_download_watcher': ignore,
    // No other launcher's folders exist to link in the browser.
    'plugin:instance|instance_sync_direct_links': () => ({
        imported: 0,
        updated: 0,
        removed: 0,
        missing: 0,
        errors: [],
    }),
    // app-lib's defaults (api/logs/logshare.rs, crash_analysis_ai_settings row 0).
    'plugin:logs|logs_get_log_share_settings': () => ({
        share_provider: 'logshare',
        ai_source: 'logshare',
        auto_upload: true,
        multi_file: true,
        no_storage: false,
        show_progress: true,
    }),
    'plugin:logs|logs_get_crash_analysis_ai_settings': () => ({
        enabled: false,
        provider_id: '',
        model_id: '',
        ai_source: 'logshare',
    }),
}
