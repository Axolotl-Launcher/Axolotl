import { computed, ref, watch } from 'vue'

import { projectIcon } from '../../data'
import { type PreviewState, usePreviewDomain } from '../../lib/state'
import type { PreviewScreenshot, ScreenshotGroupMode } from '../../lib/state/screenshots'

export type ScreenshotSort = 'newest' | 'oldest' | 'name'
export type ScreenshotGroupBy = ScreenshotGroupMode

export type ScreenshotGroupData = {
    id: string
    title: string
    screenshots: PreviewScreenshot[]
    dropInstanceId?: string
    customGroupId?: string | null
}

export type ScreenshotDragData = {
    selectionKey: string
    instanceId: string
}

export type ScreenshotDropData = {
    groupId: string
    instanceId?: string
    customGroupId?: string | null
}

/** `[instance_id, file_name]`, the app's selection identity for a screenshot. */
export function getSelectionKey(screenshot: PreviewScreenshot): string {
    return JSON.stringify([screenshot.instance_id, fileName(screenshot)])
}

export function fileName(screenshot: PreviewScreenshot): string {
    const path = screenshot.path
    return path.slice(path.lastIndexOf('/') + 1)
}

/** The gallery folder of the modpack each instance was built from (snapshot). */
const instanceProjects: Record<string, string> = {
    'fabulously-optimized': '1KVo5zza',
    cobblemon: '5FFgwNNP',
    'better-mc': 'shFhR8Vx',
    'zombie-invade': 'l9m9tuPN',
}

/**
 * Stills of each gallery present in the snapshot. A capture whose own still was
 * not captured falls back to the next still of the same modpack — and Zombie
 * Invade's modpack has no gallery at all, so its captures use the oldest
 * Fabulously Optimized stills (the same library the folder belongs to).
 */
const galleryFiles: Record<string, string[]> = {
    '1KVo5zza': [0, 1, 2, 3, 4, 5, 6, 7, 8].map(
        (n) => `/launcher-preview/galleries/1KVo5zza/${n}.webp`,
    ),
    '5FFgwNNP': Array.from(
        { length: 27 },
        (_, n) => `/launcher-preview/galleries/5FFgwNNP/${n}.webp`,
    ),
    shFhR8Vx: ['/launcher-preview/galleries/shFhR8Vx/0.webp'],
}

export function screenshotSources(screenshot: PreviewScreenshot) {
    const file = screenshot.path
    const gallery = file.includes('/galleries/')
        ? file
        : file.replace('/screenshots/', '/galleries/')
    const project = instanceProjects[screenshot.instance_id] ?? '1KVo5zza'
    const files = galleryFiles[project] ?? galleryFiles['1KVo5zza']
    if (files.includes(gallery)) return { file, gallery }

    const index = Number(gallery.slice(gallery.lastIndexOf('/') + 1).replace(/\.webp$/, ''))
    const fallback = files[Math.abs(Number.isFinite(index) ? index : 0) % files.length]
    return { file, gallery: fallback }
}

export function instanceIcon(instanceId: string): string | undefined {
    const icons: Record<string, string> = {
        'fabulously-optimized': projectIcon('1KVo5zza'),
        cobblemon: projectIcon('5FFgwNNP'),
        'better-mc': projectIcon('shFhR8Vx'),
        'zombie-invade': projectIcon('l9m9tuPN'),
    }
    return icons[instanceId]
}

export const instanceNames: Record<string, string> = {
    'fabulously-optimized': 'Fabulously Optimized',
    cobblemon: 'Cobblemon Official Modpack [Fabric]',
    'better-mc': 'Better MC [FABRIC] - BMC2',
    'zombie-invade': 'Zombie Invade 100 Days',
}

/**
 * The snapshot seeds nine captures; the page's lists, groups and marquee need a
 * fuller library, so the seeded ones are joined by other real gallery stills of
 * the same modpacks (the same folders the instances are built from).
 */
const extraCaptures: [string, string, number, number, string | null][] = [
    ['fabulously-optimized', '1KVo5zza', 3, 7 * 24 * 3600 * 1000, '建筑'],
    ['fabulously-optimized', '1KVo5zza', 4, 9 * 24 * 3600 * 1000, '建筑'],
    ['fabulously-optimized', '1KVo5zza', 5, 12 * 24 * 3600 * 1000, null],
    ['fabulously-optimized', '1KVo5zza', 6, 26 * 24 * 3600 * 1000, null],
    ['fabulously-optimized', '1KVo5zza', 7, 31 * 24 * 3600 * 1000, null],
    ['fabulously-optimized', '1KVo5zza', 8, 40 * 24 * 3600 * 1000, null],
    ['cobblemon', '5FFgwNNP', 3, 3 * 24 * 3600 * 1000, '宝可梦'],
    ['cobblemon', '5FFgwNNP', 4, 6 * 24 * 3600 * 1000, '宝可梦'],
    ['cobblemon', '5FFgwNNP', 5, 11 * 24 * 3600 * 1000, null],
    ['cobblemon', '5FFgwNNP', 6, 13 * 24 * 3600 * 1000, null],
    ['cobblemon', '5FFgwNNP', 7, 17 * 24 * 3600 * 1000, null],
    ['better-mc', 'shFhR8Vx', 1, 15 * 24 * 3600 * 1000, null],
]

const BASE_TIME = Date.UTC(2026, 6, 29, 10, 0, 0)

