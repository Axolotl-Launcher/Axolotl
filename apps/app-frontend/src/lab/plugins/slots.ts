import type { LabPlugin, LabPluginSlotContribution, LabPluginSlotName } from './types'
import type { LabPluginRegistration } from './validation'

/** One plugin component bound to a slot, ready to render. */
export type ResolvedSlotContribution = {
    plugin: LabPlugin
    contribution: LabPluginSlotContribution
}

/**
 * Collects the components a slot should render, skipping disabled plugins.
 * Sorting is by `order` and then by plugin id, so two plugins that did not set
 * an order still render in a stable, reproducible sequence.
 */
export function collectSlotContributions(
    registrations: readonly LabPluginRegistration[],
    slot: LabPluginSlotName,
    isEnabled: (pluginId: string) => boolean,
): ResolvedSlotContribution[] {
    return registrations
        .filter((entry) => isEnabled(entry.plugin.manifest.id))
        .flatMap((entry) =>
            (entry.plugin.slots ?? [])
                .filter((contribution) => contribution.slot === slot)
                .map((contribution) => ({ plugin: entry.plugin, contribution })),
        )
        .sort((a, b) => {
            const orderDelta = (a.contribution.order ?? 0) - (b.contribution.order ?? 0)
            if (orderDelta !== 0) return orderDelta
            return a.plugin.manifest.id.localeCompare(b.plugin.manifest.id)
        })
}
