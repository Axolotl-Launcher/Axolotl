import type { GameOptionCanonicalValue, GameOptionEditor } from './sync-types'

/**
 * The Minecraft settings the launcher knows how to read and write
 * (packages/app-lib/src/api/instance/synced_options/game_options/catalog): the
 * same ids, categories, "synced by default" flags, editors and the range of
 * versions whose options.txt carries each key, in the catalog's own order.
 */

/** `CATALOG_REVISION` (game_options/mod.rs). */
export const CATALOG_REVISION = 6

type Editor =
    | { kind: 'bool' }
    | { kind: 'integer'; min: number; max: number; step: number }
    | { kind: 'unbounded_integer' }
    | { kind: 'decimal'; min: number; max: number; step: number; unit: string | null }
    | { kind: 'unbounded_decimal' }
    | { kind: 'enum'; choices: readonly string[] }
    | { kind: 'language' }
    | { kind: 'key_binding' }

export interface SupportedSetting {
    id: string
    category: string
    defaultOn: boolean
    editor: Editor
    since: string
    until: string
}

const BOOL: Editor = { kind: 'bool' }
const UNBOUNDED_INTEGER: Editor = { kind: 'unbounded_integer' }
const UNBOUNDED_DECIMAL: Editor = { kind: 'unbounded_decimal' }
const UNIT_INTERVAL: Editor = { kind: 'decimal', min: 0, max: 1, step: 0.01, unit: 'percent' }
const KEY: Editor = { kind: 'key_binding' }
const int = (min: number, max: number, step: number): Editor => ({
    kind: 'integer',
    min,
    max,
    step,
})
const dec = (min: number, max: number, step: number, unit: string | null): Editor => ({
    kind: 'decimal',
    min,
    max,
    step,
    unit,
})
const choice = (...choices: string[]): Editor => ({ kind: 'enum', choices })

function s(
    id: string,
    category: string,
    defaultOn: boolean,
    editor: Editor,
    since = '1.0',
    until = '26.3',
): SupportedSetting {
    return { id, category, defaultOn, editor, since, until }
}

const key = (id: string, since: string, until = '26.3') =>
    s(id, 'controls', true, KEY, since, until)

