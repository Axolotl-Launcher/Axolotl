import type { LabPlugin, LabPluginSlotContribution } from './types'
import { LAB_PLUGIN_SLOTS } from './types'

/**
 * Shape of a discovered plugin module: `export default defineLabPlugin({ ... })`.
 * `folder` is the directory name, which has to equal the manifest id.
 */
export type LabPluginEntry = { folder: string; module: unknown }

/** A plugin that passed validation, paired with the folder it came from. */
export type LabPluginRegistration = {
    plugin: LabPlugin
    folder: string
}

/**
 * A discovered folder that does not describe a usable plugin. Rejections are
 * listed on the plugin page instead of breaking the launcher.
 */
export type LabPluginRejection = {
    folder: string
    reason: string
}

const PLUGIN_ID_PATTERN = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/
const PLUGIN_VERSION_PATTERN = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isText(value: unknown): boolean {
    if (typeof value === 'string') return value.trim().length > 0
    return (
        isRecord(value) &&
        typeof value.id === 'string' &&
        value.id.trim().length > 0 &&
        typeof value.defaultMessage === 'string' &&
        value.defaultMessage.trim().length > 0
    )
}

function isSlotContribution(value: unknown): value is LabPluginSlotContribution {
    if (!isRecord(value)) return false
    if (!LAB_PLUGIN_SLOTS.includes(value.slot as (typeof LAB_PLUGIN_SLOTS)[number])) return false
    if (!value.component) return false
    return value.order === undefined || typeof value.order === 'number'
}

/**
 * Checks one discovered module and returns either the plugin or the first
 * problem found. Kept apart from discovery so the rules can be tested without
 * Vite's glob imports.
 */
export function validateLabPlugin(
    entry: LabPluginEntry,
): { plugin: LabPlugin } | { reason: string } {
    const module = entry.module
    const candidate = isRecord(module) ? module.default : undefined
    if (!isRecord(candidate)) return { reason: 'missing default export' }

    const plugin = candidate as unknown as LabPlugin
    const manifest = plugin.manifest
    if (!isRecord(manifest)) return { reason: 'missing manifest' }

    if (typeof manifest.id !== 'string' || !manifest.id) return { reason: 'missing manifest.id' }
    if (!PLUGIN_ID_PATTERN.test(manifest.id)) {
        return { reason: `manifest.id "${manifest.id}" is not kebab-case` }
    }
    if (manifest.id !== entry.folder) {
        return { reason: `manifest.id "${manifest.id}" does not match folder "${entry.folder}"` }
    }
    if (!isText(manifest.name)) return { reason: 'missing manifest.name' }
    if (!isText(manifest.description)) return { reason: 'missing manifest.description' }
    if (typeof manifest.version !== 'string' || !PLUGIN_VERSION_PATTERN.test(manifest.version)) {
        return { reason: 'manifest.version is not a semantic version' }
    }
    if (manifest.author !== undefined && typeof manifest.author !== 'string') {
        return { reason: 'manifest.author must be a string' }
    }

    const tool = plugin.tool
    if (tool !== undefined) {
        if (!isRecord(tool)) return { reason: 'tool must be an object' }
        if (!isText(tool.title)) return { reason: 'missing tool.title' }
        if (!isText(tool.description)) return { reason: 'missing tool.description' }
        if (!tool.component) return { reason: 'missing tool.component' }
    }

    const slots = plugin.slots
    if (slots !== undefined) {
        if (!Array.isArray(slots)) return { reason: 'slots must be an array' }
        if (!slots.every(isSlotContribution)) return { reason: 'invalid slot contribution' }
    }

    if (tool === undefined && (!Array.isArray(slots) || slots.length === 0)) {
        return { reason: 'renders nothing: provide a tool page or a slot contribution' }
    }

    return { plugin }
}

/**
 * Validates a whole discovery batch. Duplicate ids keep the first plugin in
 * folder order and reject the rest, so a copy-pasted folder cannot silently
 * replace a working plugin.
 */
export function collectLabPlugins(entries: LabPluginEntry[]): {
    plugins: LabPluginRegistration[]
    rejections: LabPluginRejection[]
} {
    const plugins: LabPluginRegistration[] = []
    const rejections: LabPluginRejection[] = []
    const seen = new Set<string>()

    for (const entry of [...entries].sort((a, b) => a.folder.localeCompare(b.folder))) {
        const result = validateLabPlugin(entry)
        if ('reason' in result) {
            rejections.push({ folder: entry.folder, reason: result.reason })
            continue
        }
        const id = result.plugin.manifest.id
        if (seen.has(id)) {
            rejections.push({ folder: entry.folder, reason: `duplicate plugin id "${id}"` })
            continue
        }
        seen.add(id)
        plugins.push({ plugin: result.plugin, folder: entry.folder })
    }

    return { plugins, rejections }
}
