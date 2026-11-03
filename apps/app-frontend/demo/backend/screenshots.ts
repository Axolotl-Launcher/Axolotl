import type {
    InstanceScreenshot,
    ScreenshotGroup,
    ScreenshotGroupMembershipUpdate,
    ScreenshotKey,
    ScreenshotScanResult,
} from '@/helpers/instance'

import recorded from '../data/screenshots.json'
import { byId } from './instances'
import { desktopOnly } from './settings'
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
let screenshots: InstanceScreenshot[] = recorded.flatMap((entry, index) => {
    const instance = byId(entry.instance_id)
    if (!instance?.last_played) return []
    const taken = new Date(new Date(instance.last_played).getTime() - (index + 1) * 6 * 60 * 1000)
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

export const screenshotHandlers: Handlers = {
    'plugin:instance|instance_list_screenshots': ({ instanceId }) =>
        screenshots.filter((screenshot) => screenshot.instance_id === instanceId),
    'plugin:instance|instance_list_all_screenshots': () => scan(screenshots),
    // app-lib answers the Screenshots page's synced list with every instance's screenshots.
    'plugin:instance|instance_list_synced_screenshots': () => scan(screenshots),
    'plugin:instance|instance_list_screenshot_groups': () => groups,
    'plugin:instance|instance_create_screenshot_group': ({ name, ids }) => {
        const group = { id: crypto.randomUUID(), name: String(name) }
        groups = [...groups, group]
        const members = new Set(ids as string[])
        screenshots = screenshots.map((screenshot) =>
            members.has(screenshot.id) ? { ...screenshot, group_id: group.id } : screenshot,
        )
        return group
    },
    'plugin:instance|instance_rename_screenshot_group': ({ id, newName }) => {
        groups = groups.map((group) =>
            group.id === id ? { ...group, name: String(newName) } : group,
        )
    },
    'plugin:instance|instance_delete_screenshot_group': ({ id }) => {
        groups = groups.filter((group) => group.id !== id)
        screenshots = screenshots.map((screenshot) =>
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
        screenshots = screenshots.map((screenshot) =>
            next.has(screenshot.id)
                ? { ...screenshot, group_id: next.get(screenshot.id)! }
                : screenshot,
        )
    },
    'plugin:instance|instance_delete_screenshots': ({ keys }) => {
        screenshots = screenshots.filter((screenshot) => !matches(keys)(screenshot))
    },
    // The recorded images are already small, so a thumbnail is the image itself.
    'plugin:files|screenshot_thumbnail': async ({ instanceId, filePath }) => {
        const screenshot = screenshots.find(
            (entry) =>
                entry.instance_id === instanceId &&
                `screenshots/${entry.file_name}` === String(filePath),
        )
        if (!screenshot) throw new Error(`No such screenshot: ${String(filePath)}`)
        return (await fetch(screenshot.url)).arrayBuffer()
    },
    'plugin:instance|instance_open_screenshot': needsDesktop,
    'plugin:instance|instance_export_screenshots': needsDesktop,
}
