import { packHeatmap } from './plot'
import { calculateSectors, type SectorInput } from './sectors'

self.onmessage = (event: MessageEvent<{ id: number; input: SectorInput }>) => {
    try {
        const { distribution, ...result } = calculateSectors(event.data.input)
        const heatmap = packHeatmap(distribution)
        self.postMessage({ id: event.data.id, result, heatmap }, { transfer: [heatmap.buffer] })
    } catch {
        self.postMessage({ id: event.data.id, error: true })
    }
}
