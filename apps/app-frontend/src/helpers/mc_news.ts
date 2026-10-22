import { invoke } from '@tauri-apps/api/core'

export type MinecraftNewsItem = {
    title: string
    category?: string | null
    tag?: string | null
    date?: string | null
    image_url?: string | null
    read_more_url: string
}

export function get_minecraft_news(limit = 12): Promise<MinecraftNewsItem[]> {
    return invoke<MinecraftNewsItem[]>('plugin:utils|get_minecraft_news', { limit })
}
