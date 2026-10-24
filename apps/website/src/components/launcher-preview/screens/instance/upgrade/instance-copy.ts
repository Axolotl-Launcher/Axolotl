import type { PreviewState } from '../../../lib/state'
import type { ContentItem } from '../../../lib/state/downloads'
import type { PreviewScreenshot } from '../../../lib/state/screenshots'
import type { GameInstance } from '../../../lib/types'
import type { HomeWorld } from '../../home/home-utils'
import type { FsDir } from '../files/instance-files'
import type { InstanceLogFile } from '../logs/state'
import type { CoreComponentSnapshot } from '../settings/core-components'
import type { InstanceSettingsExtras } from '../settings/state'
import type { ContentRollback } from '../state'
import type { StoredLevel } from '../worlds/worlds-api'
import { reconcileContentFiles } from './content-files.ts'

export interface InstanceCopySnapshot {
    worlds: HomeWorld[]
    levels: [string, StoredLevel][]
    settings: InstanceSettingsExtras
    files: FsDir
    logs: InstanceLogFile[]
    screenshots: PreviewScreenshot[]
    rollbacks: Record<string, ContentRollback>
    coreComponents: CoreComponentSnapshot
}

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value))

/** Rewrite only instance ownership; paths inside saves and screenshot assets stay intact. */
export function rebindInstanceCopy(
    snapshot: InstanceCopySnapshot,
    target: Pick<GameInstance, 'id' | 'name'>,
): InstanceCopySnapshot {
    const copy = clone(snapshot)
    copy.worlds = copy.worlds.map((world) => ({ ...world, instance_id: target.id }))
    copy.screenshots = copy.screenshots.map((shot) => ({
        ...shot,
        id: `${target.id}:${shot.id}`,
        instance_id: target.id,
        instance_name: target.name,
    }))
    copy.settings.game_dir_override = null
    copy.settings.linked_dot_minecraft = null
    copy.files.name = target.name
    return copy
}

/** Snapshot all user-visible instance data before the upgrade job starts. */
export async function prepareInstanceCopy(preview: PreviewState, source: GameInstance) {
    const [
        { previewDomain },
        { createHomeState },
        { instanceFiles },
        { instanceLogs },
        { instanceSettingsState },
        { prepareWorldLevels },
        { instanceContent },
        { instanceCoreComponents },
    ] = await Promise.all([
        import('../../../lib/state'),
        import('../../home/state'),
        import('../files/instance-files'),
        import('../logs/state'),
        import('../settings/state'),
        import('../worlds/worlds-api'),
        import('../state'),
        import('../settings/core-components'),
    ])
    const home = previewDomain(preview, 'home', createHomeState)
    const files = instanceFiles(preview)
    const logs = instanceLogs(preview)
    const settings = instanceSettingsState(preview)
    const content = instanceContent(preview)
    const core = instanceCoreComponents(preview)
    const levels = await prepareWorldLevels(source.id)
    const read = (): InstanceCopySnapshot => ({
        worlds: home.worlds.value.filter((world) => world.instance_id === source.id),
        levels: [...levels.entries()]
            .filter(([key]) => key.startsWith(`${source.id}:`))
            .map(([key, value]) => [key.slice(source.id.length + 1), value]),
        settings: settings.extrasFor(source.id),
        files: files.treeFor(source),
        logs: logs.filesFor(source.id),
        screenshots: preview.screenshots.screenshots.value.filter(
            (shot) => shot.instance_id === source.id,
        ),
        rollbacks: content.rollbackSnapshot(source.id),
        coreComponents: core.snapshot(source.id),
    })
    const snapshot = clone(read())
    const signature = JSON.stringify(snapshot)
    return {
        isCurrent: () => JSON.stringify(read()) === signature,
        syncContent(target: GameInstance, before: ContentItem[], after: ContentItem[]) {
            reconcileContentFiles(files.treeFor(target), before, after, Date.now())
        },
        apply(target: GameInstance) {
            const copy = rebindInstanceCopy(snapshot, target)
            home.worlds.value = [
                ...home.worlds.value.filter((world) => world.instance_id !== target.id),
                ...copy.worlds,
            ]
            for (const [path, level] of copy.levels) levels.set(`${target.id}:${path}`, level)
            settings.extras[target.id] = copy.settings
            files.setTree(target.id, copy.files)
            logs.files[target.id] = copy.logs
            preview.screenshots.screenshots.value = [
                ...preview.screenshots.screenshots.value.filter(
                    (shot) => shot.instance_id !== target.id,
                ),
                ...copy.screenshots,
            ]
            content.restoreRollbackSnapshot(target.id, copy.rollbacks)
            core.restoreSnapshot(target.id, copy.coreComponents)
        },
    }
}
