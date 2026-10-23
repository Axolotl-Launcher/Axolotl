import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'
import type { BindingConflict, KeyBinding } from '@modrinth/ui/src/utils/keybinding.ts'
import type { RouteLocationNormalizedLoaded } from 'vue-router'

import { messages } from './messages'

/**
 * The launcher's keyboard shortcuts (apps/app-frontend/src/helpers/shortcut-actions.ts
 * and shortcut-bindings.ts). Recorded combinations live in the settings state
 * instead of local storage; everything else is unchanged.
 */

const BROWSE_CONTENT_PROJECT_TYPES = ['mod', 'modpack', 'resourcepack', 'shader', 'datapack']

function isBrowseContentProjectType(value: string) {
    return BROWSE_CONTENT_PROJECT_TYPES.includes(value)
}

/** Which half of the settings page an action belongs to. */
export type ShortcutGroupId = 'scroll' | 'nav'

/** Theme store fields that switch an action on. */
export type ShortcutEnabledField =
    | 'quickScrollEnabled'
    | 'shortcutNavHome'
    | 'shortcutNavWorlds'
    | 'shortcutNavDiscover'
    | 'shortcutNavSkins'
    | 'shortcutNavMultiplayer'
    | 'shortcutNavLibrary'
    | 'shortcutNavLab'
    | 'shortcutNavDownloads'
    | 'shortcutNavCreate'
    | 'shortcutNavSettings'

export interface ShortcutContext {
    worldsTabEnabled: boolean
    offline: boolean
}

export interface ShortcutAction {
    /** Stable id, used for the binding, the enable flag and the settings row. */
    id: string
    group: ShortcutGroupId
    enabledField: ShortcutEnabledField
    defaultBinding: KeyBinding
    label: MessageDescriptor
    description: MessageDescriptor
    /** Navigation actions know where they lead. */
    target?: (route: RouteLocationNormalizedLoaded) => string
    /** Navigation actions that are not reachable right now. */
    unavailable?: (context: ShortcutContext) => boolean
    /** Scrolling actions move the page's own scroller. */
    applyScroll?: (scroller: HTMLElement) => void
}

function keyboard(code: string, mod = false, alt = false, shift = false): KeyBinding {
    return { device: 'keyboard', code, mod, alt, shift }
}

/**
 * Resolves the Discover content target from the current route, so the nav
 * button and the keyboard shortcut share one source of truth.
 */
export function discoverContentTarget(
    route: RouteLocationNormalizedLoaded,
    lastBrowseProjectType = 'mod',
): string {
    const projectType = route.params.projectType
    if (
        !route.query.i &&
        !route.query.sid &&
        !route.query.wid &&
        typeof projectType === 'string' &&
        isBrowseContentProjectType(projectType)
    ) {
        return `/browse/${projectType}`
    }

    return `/browse/${lastBrowseProjectType}`
}

/**
 * Every shortcut the launcher answers to. The binding here is the one a fresh
 * install uses; a recorded binding is stored separately and wins over it.
 */
