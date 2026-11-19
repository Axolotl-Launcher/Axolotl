import { defineMessages, type KeyBinding, type MessageDescriptor } from '@modrinth/ui'
import type { RouteLocationNormalizedLoaded } from 'vue-router'

import { getLastBrowseContentProjectType, isBrowseContentProjectType } from '@/helpers/settings'

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

/**
 * Declared here rather than assembled from the action ids, so the extractor can
 * see them; an id built at runtime is one a translation pass would drop.
 */
const messages = defineMessages({
    scrollTop: {
        id: 'app.shortcut-settings.home-action',
        defaultMessage: 'Scroll to the top',
    },
    scrollTopDescription: {
        id: 'app.shortcut-settings.home-action-description',
        defaultMessage: 'Moves the page you are reading to the top.',
    },
    scrollBottom: {
        id: 'app.shortcut-settings.end-action',
        defaultMessage: 'Scroll to the bottom',
    },
    scrollBottomDescription: {
        id: 'app.shortcut-settings.end-action-description',
        defaultMessage: 'Moves the page you are reading to the bottom.',
    },
    scrollUp: {
        id: 'app.shortcut-settings.page-up-action',
        defaultMessage: 'Scroll up one screen',
    },
    scrollUpDescription: {
        id: 'app.shortcut-settings.page-up-action-description',
        defaultMessage: 'Moves the page you are reading up by one screen.',
    },
    scrollDown: {
        id: 'app.shortcut-settings.page-down-action',
        defaultMessage: 'Scroll down one screen',
    },
    scrollDownDescription: {
        id: 'app.shortcut-settings.page-down-action-description',
        defaultMessage: 'Moves the page you are reading down by one screen.',
    },
    navHome: { id: 'app.shortcut-settings.nav-home', defaultMessage: 'Home' },
    navHomeDescription: {
        id: 'app.shortcut-settings.nav-home-description',
        defaultMessage: 'Jump to the Home page.',
    },
    navWorlds: { id: 'app.shortcut-settings.nav-worlds', defaultMessage: 'Worlds' },
    navWorldsDescription: {
        id: 'app.shortcut-settings.nav-worlds-description',
        defaultMessage: 'Jump to your worlds.',
    },
    navDiscover: {
        id: 'app.shortcut-settings.nav-discover',
        defaultMessage: 'Discover content',
    },
    navDiscoverDescription: {
        id: 'app.shortcut-settings.nav-discover-description',
        defaultMessage: 'Jump to browsing content.',
    },
    navSkins: { id: 'app.shortcut-settings.nav-skins', defaultMessage: 'Skin selector' },
    navSkinsDescription: {
        id: 'app.shortcut-settings.nav-skins-description',
        defaultMessage: 'Jump to the skin selector.',
    },
    navMultiplayer: { id: 'app.shortcut-settings.nav-multiplayer', defaultMessage: 'Multiplayer' },
    navMultiplayerDescription: {
        id: 'app.shortcut-settings.nav-multiplayer-description',
        defaultMessage: 'Jump to multiplayer.',
    },
    navLibrary: { id: 'app.shortcut-settings.nav-library', defaultMessage: 'Library' },
    navLibraryDescription: {
        id: 'app.shortcut-settings.nav-library-description',
        defaultMessage: 'Jump to your library.',
    },
    navLab: { id: 'app.shortcut-settings.nav-lab', defaultMessage: 'Lab' },
    navLabDescription: {
        id: 'app.shortcut-settings.nav-lab-description',
        defaultMessage: 'Jump to the Lab.',
    },
    navDownloads: { id: 'app.shortcut-settings.nav-downloads', defaultMessage: 'Downloads' },
    navDownloadsDescription: {
        id: 'app.shortcut-settings.nav-downloads-description',
        defaultMessage: 'Jump to downloads.',
    },
    navCreate: { id: 'app.shortcut-settings.nav-create', defaultMessage: 'Create new instance' },
    navCreateDescription: {
        id: 'app.shortcut-settings.nav-create-description',
        defaultMessage: 'Jump to creating a new instance.',
    },
    navSettings: { id: 'app.shortcut-settings.nav-settings', defaultMessage: 'Settings' },
    navSettingsDescription: {
        id: 'app.shortcut-settings.nav-settings-description',
        defaultMessage: 'Jump to the settings.',
    },
})

