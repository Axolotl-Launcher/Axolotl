import { hyphenateUuid } from '../../../lib/offline-uuid'
import { type PreviewState, usePreview, usePreviewDomain } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'
import {
    type DiskDir,
    type DiskEntry,
    findEntry,
    screenshotName,
} from '../../settings/categories/storage/launcherDisk'
import { useStorageBackend } from '../../settings/categories/storage/storageBackend'
import { type InstanceLogsDomain, useInstanceLogs } from '../logs/state'

/**
 * The instance folder the Files tab browses: the simulated launcher disk the
 * Storage settings measure (`launcherDisk.ts`), copied once per instance so the
 * visitor can create, rename, move, delete and edit entries in it. The `logs`
 * folder lists the same files as the Logs tab, and screenshots open as the real
 * captures. A file whose contents the preview does not have fails to open the
 * way an unreadable file does in the app.
 */
export interface FsFile {
    kind: 'file'
    name: string
    size: number
    modified: number
    created: number
    /** Text the visitor wrote or the preview knows. */
    text?: string
    /** Image shown for the file. */
    url?: string
}

export interface FsDir {
    kind: 'dir'
    name: string
    modified: number
    created: number
    /** `null` for a folder whose entries the preview does not model. */
    children: FsNode[] | null
}

export type FsNode = FsFile | FsDir

/** The error of a folder or file the preview has no entries or contents for. */
export class PreviewContentUnavailable extends Error {}

function regionFiles(entry: DiskDir): FsFile[] {
    const count = Math.max(1, entry.summary?.files ?? 1)
    const size = Math.round((entry.summary?.bytes ?? 0) / count)
    const files: FsFile[] = []
    // Region files spread out from the spawn region, ring by ring.
    for (let ring = 0; files.length < count; ring += 1) {
        for (let x = -ring; x <= ring && files.length < count; x += 1) {
            for (let z = -ring; z <= ring && files.length < count; z += 1) {
                if (Math.max(Math.abs(x), Math.abs(z)) !== ring) continue
                files.push({
                    kind: 'file',
                    name: `r.${x}.${z}.mca`,
                    size,
                    modified: entry.modified,
                    created: entry.modified,
                })
            }
        }
    }
    return files
}

function expandPacked(entry: DiskDir, playerId: string | null): FsNode[] | null {
    switch (entry.name) {
        case 'region':
        case 'entities':
        case 'poi':
            return regionFiles(entry)
        case 'playerdata':
            if (!playerId) return null
            return ['dat', 'dat_old'].map((extension) => ({
                kind: 'file' as const,
                name: `${hyphenateUuid(playerId)}.${extension}`,
                size: Math.round((entry.summary?.bytes ?? 0) / 2),
                modified: entry.modified,
                created: entry.modified,
            }))
        default:
            return null
    }
}

function fromDisk(entry: DiskEntry, playerId: string | null): FsNode {
    if (entry.kind === 'file') {
        return {
            kind: 'file',
            name: entry.name,
            size: entry.size,
            modified: entry.modified,
            created: entry.modified,
        }
    }
    if (entry.kind === 'link') {
        return {
            kind: 'dir',
            name: entry.name,
            modified: entry.modified,
            created: entry.modified,
            children: null,
        }
    }
    return {
        kind: 'dir',
        name: entry.name,
        modified: entry.modified,
        created: entry.modified,
        children:
            entry.children === null
                ? expandPacked(entry, playerId)
                : entry.children.map((child) => fromDisk(child, playerId)),
    }
}

function buildInstanceTree(
    state: PreviewState,
    disk: DiskDir,
    instance: GameInstance,
    logs: InstanceLogsDomain,
): FsDir {
    const playerId = state.accounts.selected.value?.profile.id ?? null
    const found = findEntry(disk, instance.path)
    const root: FsDir =
        found && found.kind === 'dir'
            ? (fromDisk(found, playerId) as FsDir)
            : {
                  kind: 'dir',
                  name: instance.name,
                  modified: Date.now(),
                  created: Date.now(),
                  children: [],
              }
    root.children ??= []

    // The Logs tab's files, with their contents.
    const logFiles = logs.filesFor(instance.id)
    const logsFolder: FsDir = {
        kind: 'dir',
        name: 'logs',
        modified: Math.max(0, ...logFiles.map((file) => Date.parse(file.modified))),
        created: Date.parse(instance.created),
        children: logFiles
            .filter((file) => file.folder === 'logs')
            .map((file) => ({
                kind: 'file' as const,
                name: file.filename,
                size: file.size,
                modified: Date.parse(file.modified),
                created: Date.parse(file.modified),
                text: file.filename.endsWith('.gz') ? undefined : file.lines.join('\n'),
            })),
    }
    root.children = [
        ...root.children.filter((child) => child.name !== 'logs'),
        ...(logFiles.length ? [logsFolder] : []),
    ]

    // Screenshots open as the real captures of the Screenshots page.
    const shots = state.screenshots.screenshots.value.filter(
        (shot) => shot.instance_id === instance.id,
    )
    const screenshots = root.children.find((child) => child.name === 'screenshots')
    if (screenshots?.kind === 'dir' && screenshots.children) {
        for (const child of screenshots.children) {
            const shot = shots.find(
                (item) => screenshotName(Date.parse(item.taken_at)) === child.name,
            )
            if (shot && child.kind === 'file') child.url = shot.path
        }
    }
    return root
}

export function splitPath(path: string): string[] {
    return path.split('/').filter(Boolean)
}

export function nodeAt(root: FsDir, path: string): FsNode | null {
    let node: FsNode = root
    for (const part of splitPath(path)) {
        if (node.kind !== 'dir' || !node.children) return null
        const next: FsNode | undefined = node.children.find((child) => child.name === part)
        if (!next) return null
        node = next
    }
    return node
}

/** The editable copy of each instance's folder, kept while the window is open. */
export function useInstanceFiles() {
    const backend = useStorageBackend()
    const logs = useInstanceLogs()
    const trees = usePreviewDomain('instance-files', () => ({ value: new Map<string, FsDir>() }))
    const preview = usePreview()

    function treeFor(instance: GameInstance): FsDir {
        let tree = trees.get(instance.id)
        if (!tree) {
            tree = buildInstanceTree(preview, backend.currentDisk(), instance, logs)
            trees.set(instance.id, tree)
        }
        return tree
    }

    return { treeFor }
}
