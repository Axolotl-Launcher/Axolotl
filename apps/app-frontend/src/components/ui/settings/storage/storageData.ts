import type { StorageNode } from '@/helpers/storage'

export type {
    StorageNode,
    StorageNodeType,
    StoragePath,
    StorageSize,
    StorageTree,
} from '@/helpers/storage'

export function sortStorageChildren(children: StorageNode[] | undefined): StorageNode[] {
    if (!children) return []
    return [...children]
        .filter((child) => child.size.actual + child.size.symlink > 0)
        .sort((a, b) => {
            const aIsOther = a.type === 'other' ? 1 : 0
            const bIsOther = b.type === 'other' ? 1 : 0
            if (aIsOther !== bIsOther) return aIsOther - bIsOther
            const aTotal = a.size.actual + a.size.symlink
            const bTotal = b.size.actual + b.size.symlink
            return bTotal - aTotal
        })
}