function keyboard(code: string, mod = false, alt = false, shift = false): KeyBinding {
    return { device: 'keyboard', code, mod, alt, shift }
}

/**
 * Resolves the Discover content target from the current route, so the nav
 * button and the keyboard shortcut share one source of truth.
 */
export function discoverContentTarget(route: RouteLocationNormalizedLoaded): string {
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

    return `/browse/${getLastBrowseContentProjectType()}`
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
        label: messages.scrollTop,
        description: messages.scrollTopDescription,
        applyScroll: (scroller) => scroller.scrollTo({ top: 0, behavior: 'smooth' }),
    },
    {
        id: 'shortcutScrollBottom',
        group: 'scroll',
        enabledField: 'quickScrollEnabled',
        defaultBinding: keyboard('End'),
        label: messages.scrollBottom,
        description: messages.scrollBottomDescription,
        applyScroll: (scroller) =>
            scroller.scrollTo({ top: scroller.scrollHeight, behavior: 'smooth' }),
    },
    {
        id: 'shortcutScrollUp',
        group: 'scroll',
        enabledField: 'quickScrollEnabled',
        defaultBinding: keyboard('PageUp'),
        label: messages.scrollUp,
        description: messages.scrollUpDescription,
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
        label: messages.scrollDown,
        description: messages.scrollDownDescription,
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
        label: messages.navHome,
        description: messages.navHomeDescription,
        target: () => '/',
    },
    {
        id: 'shortcutNavWorlds',
        group: 'nav',
        enabledField: 'shortcutNavWorlds',
        defaultBinding: keyboard('Digit2', true),
        label: messages.navWorlds,
        description: messages.navWorldsDescription,
        target: () => '/worlds',
        unavailable: (context) => !context.worldsTabEnabled,
    },
    {
        id: 'shortcutNavDiscover',
        group: 'nav',
        enabledField: 'shortcutNavDiscover',
        defaultBinding: keyboard('Digit3', true),
        label: messages.navDiscover,
        description: messages.navDiscoverDescription,
        target: (route) => discoverContentTarget(route),
    },
    {
        id: 'shortcutNavSkins',
        group: 'nav',
        enabledField: 'shortcutNavSkins',
        defaultBinding: keyboard('Digit4', true),
        label: messages.navSkins,
        description: messages.navSkinsDescription,
        target: () => '/skins',
    },
    {
        id: 'shortcutNavMultiplayer',
        group: 'nav',
        enabledField: 'shortcutNavMultiplayer',
        defaultBinding: keyboard('Digit5', true),
        label: messages.navMultiplayer,
        description: messages.navMultiplayerDescription,
        target: () => '/multiplayer',
    },
    {
        id: 'shortcutNavLibrary',
        group: 'nav',
        enabledField: 'shortcutNavLibrary',
        defaultBinding: keyboard('Digit6', true),
        label: messages.navLibrary,
        description: messages.navLibraryDescription,
        target: () => '/library',
    },
    {
        id: 'shortcutNavLab',
        group: 'nav',
        enabledField: 'shortcutNavLab',
        defaultBinding: keyboard('Digit7', true),
        label: messages.navLab,
        description: messages.navLabDescription,
        target: () => '/lab',
    },
    {
        id: 'shortcutNavDownloads',
        group: 'nav',
        enabledField: 'shortcutNavDownloads',
        defaultBinding: keyboard('Digit8', true),
        label: messages.navDownloads,
        description: messages.navDownloadsDescription,
        target: () => '/downloads',
    },
    {
        id: 'shortcutNavCreate',
        group: 'nav',
        enabledField: 'shortcutNavCreate',
        defaultBinding: keyboard('Digit9', true),
        label: messages.navCreate,
        description: messages.navCreateDescription,
        target: () => '/create',
    },
    {
        id: 'shortcutNavSettings',
        group: 'nav',
        enabledField: 'shortcutNavSettings',
        defaultBinding: keyboard('Comma', true),
        label: messages.navSettings,
        description: messages.navSettingsDescription,
        target: () => '/settings',
    },
]

export const SCROLL_ACTIONS = SHORTCUT_ACTIONS.filter((action) => action.group === 'scroll')
export const NAV_ACTIONS = SHORTCUT_ACTIONS.filter((action) => action.group === 'nav')

export function shortcutAction(id: string): ShortcutAction | undefined {
    return SHORTCUT_ACTIONS.find((action) => action.id === id)
}