/** Versioned settings carry the combined range of their physical keys. */
const SETTINGS: SupportedSetting[] = [
    // video.rs
    s('fov', 'video', true, int(30, 110, 1)),
    s('graphics', 'video', true, choice('fast', 'fancy', 'fabulous', 'custom')),
    s('ambient_occlusion', 'video', true, choice('off', 'on', 'minimum', 'maximum')),
    s('brightness', 'video', true, UNIT_INTERVAL),
    s('render_distance', 'video', false, int(2, 64, 1), '1.7.2'),
    s('legacy_view_distance', 'video', false, choice('0', '1', '2', '3'), '1.0', '1.6.4'),
    s('simulation_distance', 'video', false, int(2, 64, 1), '1.18'),
    s('entity_distance', 'video', false, dec(0.5, 5, 0.25, 'multiplier'), '1.16'),
    s('gui_scale', 'video', true, int(0, 8, 1)),
    s('debug_gui_scale', 'video', true, UNBOUNDED_INTEGER, '26.3'),
    s('graphics_backend', 'video', false, choice('default', 'opengl', 'vulkan'), '26.2'),
    s('particles', 'video', true, choice('0', '1', '2')),
    s('clouds', 'video', true, choice('false', 'fast', 'true')),
    s('cloud_range', 'video', false, UNBOUNDED_INTEGER, '1.21.6'),
    s('entity_shadows', 'video', true, BOOL, '1.8.1'),
    s('view_bobbing', 'video', true, BOOL),
    s('vsync', 'video', true, BOOL, '1.3.1'),
    s('fullscreen', 'video', true, BOOL, '1.3.1'),
    s('exclusive_fullscreen', 'video', false, BOOL, '26.1'),
    s('mac_fullscreen_menu', 'video', false, BOOL, '26.3'),
    s('max_framerate', 'video', true, int(10, 260, 10), '1.7.2'),
    s('legacy_framerate_limit', 'video', false, choice('0', '1', '2'), '1.0', '1.6.4'),
    s('inactivity_framerate_limit', 'video', true, choice('afk', 'minimized'), '1.21.2'),
    s('mipmap_levels', 'video', false, int(0, 4, 1), '1.7.2'),
    s('biome_blend_radius', 'video', true, int(0, 7, 1), '1.13'),
    s('prioritize_chunk_updates', 'video', true, choice('0', '1', '2'), '1.18'),
    s('attack_indicator', 'video', true, choice('0', '1', '2'), '1.9'),
    s('reduced_debug_info', 'video', true, BOOL, '1.8'),
    s('chunk_fade_time', 'video', false, UNBOUNDED_DECIMAL, '1.21.11'),
    s('cutout_leaves', 'video', true, BOOL, '1.21.11'),
    s('improved_transparency', 'video', true, BOOL, '1.21.11'),
    s('texture_filtering', 'video', false, choice('0', '1', '2'), '1.21.11'),
    s('anisotropy', 'video', false, UNBOUNDED_INTEGER, '1.21.11'),
    s('vignette', 'video', true, BOOL, '1.21.11'),
    s('weather_radius', 'video', false, UNBOUNDED_INTEGER, '1.21.11'),
    s('advanced_opengl', 'video', false, BOOL, '1.0', '1.7.10'),
    s('anaglyph_3d', 'video', true, BOOL, '1.0', '1.12.2'),
    s('anisotropic_filtering', 'video', false, UNBOUNDED_INTEGER, '1.7.2', '1.7.10'),
    s('alternate_blocks', 'video', true, BOOL, '1.8', '1.8.9'),
    s('held_item_tooltips', 'video', true, BOOL, '1.4.6', '1.19.3'),
    s('use_vbo', 'video', false, BOOL, '1.8', '1.13.2'),
    // sound.rs
    s('language', 'language', true, { kind: 'language' }, '1.1'),
    s('force_unicode_font', 'language', true, BOOL, '1.7.2'),
    s('japanese_glyph_variants', 'language', true, BOOL, '1.20.5'),
    s('directional_audio', 'music_and_sound', true, BOOL, '1.19'),
    s(
        'music_toast',
        'music_and_sound',
        true,
        choice('never', 'pause', 'pause_and_toast'),
        '1.21.6',
    ),
    s(
        'music_frequency',
        'music_and_sound',
        true,
        choice('CONSTANT', 'DEFAULT', 'FREQUENT'),
        '1.21.6',
    ),
    s('master_volume', 'music_and_sound', true, UNIT_INTERVAL),
    s('music_volume', 'music_and_sound', true, UNIT_INTERVAL),
    s('record_volume', 'music_and_sound', true, UNIT_INTERVAL, '1.7.2'),
    s('weather_volume', 'music_and_sound', true, UNIT_INTERVAL, '1.7.2'),
    s('blocks_volume', 'music_and_sound', true, UNIT_INTERVAL, '1.7.2'),
    s('hostile_volume', 'music_and_sound', true, UNIT_INTERVAL, '1.7.2'),
    s('neutral_volume', 'music_and_sound', true, UNIT_INTERVAL, '1.7.2'),
    s('players_volume', 'music_and_sound', true, UNIT_INTERVAL, '1.7.2'),
    s('ambient_volume', 'music_and_sound', true, UNIT_INTERVAL, '1.7.2'),
    s('voice_volume', 'music_and_sound', true, UNIT_INTERVAL, '1.9'),
    s('ui_volume', 'music_and_sound', true, UNIT_INTERVAL, '1.21.6'),
    // controls.rs
    s('sensitivity', 'controls', true, dec(0, 1, 0.005, 'percent')),
    s('invert_mouse', 'controls', true, BOOL),
    s('invert_horizontal_mouse', 'controls', true, BOOL, '1.21.9'),
    s('auto_jump', 'controls', true, BOOL, '1.10'),
    s('toggle_crouch', 'controls', true, BOOL, '1.15'),
    s('toggle_sprint', 'controls', true, BOOL, '1.15'),
    s('toggle_attack', 'controls', true, BOOL, '1.21.9'),
    s('toggle_use', 'controls', true, BOOL, '1.21.9'),
    s('discrete_mouse_scroll', 'controls', true, BOOL, '1.14'),
    s('mouse_wheel_sensitivity', 'controls', true, UNBOUNDED_DECIMAL, '1.13'),
    s('raw_mouse_input', 'controls', false, BOOL, '1.14.4', '26.2'),
    s('touchscreen', 'controls', false, BOOL, '1.4.4', '26.1.2'),
    s('allow_cursor_changes', 'controls', true, BOOL, '1.21.9'),
    s('sprint_window', 'controls', true, UNBOUNDED_INTEGER, '1.21.9'),
    s('operator_items_tab', 'controls', true, BOOL, '1.19.3'),
    s('ctrl_click_right_click', 'controls', false, BOOL, '26.3'),
    s('quit_shortcuts', 'controls', false, BOOL, '26.3'),
    key('key.forward', '1.0'),
    key('key.left', '1.0'),
    key('key.back', '1.0'),
    key('key.right', '1.0'),
    key('key.jump', '1.0'),
    key('key.sneak', '1.0'),
    key('key.sprint', '1.7.2'),
    key('key.inventory', '1.0'),
    key('key.swap_offhand', '1.9'),
    key('key.drop', '1.0'),
    key('key.use', '1.0'),
    key('key.attack', '1.0'),
    key('key.pick_item', '1.0'),
    key('key.chat', '1.0'),
    key('key.player_list', '1.0'),
    key('key.command', '1.3.1'),
    key('key.screenshot', '1.7.2'),
    key('key.perspective', '1.7.2'),
    key('key.fullscreen', '1.7.3'),
    key('key.advancements', '1.12'),
    key('key.smooth_camera', '1.7.2'),
    key('key.spectator_outlines', '1.8'),
    key('key.save_toolbar', '1.12'),
    key('key.load_toolbar', '1.12'),
    key('key.social_interactions', '1.16.4'),
    key('key.quick_actions', '1.21.6'),
    key('key.spectator_hotbar', '1.21.9'),
    key('key.friends', '26.2'),
    key('key.toggle_gui', '1.21.11'),
    key('key.toggle_spectator_shader', '1.21.11'),
    key('key.hotbar.1', '1.7.2'),
    key('key.hotbar.2', '1.7.2'),
    key('key.hotbar.3', '1.7.2'),
    key('key.hotbar.4', '1.7.2'),
    key('key.hotbar.5', '1.7.2'),
    key('key.hotbar.6', '1.7.2'),
    key('key.hotbar.7', '1.7.2'),
    key('key.hotbar.8', '1.7.2'),
    key('key.hotbar.9', '1.7.2'),
    key('key.debug.overlay', '1.21.11'),
    key('key.debug.modifier', '1.21.11'),
    key('key.debug.reload_chunks', '1.21.11'),
    key('key.debug.hitboxes', '1.21.11'),
    key('key.debug.clear_chat', '1.21.11'),
    key('key.debug.crash', '1.21.11'),
    key('key.debug.chunk_borders', '1.21.11'),
    key('key.debug.advanced_tooltips', '1.21.11'),
    key('key.debug.copy_recreate_command', '1.21.11'),
    key('key.debug.spectate', '1.21.11'),
    key('key.debug.switch_game_mode', '1.21.11'),
    key('key.debug.options', '1.21.11'),
    key('key.debug.focus_pause', '1.21.11'),
    key('key.debug.dump_dynamic_textures', '1.21.11'),
    key('key.debug.reload_resource_packs', '1.21.11'),
    key('key.debug.profiling', '1.21.11'),
    key('key.debug.copy_location', '1.21.11'),
    key('key.debug.dump_version', '1.21.11'),
    key('key.debug.profiling_chart', '1.21.11'),
    key('key.debug.fps_charts', '1.21.11'),
    key('key.debug.network_charts', '1.21.11'),
    key('key.debug.lightmap_texture', '26.1'),
    key('key.debug.improved_transparency', '26.3'),
    // chat.rs
    s('chat_visibility', 'chat', true, choice('0', '1', '2'), '1.3.1'),
    s('chat_colors', 'chat', true, BOOL, '1.3.1'),
    s('chat_links', 'chat', true, BOOL, '1.3.1'),
    s('chat_links_prompt', 'chat', true, BOOL, '1.3.1'),
    s('chat_opacity', 'chat', true, UNIT_INTERVAL, '1.3.1'),
    s('chat_scale', 'chat', true, UNIT_INTERVAL, '1.5.1'),
    s('chat_width', 'chat', true, UNIT_INTERVAL, '1.5.1'),
    s('focused_chat_height', 'chat', true, UNIT_INTERVAL, '1.5.1'),
    s('unfocused_chat_height', 'chat', true, UNIT_INTERVAL, '1.5.1'),
    s('chat_line_spacing', 'chat', true, UNIT_INTERVAL, '1.16'),
    s('chat_delay', 'chat', true, dec(0, 6, 0.1, 'seconds'), '1.16'),
    s('text_background_opacity', 'chat', true, UNIT_INTERVAL, '1.14'),
    s('chat_background_only', 'chat', true, BOOL, '1.14'),
    s('auto_suggestions', 'chat', true, BOOL, '1.13'),
    s('secure_chat_only', 'chat', true, BOOL, '1.19'),
    s('save_chat_drafts', 'chat', true, BOOL, '1.21.9'),
    s('hide_matched_names', 'chat', true, BOOL, '1.16.4'),
    s('chat_preview', 'chat', true, choice('0', '1', '2'), '1.19', '1.19.2'),
    // accessibility.rs
    s('narrator', 'accessibility', true, choice('0', '1', '2', '3'), '1.12'),
    s('subtitles', 'accessibility', true, BOOL, '1.9'),
    s('high_contrast', 'accessibility', true, BOOL, '1.19.4'),
    s('dark_splash', 'accessibility', true, BOOL, '1.17'),
    s('notification_time', 'accessibility', true, dec(0.5, 10, 0.5, 'multiplier'), '1.19.4'),
    s('fov_effects', 'accessibility', true, UNIT_INTERVAL, '1.16.2'),
    s('screen_effects', 'accessibility', true, UNIT_INTERVAL, '1.16.2'),
    s('darkness_pulsing', 'accessibility', true, UNIT_INTERVAL, '1.19'),
    s('damage_tilt', 'accessibility', true, UNIT_INTERVAL, '1.19.4'),
    s('glint_speed', 'accessibility', true, UNIT_INTERVAL, '1.19.4'),
    s('glint_strength', 'accessibility', true, UNIT_INTERVAL, '1.19.4'),
    s('hide_lightning_flashes', 'accessibility', true, BOOL, '1.18'),
    s('hide_splash_texts', 'accessibility', true, BOOL, '1.20.3'),
    s('high_contrast_outline', 'accessibility', true, BOOL, '1.21.2'),
    s('narrator_hotkey', 'accessibility', true, BOOL, '1.20.2'),
    s('autosave_indicator', 'accessibility', true, BOOL, '1.18'),
    s('panorama_speed', 'accessibility', true, UNIT_INTERVAL, '1.19.3'),
    s('menu_background_blur', 'accessibility', true, int(0, 10, 1), '1.20.5'),
    s('rotate_with_minecart', 'accessibility', true, BOOL, '1.21.2'),
    // catalog/mod.rs GENERAL_SETTINGS
    s('main_hand', 'skin_customization', true, choice('left', 'right'), '1.9'),
    s('cape', 'skin_customization', true, BOOL, '1.4.2'),
    s('hat', 'skin_customization', true, BOOL, '1.8'),
    s('jacket', 'skin_customization', true, BOOL, '1.8'),
    s('left_sleeve', 'skin_customization', true, BOOL, '1.8'),
    s('right_sleeve', 'skin_customization', true, BOOL, '1.8'),
    s('left_pants_leg', 'skin_customization', true, BOOL, '1.8'),
    s('right_pants_leg', 'skin_customization', true, BOOL, '1.8'),
    s('allow_server_listing', 'online', true, BOOL, '1.18'),
    s('realms_notifications', 'online', true, BOOL, '1.8.9'),
    s('hide_server_address', 'online', true, BOOL, '1.3.2'),
    s('server_textures', 'online', true, BOOL, '1.3.1', '1.7.5'),
    s('snooper', 'online', true, BOOL, '1.3.1', '1.17.1'),
    s('extra_telemetry', 'online', true, BOOL, '1.19.3'),
    s('in_game_notifications', 'online', true, BOOL, '26.2'),
    s('share_presence', 'online', true, choice('all', 'limited', 'none'), '26.2'),
]

