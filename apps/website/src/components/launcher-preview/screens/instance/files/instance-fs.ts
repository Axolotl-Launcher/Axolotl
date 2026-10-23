import {
    type FsDir,
    type FsFile,
    type FsNode,
    nodeAt,
    PreviewContentUnavailable,
    splitPath,
} from './instance-files'

/** `No such file or directory`, as the OS reports a path that is not there. */
export function missing(path: string) {
    return new Error(`No such file or directory (os error 2): ${path}`)
}

function exists() {
    return new Error('File exists (os error 17)')
}

export function parentOf(path: string) {
    const parts = splitPath(path)
    return { parent: parts.slice(0, -1).join('/'), name: parts[parts.length - 1] ?? '' }
}

function clone(node: FsNode): FsNode {
    return node.kind === 'file'
        ? { ...node }
        : { ...node, children: node.children?.map(clone) ?? null }
}

/**
 * The file operations the app runs on an instance folder (`@tauri-apps/plugin-fs`),
 * over the folder's editable copy. Paths are relative to the instance root. A
 * folder or file whose entries or contents the preview does not have fails with
 * `PreviewContentUnavailable` and the given explanation.
 */
export function createInstanceFs(
    root: () => FsDir,
    unavailable: { folder: () => string; content: () => string },
) {
    function directoryAt(path: string): FsDir & { children: FsNode[] } {
        const node = nodeAt(root(), path)
        if (!node || node.kind !== 'dir') throw missing(path)
        if (!node.children) throw new PreviewContentUnavailable(unavailable.folder())
        return node as FsDir & { children: FsNode[] }
    }

    function fileAt(path: string): FsFile {
        const node = nodeAt(root(), path)
        if (!node || node.kind !== 'file') throw missing(path)
        return node
    }

    function detach(path: string): FsNode {
        const { parent, name } = parentOf(path)
        const folder = directoryAt(parent)
        const index = folder.children.findIndex((child) => child.name === name)
        if (index === -1) throw missing(path)
        const [node] = folder.children.splice(index, 1)
        folder.modified = Date.now()
        return node
    }

    function attach(path: string, node: FsNode) {
        const { parent, name } = parentOf(path)
        const folder = directoryAt(parent)
        if (folder.children.some((child) => child.name === name)) throw exists()
        node.name = name
        folder.children.push(node)
        folder.modified = Date.now()
    }

    /** `rename`: moves the entry, putting it back when the destination is taken. */
    function move(source: string, destination: string) {
        const node = detach(source)
        try {
            attach(destination, node)
        } catch (error) {
            attach(source, node)
            throw error
        }
    }

    function create(path: string, type: 'file' | 'directory') {
        const now = Date.now()
        attach(
            path,
            type === 'directory'
                ? { kind: 'dir', name: '', modified: now, created: now, children: [] }
                : { kind: 'file', name: '', size: 0, modified: now, created: now, text: '' },
        )
    }

    /** `copyFile` / a recursive copy: the copy keeps the contents the preview has. */
    function copy(source: string, destination: string) {
        const node = nodeAt(root(), source)
        if (!node) throw missing(source)
        const now = Date.now()
        attach(destination, { ...clone(node), modified: now, created: now })
    }

    function readText(path: string): string {
        const node = fileAt(path)
        if (node.text === undefined) throw new PreviewContentUnavailable(unavailable.content())
        return node.text
    }

    async function readBlob(path: string): Promise<Blob> {
        const node = fileAt(path)
        if (node.url) return await (await fetch(node.url)).blob()
        if (node.text !== undefined) return new Blob([node.text])
        throw new PreviewContentUnavailable(unavailable.content())
    }

    function writeText(path: string, content: string) {
        const node = nodeAt(root(), path)
        const size = new Blob([content]).size
        if (node?.kind === 'file') {
            node.text = content
            node.size = size
            node.modified = Date.now()
            return
        }
        const now = Date.now()
        attach(path, { kind: 'file', name: '', size, modified: now, created: now, text: content })
    }

    return {
        directoryAt,
        fileAt,
        detach,
        attach,
        move,
        create,
        copy,
        readText,
        readBlob,
        writeText,
    }
}

export type InstanceFs = ReturnType<typeof createInstanceFs>
