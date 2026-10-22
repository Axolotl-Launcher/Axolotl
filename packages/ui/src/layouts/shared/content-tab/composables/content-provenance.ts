import type { ContentItem } from '../types'

export function installedSource(
    item: Pick<ContentItem, 'installedSource' | 'origin_provider'>,
): string {
    return item.installedSource === undefined
        ? (item.origin_provider ?? 'unknown')
        : (item.installedSource ?? 'unknown')
}

export function contentOwnership(
    item: Pick<ContentItem, 'instanceOwnershipKind' | 'source_kind'>,
): string {
    if (item.instanceOwnershipKind) return item.instanceOwnershipKind
    switch (item.source_kind) {
        case 'modrinth_modpack':
        case 'imported_modpack':
            return 'pack_managed'
        case 'local':
        case 'world_datapack':
            return 'local_discovered'
        case 'curseforge':
        case 'server_project':
        case 'shared_instance':
            return 'user_added'
        default:
            return 'unknown'
    }
}
