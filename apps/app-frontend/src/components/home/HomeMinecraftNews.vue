<script setup lang="ts">
import { ExternalIcon, NewspaperIcon } from '@modrinth/assets'
import {
    defineMessages,
    injectNotificationManager,
    useFormatDateTime,
    useVIntl,
} from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed } from 'vue'

import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { get_minecraft_news, type MinecraftNewsItem } from '@/helpers/mc_news'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { offline } = useNetworkStatus()
const formatDate = useFormatDateTime({ dateStyle: 'medium' })

const messages = defineMessages({
    news: { id: 'app.home.news.title', defaultMessage: 'Minecraft news' },
    openArticle: { id: 'app.home.news.open-article', defaultMessage: 'Read on minecraft.net' },
})

const NEWS_COUNT = 12
const NEWS_SKELETON_COUNT = 4

const htmlDecoder = document.createElement('textarea')
const newsQuery = useQuery({
    queryKey: ['home', 'minecraft-news', NEWS_COUNT],
    queryFn: () => get_minecraft_news(NEWS_COUNT),
    enabled: computed(() => !offline.value),
    staleTime: 30 * 60 * 1000,
    gcTime: 30 * 60 * 1000,
    retry: false,
    // The unified network state also covers native reachability checks.
    // Re-enabling this query owns reconnect fetches, avoiding a second scheduler.
    networkMode: 'always',
    refetchOnReconnect: false,
    refetchOnWindowFocus: false,
})
const newsItems = computed(() =>
    (newsQuery.data.value ?? []).map((item) => ({
        ...item,
        title: decodeHtmlEntities(item.title),
    })),
)
const loading = computed(() => newsQuery.isPending.value)

const visible = computed(() => !offline.value && (loading.value || newsItems.value.length > 0))

function newsDateLabel(item: MinecraftNewsItem): string | null {
    if (!item.date) return null
    const parsed = new Date(item.date)
    return Number.isNaN(parsed.getTime()) ? null : formatDate(parsed)
}

function decodeHtmlEntities(value: string): string {
    htmlDecoder.innerHTML = value
    return htmlDecoder.value
}

async function openArticle(item: MinecraftNewsItem) {
    try {
        await openUrl(item.read_more_url)
    } catch (error) {
        handleError(error)
    }
}
</script>

<template>
    <section
        v-if="visible"
        class="flex min-w-0 flex-col gap-3 border-0 border-b-[1px] border-solid border-[--brand-gradient-border] p-4"
    >
        <div class="flex items-center gap-2">
            <NewspaperIcon
                class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                aria-hidden="true"
            />
            <h2 class="m-0 truncate text-lg">
                {{ formatMessage(messages.news) }}
            </h2>
        </div>
        <ul v-if="loading" class="m-0 flex list-none flex-col gap-1.5 p-0" aria-hidden="true">
            <li
                v-for="index in NEWS_SKELETON_COUNT"
                :key="index"
                class="flex animate-pulse items-center gap-2.5 px-1.5 py-1.5"
            >
                <div class="h-9 w-16 shrink-0 rounded-lg bg-surface-4" />
                <div class="flex min-w-0 flex-1 flex-col gap-1.5">
                    <div class="h-3 w-full rounded bg-surface-4" />
                    <div class="h-3 w-1/2 rounded bg-surface-4" />
                </div>
            </li>
        </ul>
        <ul v-else class="m-0 flex list-none flex-col p-0">
            <li
                v-for="item in newsItems"
                :key="`${item.date ?? ''}:${item.title}`"
                class="group min-w-0"
            >
                <button
                    v-tooltip="formatMessage(messages.openArticle)"
                    type="button"
                    class="flex w-full cursor-pointer items-center gap-2.5 rounded-lg border-0 bg-transparent px-1.5 py-1.5 text-left transition-colors hover:bg-surface-4"
                    @click="openArticle(item)"
                >
                    <img
                        v-if="item.image_url"
                        :src="item.image_url"
                        alt=""
                        class="h-9 w-16 shrink-0 rounded-lg object-cover"
                        loading="lazy"
                    />
                    <div v-else class="h-9 w-16 shrink-0 rounded-lg bg-surface-4" />
                    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                        <span
                            class="line-clamp-2 text-sm font-semibold leading-snug text-[var(--color-text-primary)]"
                        >
                            {{ item.title }}
                        </span>
                        <span
                            v-if="newsDateLabel(item)"
                            class="truncate text-xs text-[var(--color-text-tertiary)]"
                        >
                            {{ newsDateLabel(item) }}
                        </span>
                    </div>
                    <ExternalIcon
                        class="size-3.5 shrink-0 text-[var(--color-text-tertiary)] opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
                        aria-hidden="true"
                    />
                </button>
            </li>
        </ul>
    </section>
</template>
