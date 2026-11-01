import type { MessageDescriptor } from '@modrinth/ui'
import type { Component, InjectionKey } from 'vue'
import { inject } from 'vue'

import type { GameInstance } from '@/helpers/types'

/**
 * Text a plugin shows in launcher chrome: cards, headers, lists and button
 * labels. A plugin that ships with this repository passes a message descriptor
 * so the launcher translates its copy; a third-party plugin passes a plain
 * string and keeps responsibility for translating it.
 */
export type LabPluginText = string | MessageDescriptor

/**
 * Capabilities a plugin declares in its manifest. The launcher shows them to
 * the user before the plugin's UI renders, and the plugin is expected to stay
 * inside what it declared. Declaring nothing is the safe default.
 */
export const LAB_PLUGIN_PERMISSIONS = [
    'instances:read',
    'files:read',
    'files:write',
    'network',
    'clipboard',
] as const

/** One declared capability. See {@link LAB_PLUGIN_PERMISSIONS}. */
export type LabPluginPermission = (typeof LAB_PLUGIN_PERMISSIONS)[number]

/**
 * Named places in the launcher where a plugin may render its own component.
 * The launcher owns these positions; a plugin only fills them, so a bad plugin
 * can break its own widget but not the surrounding page.
 */
export const LAB_PLUGIN_SLOTS = ['lab.hub.footer', 'instance.actions'] as const

/** One place a plugin may render into. See {@link LAB_PLUGIN_SLOTS}. */
export type LabPluginSlotName = (typeof LAB_PLUGIN_SLOTS)[number]

/**
 * Props the launcher passes to a component rendered in each slot. Slot
 * components receive them as ordinary Vue props with the same names.
 */
export type LabPluginSlotContexts = {
    /** Footer area of the lab hub, below the tool list. No props. */
    'lab.hub.footer': Record<string, never>
    /** Action row of an instance page. Receives the instance being viewed. */
    'instance.actions': { instance: GameInstance }
}

/** Tool categories the lab hub filters by. */
export type LabPluginCategory = 'creation' | 'maintenance' | 'world'

/**
 * Identity and disclosure block every plugin must provide. `id` has to match
 * the plugin folder name, because the id is also the route parameter used to
 * open the plugin's tool page.
 */
export type LabPluginManifest = {
    /** Kebab-case identifier. Must equal the plugin folder name. */
    id: string
    /** Name shown in the lab card, the plugin list and breadcrumbs. */
    name: LabPluginText
    /** One sentence describing what the plugin does. */
    description: LabPluginText
    /** Semantic version of the plugin itself, e.g. `1.0.0`. */
    version: string
    /** Author shown next to the version. Omit for local plugins. */
    author?: string
    /** Lab hub category. Defaults to `creation`. */
    category?: LabPluginCategory
    /** Icon used when the plugin has no cover image. */
    icon?: Component
    /** Capabilities the plugin uses. Omit when it needs nothing. */
    permissions?: LabPluginPermission[]
}

/**
 * Per-plugin key-value store. Keys are namespaced by plugin id, so two plugins
 * cannot read or overwrite each other's values.
 */
export type LabPluginStorage = {
    /** Reads a stored value, returning `fallback` when absent or unreadable. */
    read: <T>(key: string, fallback: T) => T
    /** Stores a JSON-serialisable value. */
    write: (key: string, value: unknown) => void
    /** Removes a stored value. */
    remove: (key: string) => void
}

/**
 * What a plugin component can reach while it renders. Delivered through
 * {@link useLabPluginContext} rather than props, so a plugin's own child
 * components do not have to pass it down.
 */
export type LabPluginContext = {
    /** The manifest id of the running plugin. */
    pluginId: string
    /** Plugin-scoped storage. */
    storage: LabPluginStorage
    /** Writes a line to the frontend console with the plugin id prefixed. */
    log: (message: string, detail?: unknown) => void
}

/** Injection key used by {@link useLabPluginContext}. */
export const LAB_PLUGIN_CONTEXT: InjectionKey<LabPluginContext> = Symbol('lab-plugin-context')

/** What the plugin's own page shows. */
export type LabPluginTool = {
    /** Card title. Overrides the manifest name on the lab card. */
    title: LabPluginText
    /** Card description. Overrides the manifest description on the lab card. */
    description: LabPluginText
    /** Card icon. Falls back to the manifest icon. */
    icon?: Component
    /** Card category. Falls back to the manifest category. */
    category?: LabPluginCategory
    /** The page body rendered at `/lab/p/<id>`. */
    component: Component
}

/** One component a plugin renders into a launcher slot. */
export type LabPluginSlotContribution = {
    /** Which slot to fill. */
    slot: LabPluginSlotName
    /** Component rendered inside that slot. Receives the slot's context as props. */
    component: Component
    /** Sort weight inside the slot. Lower renders first; ties sort by plugin id. */
    order?: number
}

/**
 * A plugin. Provide at least a tool page, a slot contribution, or both; a
 * plugin that renders nothing is rejected during discovery.
 */
export type LabPlugin = {
    manifest: LabPluginManifest
    /** Lab tool page. Omit for a plugin that only fills slots. */
    tool?: LabPluginTool
    /** Launcher surfaces the plugin adds components to. */
    slots?: LabPluginSlotContribution[]
}

/**
 * Marks an object as a plugin so editors and `vue-tsc` check it in place.
 * Returns the same object; it exists only for type inference.
 */
export function defineLabPlugin(plugin: LabPlugin): LabPlugin {
    return plugin
}

/**
 * Reads the running plugin's context. Only callable from a component rendered
 * by the plugin host; throws when used outside one, because a plugin without
 * its id and storage cannot behave correctly.
 */
export function useLabPluginContext(): LabPluginContext {
    const context = inject(LAB_PLUGIN_CONTEXT)
    if (!context) {
        throw new Error('useLabPluginContext() was called outside a lab plugin host')
    }
    return context
}
