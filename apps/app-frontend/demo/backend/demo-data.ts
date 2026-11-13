import type { AppSettings } from '@/helpers/settings'
import type { ColorTheme } from '@/store/theme'

import { type DemoAccount, useAccount } from './auth'
import { showNews } from './home'
import { showInstances } from './instances'
import { showAnnouncements } from './network'
import { pinServers } from './servers'
import { demoTheme, useDemoSettings } from './settings'

/**
 * What a site can change in the demo without rebuilding it: `demo-data.json` next to
 * the demo's `index.html`, read once before the launcher starts. Every field is
 * optional; a field that is missing or invalid, or a missing file, keeps the recording.
 */
export interface DemoData {
    /** A recorded Microsoft profile, or an offline name of 1–16 letters, digits or `_`. */
    account?: DemoAccount
    /**
     * Recorded instances to show, in this order (ids from backend/instances.ts), each
     * with how many minutes ago it was last played.
     */
    instances?: { id: string; playedMinutesAgo?: number }[]
    /** Home's Minecraft news, newest first; `image` is resolved against this file's URL. */
    news?: { title: string; url: string; date: string; image?: string | null }[]
    /** Launcher announcements, as `src/helpers/remote-announcements.ts` describes them. */
    announcements?: object[]
    /** Recorded remote addresses or managed server IDs pinned to Home. */
    pinnedServers?: string[]
    /** The page the launcher opens on. */
    defaultPage?: AppSettings['default_page']
    /** Interface language, a folder name of `src/locales` such as `zh-CN` or `en-US`. */
    locale?: string
    /** Starting theme; a `?theme=` in the demo's URL wins over it. */
    theme?: ColorTheme
}

const record = (value: unknown): value is Record<string, unknown> =>
    typeof value === 'object' && value !== null && !Array.isArray(value)
const text = (value: unknown): value is string => typeof value === 'string' && value !== ''
const date = (value: unknown) => text(value) && Number.isFinite(Date.parse(value))

/** Each field's check, and what applies it once it passes. */
const fields: { [K in keyof DemoData]-?: (value: unknown, base: URL) => boolean } = {
    account: (value) => {
        if (!record(value) || !text(value.name) || !/^[\p{L}\p{N}_]{1,16}$/u.test(value.name))
            return false
        if (value.type !== undefined && value.type !== 'offline' && value.type !== 'microsoft')
            return false
        if (value.type === 'microsoft') {
            if (
                !text(value.uuid) ||
                !/^(?:[0-9a-f]{32}|[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12})$/i.test(value.uuid)
            )
                return false
            return useAccount({ name: value.name, type: value.type, uuid: value.uuid })
        }
        return useAccount({ name: value.name })
    },
    instances: (value) => {
        const valid = (entry: unknown) =>
            record(entry) &&
            text(entry.id) &&
            (entry.playedMinutesAgo === undefined ||
                (typeof entry.playedMinutesAgo === 'number' && entry.playedMinutesAgo >= 0))
        if (!Array.isArray(value) || !value.every(valid)) return false
        showInstances(value as NonNullable<DemoData['instances']>)
        return true
    },
    news: (value, base) => {
        const valid = (entry: unknown) =>
            record(entry) &&
            text(entry.title) &&
            text(entry.url) &&
            date(entry.date) &&
            (entry.image == null || text(entry.image))
        if (!Array.isArray(value) || !value.every(valid)) return false
        showNews(
            (value as NonNullable<DemoData['news']>).map((entry) => ({
                ...entry,
                image: entry.image ? new URL(entry.image, base).href : null,
            })),
        )
        return true
    },
    announcements: (value) => {
        if (!Array.isArray(value) || !value.every((entry) => record(entry) && text(entry.id)))
            return false
        showAnnouncements(value as Parameters<typeof showAnnouncements>[0])
        return true
    },
    pinnedServers: (value) => {
        if (!Array.isArray(value) || !value.every(text)) return false
        pinServers(value)
        return true
    },
    defaultPage: (value) => {
        if (value !== 'Home' && value !== 'Library' && value !== 'DiscoverContent') return false
        useDemoSettings({ defaultPage: value })
        return true
    },
    locale: (value) => {
        if (!text(value) || !/^[a-z]{2,3}(-[A-Za-z0-9]{2,4})?$/.test(value)) return false
        useDemoSettings({ locale: value })
        return true
    },
    theme: (value) => {
        const theme = demoTheme(value)
        if (!theme) return false
        useDemoSettings({ theme })
        return true
    },
}

/** Reads demo-data.json, if the site provides one, and applies what it sets. */
export async function loadDemoData() {
    const url = new URL('demo-data.json', window.location.href)
    let data: unknown
    try {
        const response = await fetch(url)
        if (!response.ok) return
        data = await response.json()
    } catch (error) {
        console.warn('[demo] demo-data.json could not be read; the recordings stay', error)
        return
    }
    if (!record(data)) {
        console.warn('[demo] demo-data.json must hold an object; the recordings stay')
        return
    }
    for (const [field, value] of Object.entries(data)) {
        const apply = fields[field as keyof DemoData]
        if (!apply) console.warn(`[demo] demo-data.json: unknown field ${field}`)
        else if (!apply(value, url))
            console.warn(`[demo] demo-data.json: invalid ${field}, ignored`)
    }
}