const SETTINGS_BY_ID = new Map(SETTINGS.map((setting) => [setting.id, setting]))

/** `all_supported_settings()`, in catalog order. */
export function allSupportedSettings(): readonly SupportedSetting[] {
    return SETTINGS
}

/** `setting_by_id()`. */
export function settingById(id: string): SupportedSetting | undefined {
    return SETTINGS_BY_ID.get(id)
}

/** The editor categories in the order `load_settings_editor` emits them. */
export const CATEGORY_ORDER = [
    'skin_customization',
    'video',
    'language',
    'music_and_sound',
    'controls',
    'chat',
    'accessibility',
    'online',
    'custom',
] as const

/** `release_version()`: numeric parts of a release id, or null for snapshots and betas. */
function releaseVersion(version: string): number[] | null {
    if (!/^\d+(\.\d+)*$/.test(version)) return null
    return version.split('.').map(Number)
}

function compareVersions(left: number[], right: number[]): number {
    for (let index = 0; index < Math.max(left.length, right.length); index++) {
        const difference = (left[index] ?? 0) - (right[index] ?? 0)
        if (difference !== 0) return difference
    }
    return 0
}

/** `supported_settings_cover_game_version()`: releases from 1.0 to the catalog's newest. */
export function catalogCoversGameVersion(version: string): boolean {
    const parsed = releaseVersion(version)
    if (!parsed) return false
    return compareVersions(parsed, [1, 0]) >= 0 && compareVersions(parsed, [26, 3]) <= 0
}

