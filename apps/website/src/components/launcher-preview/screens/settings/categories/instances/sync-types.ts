/**
 * Data shapes of the instance-sync settings, as the desktop app's helpers declare
 * them (apps/app-frontend/src/helpers/instance.ts, synced-options.ts,
 * synced-packs.ts, game-options.ts and worlds.ts).
 */

/* ---- helpers/instance.ts ------------------------------------------------ */

export type SyncedOption =
    | 'game_options'
    | 'command_history'
    | 'multiplayer_servers'
    | 'creative_hotbars'
    | 'screenshots'
    | 'resource_packs'
    | 'data_packs'

export type GlobalSyncedOptions = Record<SyncedOption, boolean>

export const SYNCED_OPTIONS: SyncedOption[] = [
    'game_options',
    'command_history',
    'multiplayer_servers',
    'creative_hotbars',
    'screenshots',
    'resource_packs',
    'data_packs',
]

/** Every synced option is currently available on every instance (helpers/instance.ts). */
export function isSyncedOptionAvailable(_option: SyncedOption): boolean {
    return true
}

export type SyncedServer = {
    id: string
    name: string
    address: string
    accept_textures: boolean | null
}

/* ---- helpers/synced-packs.ts -------------------------------------------- */

export type SyncedPackType = 'resourcepack' | 'datapack'

/** The fields of the `ContentItem` rows `instance_list_synced_packs` returns that the app reads. */
export type SyncedPack = {
    id: string
    file_name: string
    file_path: string
    size: number
    enabled: boolean
    project_type: SyncedPackType
    external: boolean
}

/* ---- helpers/worlds.ts -------------------------------------------------- */

export type ServerStatus = {
    description?: string
    players?: {
        max: number
        online: number
        sample: { name: string; id: string }[]
    }
    version?: {
        name: string
        protocol: number
        legacy: boolean
    }
    favicon?: string
    enforces_secure_chat: boolean
    ping?: number
}

export type ServerData = {
    refreshing: boolean
    lastSuccessfulRefresh?: number
    status?: ServerStatus
    rawMotd?: string
}

export type ServerPackStatus = 'enabled' | 'disabled' | 'prompt'

export type ServerWorld = {
    name: string
    type: 'server'
    index: number
    server_id?: string
    address: string
    pack_status: ServerPackStatus
    display_status: 'normal' | 'hidden' | 'favorite'
}

export type ProtocolVersion = {
    version: number
    legacy: boolean
}

/* ---- helpers/game-options.ts -------------------------------------------- */

export type GameOptionCanonicalValue =
    | { type: 'bool'; value: boolean }
    | { type: 'integer'; value: number }
    | { type: 'decimal'; value: string }
    | { type: 'enum'; value: string }
    | { type: 'text'; value: string }
    | { type: 'string_list'; value: string[] }
    | { type: 'key_binding'; value: string }
    | { type: 'external_raw'; value: string }

export type GameOptionValueState = 'canonical' | 'uniform_local' | 'mixed' | 'unset' | 'invalid'

export type GameOptionCompatibilityStatus =
    | 'ready'
    | 'deferred'
    | 'waiting_for_file'
    | 'waiting_for_base'
    | 'not_available'
    | 'unsupported_value'
    | 'unmappable'
    | 'catalog_uncovered'
    | 'controlled'
    | 'degraded'

export type GameOptionMappingKind = 'direct' | 'legacy' | 'migrated' | 'lossless' | 'lossy'

export type GameOptionCompatibilityReason =
    | 'launcher_controlled'
    | 'catalog_uncovered'
    | 'inspection_failed'
    | 'waiting_for_options_file'
    | 'unsupported_value'
    | 'migrates_on_write'
    | 'waiting_for_compatible_base'
    | 'missing_setting'

export type GameOptionCompatibilityBucket = {
    instance_count: number
    write_keys: string[]
    eventual_keys: string[]
    game_versions: string[]
    status: GameOptionCompatibilityStatus
    mapping?: GameOptionMappingKind | null
    reason?: GameOptionCompatibilityReason | null
}

export type GameOptionCompatibilitySummary = {
    total_participating: number
    will_receive: number
    write_now: number
    left_local: number
    buckets: GameOptionCompatibilityBucket[]
}

export type GameOptionEditorChoice = {
    value: string
}

export type GameOptionEditor = {
    type:
        | 'bool'
        | 'integer'
        | 'decimal'
        | 'enum'
        | 'text'
        | 'string_list'
        | 'key_binding'
        | 'external_raw'
    min?: number | null
    max?: number | null
    step?: number | null
    unit?: string | null
    choices?: GameOptionEditorChoice[]
}

export type GameSettingCategory = {
    id: string
    is_custom?: boolean
}

export type GameOptionValidationError =
    'missing_value' | 'no_compatible_instances' | 'invalid_value' | 'changed_since_opened'

export type EditableGameSetting = {
    option_id: string
    category_id: string
    kind: 'vanilla' | 'external'
    raw_key?: string | null
    sync_enabled: boolean
    canonical_value?: GameOptionCanonicalValue | null
    value_state: GameOptionValueState
    option_revision: number
    editor: GameOptionEditor
    compatibility: GameOptionCompatibilitySummary
    validation_error?: GameOptionValidationError | null
    controlled?: boolean
}

export type GameSettingsEditorState = {
    summary_revision: string
    canonical_revision: number
    catalog_revision: number
    total_participating: number
    categories: GameSettingCategory[]
    settings: EditableGameSetting[]
}

export type GameSettingChange = {
    option_id: string
    base_option_revision: number
    sync_enabled?: boolean
    canonical_value?: GameOptionCanonicalValue | null
}

export type UpdateGameSettingsRequest = {
    expected_summary_revision: string
    expected_canonical_revision: number
    expected_catalog_revision: number
    changes: GameSettingChange[]
}

export type GameOptionsSourceDisabledReason =
    | 'installing_or_updating'
    | 'unsupported_version'
    | 'missing_options_file'
    | 'no_syncable_settings'
    | 'unreadable_options_file'

export type GameOptionsSourceCandidate = {
    source_id: string
    instance_id: string
    name: string
    icon_path?: string | null
    game_version?: string | null
    eligible: boolean
    disabled_reason?: GameOptionsSourceDisabledReason | null
    recognized_setting_count: number
    custom_setting_count: number
}

export type SaveGameSettingsResult = {
    state?: GameSettingsEditorState
    applied: number
    migrated: number
    deferred: number
    unsupported: number
    failed: number
    conflicts?: string[]
}

export type GameSettingSource = {
    instance_id: string
    file_name: string
    file_path: string
    project: {
        id: string
        title: string
        icon_url: string | null
    } | null
}

export type GameSettingLocaleLabel = {
    label: string
    choices: Record<string, string>
    source?: GameSettingSource | null
}

/** The instance a sync-source choice lists (the app's `GameInstance` fields it reads). */
export type SyncInstance = {
    id: string
    name: string
    icon_path?: string | null
    loader: string
    game_version: string
    link?: { type: string } | null
    quarantined?: boolean
    synced_options: GlobalSyncedOptions
}

/**
 * A command error as Tauri hands it to the frontend: the app's
 * `TheseusSerializableError` (apps/app/src/api/mod.rs) serialised as
 * `{ field_name, message }`, which the notification manager stringifies.
 */
export type TheseusError = {
    field_name: 'Theseus'
    message: string
}

export function inputError(message: string): TheseusError {
    return { field_name: 'Theseus', message: `Invalid input: ${message}` }
}
