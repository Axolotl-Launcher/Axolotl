import { formatPreviewMessage } from '../../../lib/i18n'
import { SERVERS_DIR } from '../../../lib/state/servers'
import type { GameInstance } from '../../../lib/types'
import * as serverFs from '../../multiplayer/server-fs'
import { type FsDir, nodeAt } from '../files/instance-files'
import { createInstanceFs } from '../files/instance-fs'
import { filesPreviewMessages } from '../messages'

/**
 * The folder Studio edits, with paths relative to its root: the file calls of
 * `FileStudio.vue` (`readDir`, `readStudioText`, `writeTextFile`, `mkdir`,
 * `copyFile`, `rename`, `trashStudioFile` / `remove`) on the preview's copy of
 * an instance folder or on a managed server's folder.
 */
export interface StudioWorkspace {
    /** Absolute path of the root, for "Open path". */
    root: string
    readDir: (path: string) => Promise<{ name: string; isDirectory: boolean }[]>
    readText: (path: string) => Promise<string>
    writeText: (path: string, content: string) => Promise<void>
    mkdir: (path: string) => Promise<void>
    copyFile: (source: string, destination: string) => Promise<void>
    rename: (source: string, destination: string) => Promise<void>
    remove: (path: string) => Promise<void>
    /** The address an image file is shown from, when the preview has its pixels. */
    mediaUrl: (path: string) => string | null
}

export function instanceWorkspace(instance: GameInstance, tree: () => FsDir): StudioWorkspace {
    const fs = createInstanceFs(tree, {
        folder: () => formatPreviewMessage(filesPreviewMessages.folderUnavailable),
        content: () => formatPreviewMessage(filesPreviewMessages.contentUnavailable),
    })
    return {
        root: instance.path,
        async readDir(path) {
            return fs.directoryAt(path).children.map((child) => ({
                name: child.name,
                isDirectory: child.kind === 'dir',
            }))
        },
        async readText(path) {
            return fs.readText(path)
        },
        async writeText(path, content) {
            fs.writeText(path, content)
        },
        async mkdir(path) {
            fs.create(path, 'directory')
        },
        async copyFile(source, destination) {
            fs.copy(source, destination)
        },
        async rename(source, destination) {
            fs.move(source, destination)
        },
        async remove(path) {
            fs.detach(path)
        },
        mediaUrl(path) {
            const node = nodeAt(tree(), path)
            return node?.kind === 'file' ? (node.url ?? null) : null
        },
    }
}

export function serverWorkspace(serverId: string): StudioWorkspace {
    const root = `${SERVERS_DIR}/${serverId}`
    const resolve = (path: string) => serverFs.join(root, ...path.split('/'))
    return {
        root,
        async readDir(path) {
            return serverFs.readDir(await resolve(path))
        },
        async readText(path) {
            return serverFs.readTextFile(await resolve(path))
        },
        async writeText(path, content) {
            await serverFs.writeTextFile(await resolve(path), content)
        },
        async mkdir(path) {
            await serverFs.mkdir(await resolve(path))
        },
        async copyFile(source, destination) {
            await serverFs.copyFile(await resolve(source), await resolve(destination))
        },
        async rename(source, destination) {
            await serverFs.rename(await resolve(source), await resolve(destination))
        },
        async remove(path) {
            await serverFs.remove(await resolve(path), { recursive: true })
        },
        mediaUrl: () => null,
    }
}
