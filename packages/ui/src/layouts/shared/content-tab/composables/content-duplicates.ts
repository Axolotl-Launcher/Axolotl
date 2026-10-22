import type { ContentCardTableItem, ContentItem } from '../types'
import { hasContentUpdate } from './content-filtering.ts'

export function confirmedContentGroupKey(item: ContentItem): string | null {
    if (!item.instanceResourceId) return null
    const path = item.file_path?.replaceAll('\\', '/') ?? ''
    const world = path.match(/^saves\/[^/]+\/datapacks\//)?.[0]
    if (item.project_type === 'datapack' && !world) return null
    return JSON.stringify([item.instanceResourceId, item.project_type, world ?? 'instance'])
}

export function duplicateContentGroups(items: ContentItem[], getId: (item: ContentItem) => string) {
    const groups = new Map<string, ContentItem[]>()
    const seen = new Set<string>()
    for (const item of items) {
        const id = getId(item)
        if (seen.has(id)) continue
        seen.add(id)
        const key = confirmedContentGroupKey(item)
        if (!key) continue
        const group = groups.get(key) ?? []
        group.push(item)
        groups.set(key, group)
    }
    return new Map([...groups].filter(([, children]) => children.length > 1))
}

export function duplicateContentStatus(items: ContentItem[]) {
    return {
        updates: items.filter(
            (item) =>
                hasContentUpdate(item) &&
                item.instanceMaterializationState === 'present' &&
                item.instanceCapabilities?.canUpdate !== false,
        ).length,
        disabled: items.filter((item) => item.enabled === false).length,
        missing: items.filter(
            (item) =>
                item.instanceMaterializationState === 'missing' ||
                item.instanceMaterializationState === 'removed',
        ).length,
        installing: items.filter((item) => item.installing).length,
    }
}

export function visibleDuplicateGroups(
    groups: Map<string, ContentItem[]>,
    visibleItems: ContentItem[],
    getId: (item: ContentItem) => string,
) {
    const keys = new Map(
        [...groups].flatMap(([key, children]) =>
            children.map((item) => [getId(item), key] as const),
        ),
    )
    const seen = new Set<string>()
    const visible = new Map<string, ContentItem[]>()
    for (const item of visibleItems) {
        const id = getId(item)
        const key = keys.get(id)
        if (!key || seen.has(id)) continue
        seen.add(id)
        const children = visible.get(key) ?? []
        children.push(item)
        visible.set(key, children)
    }
    return visible
}

export function duplicateContentRows(options: {
    groups: Map<string, ContentItem[]>
    visibleGroups: Map<string, ContentItem[]>
    expanded: Set<string>
    getId: (item: ContentItem) => string
    mapItem: (item: ContentItem) => ContentCardTableItem
    summary: (status: ReturnType<typeof duplicateContentStatus>, visible: number) => string
    updateLabel: (count: number) => string
    canUpdate: boolean
    isBusy: (items: ContentItem[]) => boolean
}): ContentCardTableItem[] {
    const rows: ContentCardTableItem[] = []
    for (const [key, children] of options.visibleGroups) {
        const all = options.groups.get(key)!
        const group = `duplicate:${key}`
        const first = options.mapItem(children[0])
        const status = duplicateContentStatus(all)
        rows.push({
            id: group,
            group,
            isGroupHeader: true,
            groupKind: 'duplicate',
            groupChildIds: children.map(options.getId),
            groupItemCount: all.length,
            project: first.project,
            projectLink: first.projectLink,
            groupStatus: options.summary(status, children.length),
            updateLabel: options.updateLabel(all.length),
            hasUpdate: status.updates > 0 && options.canUpdate,
            disabled: options.isBusy(all),
        })
        if (options.expanded.has(group)) {
            rows.push(
                ...children.map((item) => ({
                    ...options.mapItem(item),
                    group,
                    groupKind: 'duplicate' as const,
                })),
            )
        }
    }
    return rows
}

export function duplicateUpdateAnchor(
    rows: ContentCardTableItem[],
    id: string,
): string | undefined {
    const row = rows.find(
        (row) => row.id === id && row.isGroupHeader && row.groupKind === 'duplicate',
    )
    return row?.groupChildIds?.[0]
}
