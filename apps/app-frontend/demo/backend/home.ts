import type { DailyPlaytime, DailyPlaytimeEntry } from '@/helpers/instance'

import news from '../data/news.json'
import { instancePlaySessions } from './instances'
import type { Handlers } from './types'

const newsImages = import.meta.glob<string>('../assets/news/*.webp', {
    eager: true,
    query: '?url',
    import: 'default',
})

/** The recorded news, with their images as the bundled files. */
let shownNews = news.map((item) => ({
    ...item,
    image_url: item.image_url ? newsImages[`../assets/news/${item.image_url}`] : null,
}))

/** News items from demo-data.json, newest first, in place of the recorded ones. */
export function showNews(
    items: { title: string; url: string; date: string; image: string | null }[],
) {
    shownNews = items.map((item) => ({
        title: item.title,
        category: null,
        tag: null,
        date: item.date,
        image_url: item.image,
        read_more_url: item.url,
    }))
}

function localDate(date: Date) {
    const pad = (value: number) => String(value).padStart(2, '0')
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

/** Per-day entries derived from the instances' recorded sessions. */
function entriesOn(date: string): DailyPlaytimeEntry[] {
    return instancePlaySessions()
        .filter((session) => localDate(session.at) === date)
        .map((session) => ({
            instance_id: session.instance.id,
            instance_name: session.instance.name,
            played_seconds: session.seconds,
            session_count: 1,
        }))
}

export const homeHandlers: Handlers = {
    'plugin:utils|get_minecraft_news': ({ limit }) => shownNews.slice(0, Number(limit ?? 12)),
    'plugin:instance|instance_get_daily_playtime': ({ startDate, endDate }) => {
        const days = new Map<string, DailyPlaytime>()
        for (const session of instancePlaySessions()) {
            const date = localDate(session.at)
            if (date < String(startDate) || date > String(endDate)) continue
            const day = days.get(date) ?? {
                date,
                played_seconds: 0,
                session_count: 0,
                top_instance_name: null,
            }
            day.played_seconds += session.seconds
            day.session_count += 1
            day.top_instance_name = entriesOn(date).sort(
                (a, b) => b.played_seconds - a.played_seconds,
            )[0]?.instance_name
            days.set(date, day)
        }
        return [...days.values()]
    },
    'plugin:instance|instance_get_daily_playtime_details': ({ date }) => entriesOn(String(date)),
    'plugin:instance|instance_list_groups': () => [],
}
