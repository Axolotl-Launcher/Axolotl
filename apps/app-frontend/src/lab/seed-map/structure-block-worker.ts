import { gunzipSync, strFromU8 } from 'fflate'

import { errorText } from '@/helpers/error-text'

import type {
    StructureBlocksRequest,
    StructureBlocksResponse,
    StructureTemplatePack,
} from './structure-blocks'
import {
    assembleStructureBlocks,
    seedMapStructureResourceVersion,
    structureChestBlocks,
} from './structure-blocks'

type WorkerScope = {
    onmessage: ((event: MessageEvent<StructureBlocksRequest>) => void) | null
    postMessage(message: unknown, transfer?: Transferable[]): void
}
const scope = self as unknown as WorkerScope
const packs = import.meta.glob('./assets/versions/*/structures.json.gz', {
    query: '?url',
    import: 'default',
})
scope.onmessage = async (event: MessageEvent<StructureBlocksRequest>) => {
    try {
        const version = seedMapStructureResourceVersion(event.data.version)
        const load = packs[`./assets/versions/${version}/structures.json.gz`]
        if (!load) throw new Error('No structure assets for this version')
        const templatesUrl = (await load()) as string
        const response = await fetch(templatesUrl)
        if (!response.ok) throw new Error('Unable to load structure templates')
        const bytes = new Uint8Array(await response.arrayBuffer())
        const decoded = bytes[0] === 0x1f && bytes[1] === 0x8b ? gunzipSync(bytes) : bytes
        const pack = JSON.parse(strFromU8(decoded)) as StructureTemplatePack
        if (pack.version !== version) throw new Error('Structure asset version mismatch')
        if (event.data.mode === 'chests') {
            const chests = structureChestBlocks(assembleStructureBlocks(event.data.pieces, pack))
            scope.postMessage({ type: 'chests', chests } satisfies StructureBlocksResponse)
            return
        }
        const { buildStructureBlockMeshes } = await import('./structure-block-mesh')
        const result = buildStructureBlockMeshes(event.data.pieces, pack, event.data.sparseBlocks)
        scope.postMessage(
            { type: 'ready', result } satisfies StructureBlocksResponse,
            result.meshes.flatMap((mesh) => [mesh.positions.buffer, mesh.colors.buffer]),
        )
    } catch (error) {
        scope.postMessage({
            type: 'error',
            message: errorText(error),
        } satisfies StructureBlocksResponse)
    }
}
