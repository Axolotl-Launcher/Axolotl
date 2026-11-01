import { isSchematicSource } from '@/helpers/schematics'

import type { SchematicPreviewSource } from './backend'

const RECENT_KEY = 'axolotl:lab:schematic-preview:recent:v1'
const MAX_RECENT = 5

export type RecentSchematic = {
    id: string
    source: SchematicPreviewSource
    fileName: string
    openedAt: number
}

function safeParse<T>(key: string, fallback: T): T {
    try {
        return JSON.parse(localStorage.getItem(key) ?? '') as T
    } catch {
        return fallback
    }
}

function sourceId(source: SchematicPreviewSource) {
    if (source.kind === 'remote') return `${source.source}:${source.projectId}:${source.fileName}`
    if (source.kind === 'shimu') return `shimu:${source.postId}:${source.fileName}`
    return source.kind === 'external'
        ? `external:${source.path}`
        : `${source.kind}:${source.instanceId}:${source.relativePath}`
}

export function loadRecentSchematics(): RecentSchematic[] {
    const records = safeParse<RecentSchematic[]>(RECENT_KEY, [])
    if (!Array.isArray(records)) return []
    return records
        .filter(
            (record) =>
                record &&
                typeof record.id === 'string' &&
                typeof record.fileName === 'string' &&
                typeof record.openedAt === 'number' &&
                (record.source?.kind === 'external' ||
                    (record.source?.kind === 'remote' &&
                        isSchematicSource(record.source.source) &&
                        typeof record.source.projectId === 'string' &&
                        typeof record.source.fileName === 'string') ||
                    record.source?.kind === 'instance' ||
                    record.source?.kind === 'instance_file' ||
                    (record.source?.kind === 'shimu' &&
                        Number.isSafeInteger(record.source.postId) &&
                        record.source.postId > 0 &&
                        typeof record.source.fileName === 'string')),
        )
        .slice(0, MAX_RECENT)
}

export function recordRecentSchematic(
    source: SchematicPreviewSource,
    fileName: string,
): RecentSchematic[] {
    const id = sourceId(source)
    const records = loadRecentSchematics().filter((record) => record.id !== id)
    records.unshift({ id, source, fileName, openedAt: Date.now() })
    const next = records.slice(0, MAX_RECENT)
    localStorage.setItem(RECENT_KEY, JSON.stringify(next))
    return next
}

export function removeRecentSchematic(id: string): RecentSchematic[] {
    const next = loadRecentSchematics().filter((record) => record.id !== id)
    localStorage.setItem(RECENT_KEY, JSON.stringify(next))
    return next
}

export function clearRecentSchematics(): RecentSchematic[] {
    localStorage.removeItem(RECENT_KEY)
    return []
}
