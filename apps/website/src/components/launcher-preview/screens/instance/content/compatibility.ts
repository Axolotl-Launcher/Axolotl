import type { ContentItem as LayoutContentItem } from '@modrinth/ui/src/layouts/shared/content-tab/types.ts'

import type { ContentItem } from '../../../lib/state/downloads'
import type { GameInstance } from '../../../lib/types'

/** Use the installed release's declarations, never a different release of the project. */
export function contentCompatibility(
    item: Pick<ContentItem, 'project_type'>,
    info: { gameVersions: string[]; loaders: string[] } | null,
    instance: Pick<GameInstance, 'game_version' | 'loader'>,
): NonNullable<LayoutContentItem['compatibility']> {
    const gameVersions = info?.gameVersions ?? []
    const loaders = info?.loaders ?? []
    const independent = item.project_type === 'resourcepack' || item.project_type === 'shader'
    return {
        gameVersion: gameVersions.length
            ? gameVersions.includes(instance.game_version)
                ? 'compatible'
                : 'incompatible'
            : 'unknown',
        loader: independent
            ? 'compatible'
            : loaders.length
              ? loaders.includes(instance.loader) ||
                (item.project_type === 'datapack' && loaders.includes('datapack'))
                  ? 'compatible'
                  : 'incompatible'
              : 'unknown',
        reasons: [],
    }
}
