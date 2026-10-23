import { computed } from 'vue'

import { useJre } from './jre'

/**
 * `useMemorySlider` (apps/app-frontend/src/composables/useMemorySlider.js): the
 * slider's maximum is the machine's memory (`get_max_memory`, in KiB, as MiB) with
 * snap points at every doubling from 2 GB. The app caches the query for the session,
 * so the value is there when the page renders.
 */
export default function useMemorySlider() {
    const jre = useJre()
    const maxMemory = computed(() => Math.floor(Number(jre.getMaxMemory() ?? 0) / 1024))

    const snapPoints = computed(() => {
        const points: number[] = []
        let memory = 2048

        while (memory <= maxMemory.value) {
            points.push(memory)
            memory *= 2
        }

        return points
    })

    return { maxMemory, snapPoints }
}
