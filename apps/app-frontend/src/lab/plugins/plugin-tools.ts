import type { Component } from 'vue'

import type { LabPluginCategory, LabPluginText } from './types'
import type { LabPluginRegistration } from './validation'

/** A plugin tool, shaped like a built-in lab tool so the hub can list both. */
export type LabPluginToolEntry = {
    /** Plugin manifest id, used as the card key and route parameter. */
    id: string
    /** Where the card links. */
    route: string
    /** Hub filter this card belongs to. */
    category: LabPluginCategory
    /** Card icon; a plugin may omit it. */
    icon?: Component
    /** Card title, resolved by the caller through i18n. */
    title: LabPluginText
    /** Card description, resolved by the caller through i18n. */
    description: LabPluginText
    /** Marks the card as coming from a plugin, so the hub can badge it. */
    plugin: true
}

/**
 * URL of a plugin's tool page. Colocated with the entry builder so the route
 * and the card cannot drift apart.
 */
export function labPluginToolRoute(pluginId: string): string {
    return `/lab/p/${pluginId}`
}

/** Tool cards for every enabled plugin that provides a tool page. */
export function collectLabPluginToolEntries(
    registrations: readonly LabPluginRegistration[],
    isEnabled: (pluginId: string) => boolean,
): LabPluginToolEntry[] {
    return registrations
        .filter((entry) => isEnabled(entry.plugin.manifest.id))
        .flatMap((entry) => {
            const { plugin } = entry
            const tool = plugin.tool
            if (!tool) return []
            return [
                {
                    id: plugin.manifest.id,
                    route: labPluginToolRoute(plugin.manifest.id),
                    category: tool.category ?? plugin.manifest.category ?? 'creation',
                    icon: tool.icon ?? plugin.manifest.icon,
                    title: tool.title,
                    description: tool.description,
                    plugin: true as const,
                },
            ]
        })
        .sort((a, b) => a.id.localeCompare(b.id))
}
