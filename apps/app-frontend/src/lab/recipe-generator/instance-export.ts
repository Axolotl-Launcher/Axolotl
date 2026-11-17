// 由 S4 集成
import { invoke } from '@/helpers/invoke'

import { createDatapackBlob, type PackFile } from './datapack.ts'

function normalizePackFileName(fileName: string): string {
    const segments = fileName.replaceAll('\\', '/').split('/').filter(Boolean)
    const safeName = segments[segments.length - 1] ?? fileName
    return safeName.toLowerCase().endsWith('.zip') ? safeName : `${safeName}.zip`
}

/**
 * Installs a generated datapack into a singleplayer world's `datapacks` directory.
 * Returns the installed path relative to the instance root.
 */
export async function exportDatapackToWorld(
    instanceId: string,
    worldPath: string,
    files: PackFile[],
    fileName = 'axolotl-recipes.zip',
): Promise<string> {
    const blob = createDatapackBlob(files)
    const bytes = new Uint8Array(await blob.arrayBuffer())
    return await invoke<string>('plugin:instance|instance_install_datapack_to_world_bytes', {
        instanceId,
        worldPath,
        fileName: normalizePackFileName(fileName),
        bytes,
    })
}
