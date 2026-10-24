import type { PreviewClock } from '../../../lib/clock'
import type { ContentItem } from '../../../lib/state/downloads'
import type { GameInstance } from '../../../lib/types'
import type { InstanceUpgradeSelection } from './types'

/** Install-stage changes are the runner's busy signal, not changes to the analyzed source. */
export function upgradeSourceRevision(instance: GameInstance, files: ContentItem[]) {
    return JSON.stringify([
        instance.game_version,
        instance.loader,
        instance.loader_version,
        instance.loader_components,
        instance.link,
        instance.symlink_target,
        files,
    ])
}

export function applyUpgradeSelection(
    previous: ContentItem,
    selection: InstanceUpgradeSelection | undefined,
    replacement: () => ContentItem,
): ContentItem {
    if (!selection) return { ...previous }
    return {
        ...(selection.action === 'upgrade' ? replacement() : previous),
        enabled: selection.enabled,
    }
}

/** The local adapter stages work until one guarded commit; cancellation never calls commit. */
export function runUpgradeTransaction(options: {
    clock: PreviewClock
    sourceIsCurrent: () => boolean
    progress: (fraction: number, elapsed: number) => void
    commit: () => void
    failed: (error: unknown) => void
    finished: () => void
}) {
    let elapsed = 0
    let settled = false
    const stop = options.clock.every((delta) => {
        if (settled) return
        elapsed += delta
        options.progress(Math.min(1, elapsed / 4200), elapsed)
        if (elapsed < 4200) return
        settled = true
        stop()
        try {
            if (!options.sourceIsCurrent())
                throw new Error(
                    'Instance content changed during the upgrade. No changes were applied.',
                )
            options.commit()
        } catch (error) {
            options.failed(error)
        } finally {
            options.finished()
        }
    })
    return () => {
        if (settled) return
        settled = true
        stop()
        options.finished()
    }
}