/** `setting_available_for_version()`. */
export function settingAvailableForVersion(setting: SupportedSetting, version: string): boolean {
    const parsed = releaseVersion(version)
    const since = releaseVersion(setting.since)
    const until = releaseVersion(setting.until)
    if (!parsed || !since || !until) return false
    return compareVersions(since, parsed) <= 0 && compareVersions(parsed, until) <= 0
}

function versionAtLeast(version: string, minimum: string): boolean {
    const parsed = releaseVersion(version)
    const bound = releaseVersion(minimum)
    return !!parsed && !!bound && compareVersions(parsed, bound) >= 0
}

/**
 * `encode_value(...).is_some()`: whether a version's options.txt can hold the
 * value. The keys whose format changed between versions limit the choices an
 * older file can store (graphics presets, the boolean `ao`, fast clouds).
 */
export function valueRepresentable(
    setting: SupportedSetting,
    value: GameOptionCanonicalValue,
    version: string,
): boolean {
    if (value.type !== 'enum') return true
    switch (setting.id) {
        case 'graphics':
            if (value.value === 'custom') return versionAtLeast(version, '1.21.11')
            if (value.value === 'fabulous') return versionAtLeast(version, '1.16')
            return true
        case 'ambient_occlusion':
            if (versionAtLeast(version, '1.19.3'))
                return value.value === 'off' || value.value === 'on'
            if (versionAtLeast(version, '1.5.1')) return value.value !== 'on'
            return value.value === 'off' || value.value === 'on'
        case 'clouds':
            return value.value !== 'fast' || versionAtLeast(version, '1.8.1')
        default:
            return true
    }
}

