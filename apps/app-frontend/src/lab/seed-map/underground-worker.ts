import { errorText } from '@/helpers/error-text'

import type { UndergroundMeshRequest } from './underground'
import { buildUndergroundMesh } from './underground-mesh'

type WorkerScope = {
    onmessage: ((event: MessageEvent<Message>) => void) | null
    postMessage(message: unknown, transfer?: Transferable[]): void
}
const scope = self as unknown as WorkerScope
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
        scope.postMessage({ error: errorText(error) })
    }
}
