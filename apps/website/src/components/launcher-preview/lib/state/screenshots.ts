import { computed, ref } from 'vue'

import screenshotSnapshot from '../../data/screenshots.json'
import type { PreviewClock } from '../clock'

export type ScreenshotGroupMode = 'custom' | 'instance' | 'date' | 'none'

export interface PreviewScreenshot {
    id: string
    instance_id: string
    instance_name: string
    path: string
    taken_at: string
    width: number
    height: number
    group: string | null
}

const HOUR = 3600 * 1000
const DAY = 24 * HOUR

/** When the user took each instance's captures and the group they filed them in. */
const SESSIONS: Record<string, { name: string; takenAgo: number; group: string | null }> = {
    'fabulously-optimized': { name: 'Fabulously Optimized', takenAgo: 1 * HOUR, group: null },
    cobblemon: {
        name: 'Cobblemon Official Modpack [Fabric]',
        takenAgo: 27 * HOUR,
        group: '宝可梦',
    },
    'fabric-26-3': { name: 'Fabric 26.3', takenAgo: 2 * DAY + 2 * HOUR, group: '风景' },
}

/**
 * Screenshots of the preview user: real in-game captures from the galleries of
 * the pack or mods each instance runs (`data/screenshots.json`), at their real
 * sizes. Captures of one instance are a day apart, newest first.
 */
function defaultScreenshots(): PreviewScreenshot[] {
    const counts: Record<string, number> = {}
    return screenshotSnapshot.screenshots.map((entry, index) => {
        const session = SESSIONS[entry.instance_id]
        const order = counts[entry.instance_id] ?? 0
        counts[entry.instance_id] = order + 1
        return {
            id: `ss-${index + 1}`,
            instance_id: entry.instance_id,
            instance_name: session.name,
            path: entry.path,
            taken_at: new Date(Date.now() - session.takenAgo - order * DAY).toISOString(),
            width: entry.width,
            height: entry.height,
            group: session.group,
        }
    })
}

export function createScreenshotsState(_clock: PreviewClock) {
    const screenshots = ref<PreviewScreenshot[]>(defaultScreenshots())
    const groups = ref<string[]>(['宝可梦', '风景'])
    const groupMode = ref<ScreenshotGroupMode>('custom')
    const query = ref('')

    function createGroup(name: string) {
        if (!groups.value.includes(name)) groups.value = [...groups.value, name]
    }

    function renameGroup(from: string, to: string) {
        groups.value = groups.value.map((entry) => (entry === from ? to : entry))
        screenshots.value = screenshots.value.map((entry) =>
            entry.group === from ? { ...entry, group: to } : entry,
        )
    }

    function deleteGroup(name: string) {
        groups.value = groups.value.filter((entry) => entry !== name)
        screenshots.value = screenshots.value.map((entry) =>
            entry.group === name ? { ...entry, group: null } : entry,
        )
    }

    function moveToGroup(ids: string[], group: string | null) {
        screenshots.value = screenshots.value.map((entry) =>
            ids.includes(entry.id) ? { ...entry, group } : entry,
        )
    }

    function remove(ids: string[]) {
        screenshots.value = screenshots.value.filter((entry) => !ids.includes(entry.id))
    }

    const visible = computed(() => {
        const needle = query.value.trim().toLowerCase()
        if (!needle) return screenshots.value
        return screenshots.value.filter(
            (entry) =>
                entry.instance_name.toLowerCase().includes(needle) ||
                (entry.group ?? '').toLowerCase().includes(needle),
        )
    })

    return {
        screenshots,
        groups,
        groupMode,
        query,
        visible,
        createGroup,
        renameGroup,
        deleteGroup,
        moveToGroup,
        remove,
    }
}

export type ScreenshotsState = ReturnType<typeof createScreenshotsState>