/** `editor_for()`. */
export function editorFor(setting: SupportedSetting): GameOptionEditor {
    const editor = setting.editor
    switch (editor.kind) {
        case 'bool':
            return { type: 'bool', min: null, max: null, step: null, unit: null, choices: [] }
        case 'unbounded_integer':
            return { type: 'integer', min: null, max: null, step: 1, unit: null, choices: [] }
        case 'integer':
            return {
                type: 'integer',
                min: editor.min,
                max: editor.max,
                step: editor.step,
                unit: null,
                choices: [],
            }
        case 'unbounded_decimal':
            return { type: 'decimal', min: null, max: null, step: 0.01, unit: null, choices: [] }
        case 'decimal':
            return {
                type: 'decimal',
                min: editor.min,
                max: editor.max,
                step: editor.step,
                unit: editor.unit,
                choices: [],
            }
        case 'enum':
            return {
                type: 'enum',
                min: null,
                max: null,
                step: null,
                unit: null,
                choices: editor.choices.map((value) => ({ value })),
            }
        case 'language':
            return { type: 'text', min: null, max: null, step: null, unit: null, choices: [] }
        case 'key_binding':
            return {
                type: 'key_binding',
                min: null,
                max: null,
                step: null,
                unit: null,
                choices: [],
            }
    }
}