export const SHORTCUT_ACTIONS: ShortcutAction[] = [
    {
        id: 'shortcutScrollTop',
        group: 'scroll',
        enabledField: 'quickScrollEnabled',
        defaultBinding: keyboard('Home'),
        label: messages.appShortcutSettingsHomeAction,
        description: messages.appShortcutSettingsHomeActionDescription,
        applyScroll: (scroller) => scroller.scrollTo({ top: 0, behavior: 'smooth' }),
    },
    {
        id: 'shortcutScrollBottom',
        group: 'scroll',
        enabledField: 'quickScrollEnabled',
        defaultBinding: keyboard('End'),
        label: messages.appShortcutSettingsEndAction,
        description: messages.appShortcutSettingsEndActionDescription,
        applyScroll: (scroller) =>
            scroller.scrollTo({ top: scroller.scrollHeight, behavior: 'smooth' }),
    },
    {
        id: 'shortcutScrollUp',
        group: 'scroll',
        enabledField: 'quickScrollEnabled',
        defaultBinding: keyboard('PageUp'),
        label: messages.appShortcutSettingsPageUpAction,
        description: messages.appShortcutSettingsPageUpActionDescription,
        applyScroll: (scroller) => {
            // Immediate scrolling keeps rapid key repeats responsive.
            scroller.scrollTop = Math.max(0, scroller.scrollTop - scroller.clientHeight * 0.9)
        },
    },
    {
        id: 'shortcutScrollDown',
        group: 'scroll',
        enabledField: 'quickScrollEnabled',
        defaultBinding: keyboard('PageDown'),
        label: messages.appShortcutSettingsPageDownAction,
        description: messages.appShortcutSettingsPageDownActionDescription,
        applyScroll: (scroller) => {
            scroller.scrollTop = Math.min(
                scroller.scrollHeight,
                scroller.scrollTop + scroller.clientHeight * 0.9,
            )
        },
    },
    {
        id: 'shortcutNavHome',
        group: 'nav',
        enabledField: 'shortcutNavHome',
        defaultBinding: keyboard('Digit1', true),
        label: messages.appShortcutSettingsNavHome,
        description: messages.appShortcutSettingsNavHomeDescription,
        target: () => '/',
    },
    {
        id: 'shortcutNavWorlds',
        group: 'nav',
        enabledField: 'shortcutNavWorlds',
        defaultBinding: keyboard('Digit2', true),
        label: messages.appShortcutSettingsNavWorlds,
        description: messages.appShortcutSettingsNavWorldsDescription,
        target: () => '/worlds',
        unavailable: (context) => !context.worldsTabEnabled,
    },
    {
        id: 'shortcutNavDiscover',
        group: 'nav',
        enabledField: 'shortcutNavDiscover',
        defaultBinding: keyboard('Digit3', true),
        label: messages.appShortcutSettingsNavDiscover,
        description: messages.appShortcutSettingsNavDiscoverDescription,
        target: (route) => discoverContentTarget(route),
        unavailable: (context) => context.offline,
    },
    {
        id: 'shortcutNavSkins',
        group: 'nav',
        enabledField: 'shortcutNavSkins',
        defaultBinding: keyboard('Digit4', true),
        label: messages.appShortcutSettingsNavSkins,
        description: messages.appShortcutSettingsNavSkinsDescription,
        target: () => '/skins',
    },
    {
        id: 'shortcutNavMultiplayer',
        group: 'nav',
        enabledField: 'shortcutNavMultiplayer',
        defaultBinding: keyboard('Digit5', true),
        label: messages.appShortcutSettingsNavMultiplayer,
        description: messages.appShortcutSettingsNavMultiplayerDescription,
        target: () => '/multiplayer',
    },
    {
        id: 'shortcutNavLibrary',
        group: 'nav',
        enabledField: 'shortcutNavLibrary',
        defaultBinding: keyboard('Digit6', true),
        label: messages.appShortcutSettingsNavLibrary,
        description: messages.appShortcutSettingsNavLibraryDescription,
        target: () => '/library',
    },
    {
        id: 'shortcutNavLab',
        group: 'nav',
        enabledField: 'shortcutNavLab',
        defaultBinding: keyboard('Digit7', true),
        label: messages.appShortcutSettingsNavLab,
        description: messages.appShortcutSettingsNavLabDescription,
        target: () => '/lab',
    },
    {
        id: 'shortcutNavDownloads',
        group: 'nav',
        enabledField: 'shortcutNavDownloads',
        defaultBinding: keyboard('Digit8', true),
        label: messages.appShortcutSettingsNavDownloads,
        description: messages.appShortcutSettingsNavDownloadsDescription,
        target: () => '/downloads',
    },
    {
        id: 'shortcutNavCreate',
        group: 'nav',
        enabledField: 'shortcutNavCreate',
        defaultBinding: keyboard('Digit9', true),
        label: messages.appShortcutSettingsNavCreate,
        description: messages.appShortcutSettingsNavCreateDescription,
        target: () => '/create',
        unavailable: (context) => context.offline,
    },
    {
        id: 'shortcutNavSettings',
        group: 'nav',
        enabledField: 'shortcutNavSettings',
        defaultBinding: keyboard('Comma', true),
        label: messages.appShortcutSettingsNavSettings,
        description: messages.appShortcutSettingsNavSettingsDescription,
        target: () => '/settings',
    },
]

export const SCROLL_ACTIONS = SHORTCUT_ACTIONS.filter((action) => action.group === 'scroll')
export const NAV_ACTIONS = SHORTCUT_ACTIONS.filter((action) => action.group === 'nav')

export function shortcutAction(id: string): ShortcutAction | undefined {
    return SHORTCUT_ACTIONS.find((action) => action.id === id)
}

/* ---- Recorded combinations (helpers/shortcut-bindings.ts) ------------------ */

/**
 * Combinations the shell or the operating system answers to before the launcher
 * sees them. Saved shortcuts may not take these over.
 */
export const RESERVED_BINDINGS: BindingConflict[] = [
    {
        id: 'system-find',
        label: 'Find in page (Ctrl/Cmd + F)',
        binding: keyboard('KeyF', true),
        reserved: true,
    },
    {
        id: 'system-devtools',
        label: 'Developer tools (F12)',
        binding: keyboard('F12'),
        reserved: true,
    },
    {
        id: 'system-close',
        label: 'Close window (Ctrl/Cmd + W)',
        binding: keyboard('KeyW', true),
        reserved: true,
    },
    {
        id: 'system-quit',
        label: 'Quit (Ctrl/Cmd + Q)',
        binding: keyboard('KeyQ', true),
        reserved: true,
    },
    {
        id: 'system-reload',
        label: 'Reload (Ctrl/Cmd + R)',
        binding: keyboard('KeyR', true),
        reserved: true,
    },
    { id: 'system-refresh', label: 'Refresh (F5)', binding: keyboard('F5'), reserved: true },
    {
        id: 'system-exit',
        label: 'Close window (Alt + F4)',
        binding: keyboard('F4', false, true),
        reserved: true,
    },
]

/** The combination an action answers to now: the recorded one, or its default. */
export function resolveBinding(stored: Record<string, KeyBinding>, id: string): KeyBinding | null {
    const action = shortcutAction(id)
    if (!action) return null
    return stored[id] ?? action.defaultBinding
}

export function resolveAllBindings(stored: Record<string, KeyBinding>): Record<string, KeyBinding> {
    const bindings: Record<string, KeyBinding> = {}
    for (const action of SHORTCUT_ACTIONS) {
        bindings[action.id] = resolveBinding(stored, action.id) ?? action.defaultBinding
    }
    return bindings
}

/**
 * Everything a new combination would collide with: the other actions, plus the
 * combinations the shell owns. `label` is what the dialog names in the error.
 */
export function buildConflicts(
    stored: Record<string, KeyBinding>,
    exceptId: string,
    label: (id: string) => string,
): BindingConflict[] {
    const conflicts = SHORTCUT_ACTIONS.filter((action) => action.id !== exceptId).flatMap(
        (action) => {
            const binding = resolveBinding(stored, action.id)
            return binding ? [{ id: action.id, label: label(action.id), binding }] : []
        },
    )

    return [...conflicts, ...RESERVED_BINDINGS]
}
