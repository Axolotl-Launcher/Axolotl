import type {
    InstanceScreenshot,
    ScreenshotGroup,
    ScreenshotGroupMembershipUpdate,
    ScreenshotKey,
    ScreenshotScanResult,
} from '@/helpers/instance'

import recorded from '../data/screenshots.json'
import { liveCaptureSupported, vibrate } from '../warden/egg'
import {
    ANCIENT_CITY_THUMBNAIL_URL,
    ANCIENT_CITY_URL,
    deleteButton,
    showInViewer,
    watchViewer,
} from '../warden/viewer'
import { byId } from './instances'
import { demoText, desktopOnly } from './settings'
import type { Handlers } from './types'

const images = import.meta.glob<string>('../assets/screenshots/*/*.webp', {
    eager: true,
    query: '?url',
    import: 'default',
})

const pad = (value: number) => String(value).padStart(2, '0')

/** The name Minecraft gives a screenshot taken at `date`. */
function fileName(date: Date) {
    return (
        `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}_` +
        `${pad(date.getHours())}.${pad(date.getMinutes())}.${pad(date.getSeconds())}.png`
    )
}

/** Each recorded image, as a screenshot taken during its instance's last session. */
const take = () =>
    recorded.flatMap((entry, index): InstanceScreenshot[] => {
        const instance = byId(entry.instance_id)
        if (!instance?.last_played) return []
        const taken = new Date(
            new Date(instance.last_played).getTime() - (index + 1) * 6 * 60 * 1000,
        )
        const name = fileName(taken)
        return [
            {
                id: `${instance.id}/${name}`,
                instance_id: instance.id,
                instance_name: instance.name,
                file_name: name,
                created_at: taken.toISOString(),
                modified_at: Math.floor(taken.getTime() / 1000),
                group_id: null,
                path: `${instance.path}/screenshots/${name}`,
                url: images[`../assets/screenshots/${entry.image}`],
            },
        ]
    })

// Taken on first use, after demo-data.json has chosen the instances and when they ran.
let listed: InstanceScreenshot[] | null = null
const screenshots = () => {
    if (!listed) {
        listed = take()
        // From then on, the ancient city capture comes alive in the image viewer.
        watchViewer()
    }
    return listed
}

let groups: ScreenshotGroup[] = []

const scan = (list: InstanceScreenshot[]): ScreenshotScanResult => ({
    screenshots: list,
    skipped_instances: [],
})

const matches = (keys: unknown) => (screenshot: InstanceScreenshot) =>
    (keys as ScreenshotKey[]).some(
        (key) =>
            key.instance_id === screenshot.instance_id && key.file_name === screenshot.file_name,
    )

const needsDesktop = () => {
    throw desktopOnly()
}

/**
 * A delete of the ancient city capture is heard in its world first (warden/egg.ts),
 * shown in the viewer: where it was asked from, or opened there from the grid.
 * True: the warden came and takes it; false: it stays; null: an ordinary delete,
 * as it is without the capture or without WebGL 2.
 */
async function hearDelete(keys: unknown) {
    const capture = screenshots().find(
        (screenshot) => screenshot.url === ANCIENT_CITY_URL && matches(keys)(screenshot),
    )
    if (!capture || !liveCaptureSupported()) return null
    if (!(await showInViewer(capture.id, capture.file_name))) return false
    return await vibrate(deleteButton)
}

export const screenshotHandlers: Handlers = {
    'plugin:instance|instance_list_screenshots': ({ instanceId }) =>
        screenshots().filter((screenshot) => screenshot.instance_id === instanceId),
    'plugin:instance|instance_list_all_screenshots': () => scan(screenshots()),
    // app-lib answers the Screenshots page's synced list with every instance's screenshots.
    'plugin:instance|instance_list_synced_screenshots': () => scan(screenshots()),
    'plugin:instance|instance_list_screenshot_groups': () => groups,
    'plugin:instance|instance_create_screenshot_group': ({ name, ids }) => {
        const group = { id: crypto.randomUUID(), name: String(name) }
        groups = [...groups, group]
        const members = new Set(ids as string[])
        listed = screenshots().map((screenshot) =>
            members.has(screenshot.id) ? { ...screenshot, group_id: group.id } : screenshot,
        )
        return group
    },
    'plugin:instance|instance_rename_screenshot_group': ({ id, newName }): ScreenshotGroup => {
        if (!groups.some((group) => group.id === id)) throw new Error(`Unknown group ${String(id)}`)
        const renamed = { id: String(id), name: String(newName) }
        groups = groups.map((group) => (group.id === id ? renamed : group))
        return renamed
    },
    'plugin:instance|instance_delete_screenshot_group': ({ id }) => {
        groups = groups.filter((group) => group.id !== id)
        listed = screenshots().map((screenshot) =>
            screenshot.group_id === id ? { ...screenshot, group_id: null } : screenshot,
        )
    },
    'plugin:instance|instance_set_screenshot_group_memberships': ({ updates }) => {
        const next = new Map(
            (updates as ScreenshotGroupMembershipUpdate[]).map((update) => [
                update.screenshot_id,
                update.group_id,
            ]),
        )
        listed = screenshots().map((screenshot) =>
            next.has(screenshot.id)
                ? { ...screenshot, group_id: next.get(screenshot.id)! }
                : screenshot,
        )
    },
    'plugin:instance|instance_delete_screenshots': async ({ keys }) => {
        if ((await hearDelete(keys)) === false)
            throw new Error(
                demoText(
                    '没能删除：有什么东西听见了。',
                    "Couldn't delete it: something heard you.",
                ),
            )
        listed = screenshots().filter((screenshot) => !matches(keys)(screenshot))
    },
    // The recorded images are already small, so a thumbnail is the image itself
    // (but for the ancient city capture, kept at full size).
    'plugin:files|screenshot_thumbnail': async ({ instanceId, filePath }) => {
        const screenshot = screenshots().find(
            (entry) =>
                entry.instance_id === instanceId &&
                `screenshots/${entry.file_name}` === String(filePath),
        )
        if (!screenshot) throw new Error(`No such screenshot: ${String(filePath)}`)
        const url =
            screenshot.url === ANCIENT_CITY_URL ? ANCIENT_CITY_THUMBNAIL_URL : screenshot.url
        return (await fetch(url)).arrayBuffer()
    },
    // The image viewer and Copy read a screenshot's own file; other files need the desktop app.
    'plugin:fs|read_file': async ({ path }) => {
        const screenshot = screenshots().find((entry) => entry.path === String(path))
        if (!screenshot) throw desktopOnly()
        return (await fetch(screenshot.url)).arrayBuffer()
    },
    'plugin:instance|instance_open_screenshot': needsDesktop,
    'plugin:instance|instance_export_screenshots': needsDesktop,
}
