import { computed } from 'vue'

import type { ContentItem } from '../../../../lib/state/downloads'
import { MACHINE_TOTAL_MEMORY } from './machine'

/** `automatic_memory_max_mb` of `packages/app-lib/src/api/jre.rs`. */
export function automaticMemoryMaxMb(
    availableMemoryBytes: number,
    modCount: number,
    modded: boolean,
): number {
    const BYTES_PER_GIB = 1024 * 1024 * 1024

    let availableGib = roundTiesEven((availableMemoryBytes / BYTES_PER_GIB) * 10) / 10
    const [minimum, target1, target2, target3] = modded
        ? [0.5 + modCount / 150, 1.5 + modCount / 90, 2.7 + modCount / 50, 4.5 + modCount / 25]
        : [0.5, 1.5, 2.5, 4.0]

    let allocated = 0
    const stages: [number, number][] = [
        [target1, 1.0],
        [target2 - target1, 0.7],
        [target3 - target2, 0.4],
        [target3, 0.15],
    ]
    for (const [delta, ratio] of stages) {
        allocated += Math.min(availableGib * ratio, delta)
        availableGib -= delta / ratio
        if (availableGib < 0.1) break
    }

    const allocatedGib = roundTiesEven(Math.max(allocated, minimum) * 10) / 10
    return Math.max(512, Math.floor(allocatedGib * 1024))
}

function roundTiesEven(value: number) {
    const floor = Math.floor(value)
    const diff = value - floor
    if (Math.abs(diff - 0.5) < 1e-9) return floor % 2 === 0 ? floor : floor + 1
    return Math.round(value)
}

/** The loaders `get_memory_status` counts as modded. */
export function isModdedLoader(loader: string) {
    return [
        'forge',
        'fabric',
        'quilt',
        'neoforge',
        'cleanroom',
        'lite_loader',
        'legacy_fabric',
    ].includes(loader)
}

function folderOf(item: ContentItem) {
    const slash = item.id.lastIndexOf('/')
    if (slash > 0 && item.id.slice(slash + 1) === item.file_name) return item.id.slice(0, slash)
    return item.project_type === 'resourcepack'
        ? 'resourcepacks'
        : item.project_type === 'shader'
          ? 'shaderpacks'
          : item.project_type === 'datapack'
            ? 'datapacks'
            : 'mods'
}

/** `count_mods`: the enabled `.jar`, `.zip` and `.litemod` files in `mods/`. */
export function countMods(items: ContentItem[]) {
    return items.filter(
        (item) =>
            item.enabled &&
            !item.removed &&
            folderOf(item) === 'mods' &&
            /\.(jar|zip|litemod)$/i.test(item.file_name),
    ).length
}

/** The mods a content snapshot lists as present (`get_content_snapshot`). */
export function presentMods(items: ContentItem[]) {
    return items.filter((item) => item.project_type === 'mod' && !item.removed).length
}

/** `composables/useMemorySlider.js`. */
export function useMemorySlider() {
    const maxMemory = computed(() => Math.floor(MACHINE_TOTAL_MEMORY / 1024 / 1024))

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