function seedExtraCaptures(preview: PreviewState) {
    const { screenshots } = preview.screenshots
    if (screenshots.value.length !== 9) return
    const existing = new Set(screenshots.value.map((shot) => shot.path))
    const extra = extraCaptures
        .filter(
            ([, project, index]) =>
                !existing.has(`/launcher-preview/galleries/${project}/${index}.webp`),
        )
        .map(([instanceId, project, index, ago, group]) => ({
            id: `ss-x-${project}-${index}`,
            instance_id: instanceId,
            instance_name: instanceNames[instanceId],
            path: `/launcher-preview/galleries/${project}/${index}.webp`,
            taken_at: new Date(BASE_TIME - ago).toISOString(),
            width: 1920,
            height: 1080,
            group,
        }))
    screenshots.value = [...screenshots.value, ...extra]
}

function createScreenshotsPageState(preview: PreviewState) {
    seedExtraCaptures(preview)
    const screenshots = preview.screenshots

    /** Canvas snapshots of screenshots saved from the viewer's editor. */
    const editedSources = ref<Record<string, string>>({})
    const sort = ref<ScreenshotSort>('newest')
    const collapsedGroups = ref<Record<string, boolean>>({})
    const copiedScreenshotIds = ref<Set<string>>(new Set())
    const saving = ref(false)
    /** Whether the library has been scanned in this window (the app's query cache). */
    const ready = ref(false)
    /** Instance folders the scan could not read, as `skipped_instances` reports. */
    const skippedFolders = ref<string[]>([])
    const lastSaved = ref<PreviewScreenshot | null>(null)
    /** IDs the app just revealed after saving an edit or navigating with `?focus`. */
    const highlightedScreenshotId = ref<string | undefined>()

    const sortModel = computed<string>({
        get: () => sort.value,
        set: (value) => {
            sort.value = value as ScreenshotSort
        },
    })
    const groupByModel = computed<string>({
        get: () => screenshots.groupMode.value,
        set: (value) => {
            screenshots.groupMode.value = value as ScreenshotGroupBy
        },
    })

    function imageSource(screenshot: PreviewScreenshot): string {
        return editedSources.value[screenshot.id] ?? screenshotSources(screenshot).gallery
    }

    function replaceImage(screenshot: PreviewScreenshot, src: string) {
        editedSources.value = { ...editedSources.value, [screenshot.id]: src }
    }

    function markCopied(id: string) {
        copiedScreenshotIds.value = new Set([...copiedScreenshotIds.value, id])
        // The app resets the copied chip after two seconds (`markScreenshotCopied`).
        preview.clock.after(2000, () => {
            const next = new Set(copiedScreenshotIds.value)
            next.delete(id)
            copiedScreenshotIds.value = next
        })
    }

    function setGroupCollapsed(groupId: string, collapsed: boolean) {
        collapsedGroups.value = { ...collapsedGroups.value, [groupId]: collapsed }
    }

    function highlight(id: string) {
        highlightedScreenshotId.value = id
        preview.clock.after(2400, () => {
            if (highlightedScreenshotId.value === id) highlightedScreenshotId.value = undefined
        })
    }

    /**
     * `instance_save_edited_screenshot`: `create_copy` writes a new capture next
     * to the original, `replace_edit` overwrites it. Both re-read the PNG the
     * canvas produced, so cards, groups and the viewer show the edited pixels.
     */
    function saveEdit(
        screenshot: PreviewScreenshot,
        pngBytes: Uint8Array,
        mode: 'create_copy' | 'replace_edit',
    ): Promise<PreviewScreenshot> {
        saving.value = true
        const url = URL.createObjectURL(new Blob([pngBytes], { type: 'image/png' }))
        return new Promise((resolve) => {
            preview.clock.after(600, () => {
                saving.value = false
                if (mode === 'replace_edit') {
                    replaceImage(screenshot, url)
                    lastSaved.value = screenshot
                    resolve(screenshot)
                    return
                }
                const copy: PreviewScreenshot = {
                    ...screenshot,
                    id: `${screenshot.id}-copy-${Date.now()}`,
                    path: screenshot.path.replace(
                        /([^/]+)\.webp$/,
                        (_match, name: string) => `${name}-edited.webp`,
                    ),
                    taken_at: new Date().toISOString(),
                }
                editedSources.value = { ...editedSources.value, [copy.id]: url }
                screenshots.screenshots.value = [copy, ...screenshots.screenshots.value]
                lastSaved.value = copy
                resolve(copy)
            })
        })
    }

    /**
     * `instance_move_screenshots`: the file is copied into the target instance's
     * folder and removed from the source one. The preview shows the same result
     * by moving the capture to the target instance's list.
     */
    function moveToInstance(ids: string[], targetInstanceId: string) {
        const name = instanceNames[targetInstanceId]
        screenshots.screenshots.value = screenshots.screenshots.value.map((screenshot) =>
            ids.includes(screenshot.id)
                ? {
                      ...screenshot,
                      instance_id: targetInstanceId,
                      instance_name: name ?? targetInstanceId,
                      group: null,
                  }
                : screenshot,
        )
    }

    // The scan reports one instance whose folder it cannot read, the app's
    // `skipped_instances` warning path.
    skippedFolders.value = ['zombie-invade']

    watch(screenshots.groupMode, () => {
        collapsedGroups.value = {}
    })

    return {
        preview,
        screenshots,
        saveEdit,
        moveToInstance,
        saving,
        lastSaved,
        ready,
        skippedFolders,
        sort,
        sortModel,
        groupByModel,
        collapsedGroups,
        copiedScreenshotIds,
        highlightedScreenshotId,
        imageSource,
        replaceImage,
        markCopied,
        setGroupCollapsed,
        highlight,
    }
}

export type ScreenshotsPageState = ReturnType<typeof createScreenshotsPageState>

export function useScreenshotsPageState(): ScreenshotsPageState {
    return usePreviewDomain('screenshots-page', (state) => ({
        value: createScreenshotsPageState(state),
    }))
}
