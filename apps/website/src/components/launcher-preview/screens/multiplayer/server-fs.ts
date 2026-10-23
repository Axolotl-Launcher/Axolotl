import { formatPreviewMessage } from '../../lib/i18n'
import { download } from '../../lib/platform/fs'
import { SERVERS_DIR } from '../../lib/state/servers'
import { serverDirectory, type ServerFile, updateServerFiles } from './backend.ts'
import { serverPreviewMessages } from './server-preview-messages.ts'

/**
 * The `@tauri-apps/plugin-fs` and `@tauri-apps/api/path` calls of the server
 * files panel, over the server folders of `backend.ts`. Text files the
 * recorded server wrote have their contents; other files (jars, worlds,
 * archives) are listed with their real sizes but their bytes stay with the
 * desktop app. Writes to a path outside the server folders follow a save
 * panel, so the browser downloads them.
 */

function locate(path: string) {
    const prefix = `${SERVERS_DIR}/`
    if (!path.startsWith(prefix)) return null
    const [id, ...rest] = path.slice(prefix.length).split('/')
    return { id, path: rest.filter(Boolean).join('/') }
}

function unavailable() {
    return new Error(formatPreviewMessage(serverPreviewMessages.contentUnavailable))
}

function missing(path: string) {
    return new Error(`No such file or directory (os error 2): ${path}`)
}

async function entry(path: string): Promise<ServerFile | null> {
    const target = locate(path)
    if (!target) return null
    if (!target.path) return { kind: 'directory', size: 0, modified: Date.now() }
    const parent = target.path.includes('/')
        ? target.path.slice(0, target.path.lastIndexOf('/'))
        : ''
    const children = await serverDirectory(target.id, parent)
    return children.find((child) => child.path === target.path)?.file ?? null
}

export async function join(...parts: string[]): Promise<string> {
    return parts
        .map((part, index) =>
            index === 0 ? part.replace(/\/+$/, '') : part.replace(/^\/+|\/+$/g, ''),
        )
        .filter(Boolean)
        .join('/')
}

export async function readDir(path: string) {
    const target = locate(path)
    if (!target) throw missing(path)
    const children = await serverDirectory(target.id, target.path)
    return children.map((child) => ({
        name: child.name,
        isDirectory: child.file.kind === 'directory',
        isFile: child.file.kind === 'file',
        isSymlink: false,
    }))
}

export async function stat(path: string) {
    const file = await entry(path)
    if (!file) throw missing(path)
    return {
        size: file.size,
        mtime: new Date(file.modified),
        birthtime: new Date(file.modified),
        isDirectory: file.kind === 'directory',
        isFile: file.kind === 'file',
    }
}

export async function mkdir(path: string) {
    const target = locate(path)
    if (!target) throw missing(path)
    await updateServerFiles(target.id, (tree) =>
        tree.set(target.path, { kind: 'directory', size: 0, modified: Date.now() }),
    )
}

export async function writeTextFile(path: string, text: string) {
    const target = locate(path)
    if (!target) {
        download(path, new Blob([text], { type: 'text/plain' }))
        return
    }
    const size = new TextEncoder().encode(text).length
    await updateServerFiles(target.id, (tree) =>
        tree.set(target.path, { kind: 'file', size, modified: Date.now(), content: text }),
    )
}

export async function readTextFile(path: string): Promise<string> {
    const file = await entry(path)
    if (!file) throw missing(path)
    if (file.kind !== 'file' || file.content === undefined) throw unavailable()
    return file.content
}

export async function readFile(path: string): Promise<Uint8Array> {
    return new TextEncoder().encode(await readTextFile(path))
}

export async function remove(path: string, _options: { recursive?: boolean } = {}) {
    const target = locate(path)
    if (!target) throw missing(path)
    await updateServerFiles(target.id, (tree) => {
        for (const key of [...tree.keys()]) {
            if (key === target.path || key.startsWith(`${target.path}/`)) tree.delete(key)
        }
    })
}

export async function rename(from: string, to: string) {
    const source = locate(from)
    const destination = locate(to)
    if (!source || !destination || source.id !== destination.id) throw missing(from)
    await updateServerFiles(source.id, (tree) => {
        for (const [key, file] of [...tree.entries()]) {
            if (key !== source.path && !key.startsWith(`${source.path}/`)) continue
            tree.delete(key)
            tree.set(destination.path + key.slice(source.path.length), file)
        }
    })
}

export async function copyFile(from: string, to: string) {
    const text = await readTextFile(from)
    await writeTextFile(to, text)
}