/** `valid_modern_key_binding()`: `key.keyboard.*`, `key.mouse.*` or `scancode.*`, plus modifiers. */
function validModernKeyBinding(value: string): boolean {
    const [key, ...modifiers] = value.split(':')
    if (!/^(key\.keyboard\.[a-z0-9.]+|key\.mouse\.[a-z0-9]+|scancode\.\d+)$/.test(key)) return false
    return modifiers.every((modifier) => /^[A-Za-z]+([+][A-Za-z]+)*$/.test(modifier))
}

/** `validate_canonical_value()` (catalog/version_changes.rs `validate_value`, editor increments enforced). */
export function validateCanonicalValue(
    setting: SupportedSetting | undefined,
    value: GameOptionCanonicalValue,
): string | null {
    if (!setting) {
        if (value.type !== 'external_raw') return 'Custom settings require a raw string value'
        return /[\r\n]/.test(value.value) ? 'Invalid value' : null
    }
    const editor = setting.editor
    const typeValid = (() => {
        switch (editor.kind) {
            case 'bool':
                return value.type === 'bool'
            case 'integer':
            case 'unbounded_integer':
                return value.type === 'integer'
            case 'decimal':
            case 'unbounded_decimal':
                return value.type === 'decimal' || value.type === 'integer'
            case 'enum':
                return value.type === 'enum'
            case 'language':
                return value.type === 'text'
            case 'key_binding':
                return value.type === 'key_binding'
        }
    })()
    if (!typeValid) return `Invalid value type for ${setting.id}`
    const outOfRange = `${setting.id} is outside its supported range or increment`
    if (editor.kind === 'unbounded_decimal' && value.type === 'decimal') {
        const parsed = Number.parseFloat(value.value)
        if (!Number.isFinite(parsed)) return 'Invalid decimal value'
    }
    if (editor.kind === 'integer' && value.type === 'integer') {
        if (
            value.value < editor.min ||
            value.value > editor.max ||
            (editor.step > 0 && (value.value - editor.min) % editor.step !== 0)
        ) {
            return outOfRange
        }
    }
    if (editor.kind === 'decimal' && (value.type === 'decimal' || value.type === 'integer')) {
        const parsed = value.type === 'decimal' ? Number.parseFloat(value.value) : value.value
        if (!Number.isFinite(parsed) && value.type === 'decimal') return 'Invalid decimal value'
        const increment = (parsed - editor.min) / editor.step
        if (
            !Number.isFinite(parsed) ||
            parsed < editor.min ||
            parsed > editor.max ||
            (editor.step > 0 && Math.abs(increment - Math.round(increment)) > 0.000_001)
        ) {
            return outOfRange
        }
    }
    if (editor.kind === 'enum' && value.type === 'enum' && !editor.choices.includes(value.value)) {
        return `Invalid choice for ${setting.id}`
    }
    if (
        editor.kind === 'language' &&
        value.type === 'text' &&
        (value.value.length === 0 || value.value.length > 64 || /\p{Cc}/u.test(value.value))
    ) {
        return 'Invalid language value'
    }
    if (
        editor.kind === 'key_binding' &&
        value.type === 'key_binding' &&
        (value.value.length > 256 ||
            /[\r\n]/.test(value.value) ||
            !validModernKeyBinding(value.value))
    ) {
        return 'Invalid key binding value'
    }
    return null
}

export function canonicalValuesEqual(
    left: GameOptionCanonicalValue | null | undefined,
    right: GameOptionCanonicalValue | null | undefined,
): boolean {
    if (!left || !right) return !left && !right
    if (left.type === 'decimal' && right.type === 'decimal') {
        return Number.parseFloat(left.value) === Number.parseFloat(right.value)
    }
    return JSON.stringify(left) === JSON.stringify(right)
}
