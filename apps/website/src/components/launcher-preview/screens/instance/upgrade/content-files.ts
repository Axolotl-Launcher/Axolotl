import type { ContentItem } from '../../../lib/state/downloads'
import type { FsDir, FsNode } from '../files/instance-files'

function contentPath(item: ContentItem) {
    const folder =
        item.folder ??
        {
            mod: 'mods',
            resourcepack: 'resourcepacks',
            shader: 'shaderpacks',
            datapack: 'datapacks',
        }[item.project_type] ??
        'mods'
    return [
        ...folder.split('/').filter(Boolean),
        `${item.file_name}${item.enabled ? '' : '.disabled'}`,
    ]
}

/** Replace only managed content paths; retain configuration and visitor-created files. */
export function reconcileContentFiles(
    root: FsDir,
    before: ContentItem[],
    after: ContentItem[],
    now: number,
) {
    function folderAt(parts: string[], create: boolean): FsDir | null {
        let folder = root
        for (const part of parts) {
            let child = folder.children?.find((entry) => entry.name === part)
            if (!child && create) {
                child = { kind: 'dir', name: part, children: [], created: now, modified: now }
                ;(folder.children ??= []).push(child)
            }
            if (!child || child.kind !== 'dir') return null
            folder = child
        }
        return folder
    }
    const preserved = new Map<string, FsNode>()
    for (const item of before.filter((entry) => !entry.removed)) {
        const parts = contentPath(item)
        const name = parts.pop()!
        const folder = folderAt(parts, false)
        const node = folder?.children?.find((entry) => entry.name === name)
        if (node) preserved.set([...parts, name].join('/'), node)
        if (folder?.children)
            folder.children = folder.children.filter((entry) => entry.name !== name)
    }
    for (const item of after.filter((entry) => !entry.removed)) {
        const parts = contentPath(item)
        const name = parts.pop()!
        const folder = folderAt(parts, true)
        if (!folder) continue
        const existing = preserved.get([...parts, name].join('/'))
        const file: FsNode = existing ?? {
            kind: 'file',
            name,
            size: item.size,
            created: now,
            modified: now,
        }
        folder.children = [...(folder.children ?? []).filter((entry) => entry.name !== name), file]
    }
}
