import type { LabPluginEntry, LabPluginRegistration, LabPluginRejection } from './validation'
import { collectLabPlugins } from './validation'

export type { LabPluginEntry, LabPluginRegistration, LabPluginRejection } from './validation'

type LabPluginModule = { default?: unknown }

/**
 * Every plugin package compiled into this build. Each folder under
 * `packages/` with an `index.ts` is discovered here, which is why adding a
 * plugin never requires touching this file, the router or the lab registry.
 */
const discoveredModules = import.meta.glob<LabPluginModule>('./packages/*/index.ts', {
    eager: true,
})

const discoveredEntries: LabPluginEntry[] = Object.entries(discoveredModules).map(
    ([path, module]) => ({
        folder: path.split('/')[2] ?? path,
        module,
    }),
)

const collected = collectLabPlugins(discoveredEntries)

/** Every valid plugin compiled into this build, sorted by folder name. */
export const labPluginRegistrations: readonly LabPluginRegistration[] = collected.plugins

/** Folders that failed validation, listed on the plugin page. */
export const labPluginRejections: readonly LabPluginRejection[] = collected.rejections

/** Looks up one plugin by manifest id. */
export function getLabPluginRegistration(id: string): LabPluginRegistration | undefined {
    return labPluginRegistrations.find((entry) => entry.plugin.manifest.id === id)
}
