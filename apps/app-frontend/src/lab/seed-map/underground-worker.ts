import type { UndergroundMeshRequest } from './underground'
import { buildUndergroundMesh } from './underground-mesh'

const scope = self as DedicatedWorkerGlobalScope
let textureUvs: UndergroundMeshRequest['textureUvs'] = {}
type Message =
    | { type: 'init'; textureUvs: UndergroundMeshRequest['textureUvs'] }
    | ({ type: 'mesh' } & Omit<UndergroundMeshRequest, 'textureUvs'>)
scope.onmessage = (event: MessageEvent<Message>) => {
    if (event.data.type === 'init') {
        textureUvs = event.data.textureUvs
        return
    }
    try {
        const mesh = buildUndergroundMesh({ ...event.data, textureUvs })
        const buffers = Object.values(mesh.layers).flatMap((layer) =>
            Object.values(layer).map((data) => data.buffer),
        )
        scope.postMessage({ mesh }, buffers)
    } catch (error) {
        scope.postMessage({ error: error instanceof Error ? error.message : String(error) })
    }
}
