import { SeedMapPreviewLimit } from './preview-data.ts'
import type {
    StructureBlocksRequest,
    StructureBlocksResponse,
    StructureTemplatePack,
} from './structure-blocks.ts'
import {
    assembleStructureBlocks,
    seedMapStructureResourceVersion,
    structureChestBlocks,
} from './structure-blocks.ts'

const scope = self as DedicatedWorkerGlobalScope
// Eager: the URLs are plain strings, and a worker bundle cannot split into chunks.
const packs = import.meta.glob<string>('./assets/versions/*/structures.json.gz', {
    query: '?url',
    import: 'default',
    eager: true,
})
scope.onmessage = async (event: MessageEvent<StructureBlocksRequest>) => {
    try {
        const version = seedMapStructureResourceVersion(event.data.version)
        const templatesUrl = packs[`./assets/versions/${version}/structures.json.gz`]
        if (!templatesUrl) throw new Error('No structure assets for this version')
        const response = await fetch(templatesUrl)
        if (!response.ok) throw new Error('Unable to load structure templates')
        const bytes = new Uint8Array(await response.arrayBuffer())
        const decoded =
            bytes[0] === 0x1f && bytes[1] === 0x8b
                ? await new Response(
                      new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip')),
                  ).arrayBuffer()
                : bytes
        const pack = JSON.parse(new TextDecoder().decode(decoded)) as StructureTemplatePack
        if (pack.version !== version) throw new Error('Structure asset version mismatch')
        if (event.data.mode === 'chests') {
            const chests = structureChestBlocks(assembleStructureBlocks(event.data.pieces, pack))
            scope.postMessage({ type: 'chests', chests } satisfies StructureBlocksResponse)
            return
        }
        // Block meshes are built with the desktop app's block models.
        throw new SeedMapPreviewLimit('blocks')
    } catch (error) {
        scope.postMessage({
            type: 'error',
            message: error instanceof Error ? error.message : String(error),
        } satisfies StructureBlocksResponse)
    }
}
