import { invoke } from '@tauri-apps/api/core'

export type ShimuSchematicSort = 'hot' | 'new' | 'featured'

export interface ShimuSchematic {
    source: 'shimu'
    postId: number
    title: string
    summary: string
    author: string
    cover: string | null
    gameVersions: string[]
    files: { name: string; size: number | null }[]
}

export interface ShimuSchematicDetail extends ShimuSchematic {
    description: string
}

export function getShimuSchematic(postId: number): Promise<ShimuSchematicDetail> {
    return invoke('plugin:third-party-accounts|shimu_get_schematic', { postId })
}

export function listShimuSchematics(
    page: number,
    query: string,
    pageSize: number,
    sort: ShimuSchematicSort,
): Promise<{ items: ShimuSchematic[]; pageCount: number }> {
    return invoke('plugin:third-party-accounts|shimu_list_schematics', {
        page,
        query,
        pageSize,
        sort,
    })
}

export function installShimuSchematic(request: {
    instanceId: string
    postId: number
    fileName: string
}): Promise<string> {
    return invoke('plugin:third-party-accounts|shimu_install_schematic', request)
}
