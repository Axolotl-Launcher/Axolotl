import { invoke } from '@tauri-apps/api/core'

import type { SingleplayerWorld } from '@/helpers/worlds'

export type DatapackKind = 'folder' | 'zip'

export type WorldDatapack = {
    file_name: string
    display_name: string
    kind: DatapackKind
    pack_format?: number
    supported_formats?: number | number[] | { min_inclusive: number; max_inclusive: number }
    min_format?: [number, number]
    max_format?: [number, number]
    description?: unknown
    icon?: string
    enabled?: boolean
    size: number
    modified?: string
}

export type WorldWithDatapacks = SingleplayerWorld & {
    datapacks: WorldDatapack[]
}

export async function listDatapacks(instanceId: string): Promise<WorldWithDatapacks[]> {
    return await invoke('plugin:datapacks|list_datapacks', { instanceId })
}

export async function deleteDatapack(
    instanceId: string,
    worldPath: string,
    fileName: string,
): Promise<void> {
    return await invoke('plugin:datapacks|delete_datapack', {
        instanceId,
        worldPath,
        fileName,
    })
}

export async function setDatapackEnabled(
    instanceId: string,
    worldPath: string,
    fileName: string,
    enabled: boolean,
): Promise<void> {
    return await invoke('plugin:datapacks|set_datapack_enabled', {
        instanceId,
        worldPath,
        fileName,
        enabled,
    })
}
