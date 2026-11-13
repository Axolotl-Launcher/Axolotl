import type { FileItem } from '@modrinth/ui'
import { sep } from '@tauri-apps/api/path'
import { readDir, stat } from '@tauri-apps/plugin-fs'

/** Read this directory before loading optional child counts. */
export async function readInstanceDirectory(
    absolutePath: string,
    relativePath: string,
    isCurrent: () => boolean,
): Promise<FileItem[]> {
    const entries = await readDir(absolutePath)
    const separator = sep()
    const prefix = absolutePath.endsWith(separator) ? absolutePath : `${absolutePath}${separator}`
    const results: Array<FileItem | undefined> = new Array(entries.length)
    let cursor = 0

    // Large folders otherwise queue thousands of metadata requests in the webview.
    await Promise.all(
        Array.from({ length: Math.min(16, entries.length) }, async () => {
            while (isCurrent() && cursor < entries.length) {
                const index = cursor++
                const entry = entries[index]
                try {
                    const metadata = await stat(`${prefix}${entry.name}`)
                    if (!isCurrent()) return
                    results[index] = {
                        name: entry.name,
                        type: entry.isDirectory ? 'directory' : 'file',
                        path: relativePath ? `${relativePath}/${entry.name}` : entry.name,
                        modified: metadata.mtime ? Math.floor(metadata.mtime.getTime() / 1000) : 0,
                        created: metadata.birthtime
                            ? Math.floor(metadata.birthtime.getTime() / 1000)
                            : 0,
                        ...(!entry.isDirectory ? { size: metadata.size } : {}),
                    }
                } catch {
                    // A directory entry may disappear or become unreadable while listing.
                }
            }
        }),
    )
    return results.filter((item): item is FileItem => item !== undefined)
}

/** Fill folder counts after the list is visible, without flooding the native queue. */
export async function readInstanceDirectoryCounts(
    absolutePath: string,
    items: FileItem[],
    isCurrent: () => boolean,
): Promise<void> {
    const separator = sep()
    const prefix = absolutePath.endsWith(separator) ? absolutePath : `${absolutePath}${separator}`
    for (const item of items) {
        if (!isCurrent()) return
        if (item.type !== 'directory') continue
        try {
            const children = await readDir(`${prefix}${item.name}`)
            if (!isCurrent()) return
            item.count = children.length
        } catch {
            // An unreadable folder's count remains unknown.
        }
    }
}
