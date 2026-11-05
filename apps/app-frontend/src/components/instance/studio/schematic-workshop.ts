import type { StudioDocument } from './useStudioDocuments'

/**
 * Studio can only edit text and NBT, so both schematic formats land in its
 * "unsupported" state: Litematica (`.litematic`) and Sponge (`.schem`) store
 * gzip-compressed NBT, which `studio_read_text` refuses. The schematic workshop
 * parses exactly those two formats; the legacy MCEdit `.schematic` container is
 * rejected by the preview backend, so offering it here would hand the user a
 * file the destination cannot open.
 */
const SCHEMATIC_FILE = /\.(litematic|schem)$/i

export interface SchematicWorkshopTarget {
    instanceId: string
    path: string
}

/**
 * Decides whether the open Studio document can be handed to the schematic
 * workshop. The workshop resolves an instance file against the instance root,
 * so the document's relative path is passed through unchanged, and the file has
 * to belong to an instance: a server workspace has no instance to resolve it.
 */
export function schematicWorkshopTarget(
    document: Pick<StudioDocument, 'kind' | 'name' | 'path'> | null | undefined,
    instanceId: string | undefined,
): SchematicWorkshopTarget | null {
    if (!document || !instanceId) return null
    if (document.kind !== 'unsupported') return null
    if (!SCHEMATIC_FILE.test(document.name)) return null
    return { instanceId, path: document.path }
}
