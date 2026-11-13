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

import HomeWidgetFrame from './HomeWidgetFrame.vue'

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
const featured = computed(() => newsItems.value[0] ?? null)
const rest = computed(() => newsItems.value.slice(1))

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
    <HomeWidgetFrame
        v-if="visible"
        class="home-sidebar-section home-surface"
        :title="formatMessage(messages.news)"
        :icon="NewspaperIcon"
        :loading="loading"
        :skeleton-rows="NEWS_SKELETON_COUNT"
    >
        <div class="flex min-w-0 flex-col gap-1">
            <button
                v-if="featured"
                v-tooltip="formatMessage(messages.openArticle)"
                type="button"
                class="home-news-featured home-row group"
                @click="openArticle(featured)"
            >
                <span class="home-news-cover">
                    <img
                        v-if="featured.image_url"
                        :src="featured.image_url"
                        alt=""
                        loading="lazy"
                    />
                </span>
                <span
                    class="line-clamp-2 text-sm font-bold leading-snug text-[var(--color-text-primary)]"
                >
                    {{ featured.title }}
                </span>
                <span
                    v-if="newsDateLabel(featured)"
                    class="flex items-center gap-1 text-xs text-[var(--color-text-tertiary)]"
                >
                    {{ newsDateLabel(featured) }}
                    <ExternalIcon
                        class="size-3 opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
                        aria-hidden="true"
                    />
                </span>
            </button>
            <ul class="m-0 flex list-none flex-col p-0">
                <li v-for="item in rest" :key="`${item.date ?? ''}:${item.title}`" class="min-w-0">
                    <button
                        v-tooltip="formatMessage(messages.openArticle)"
                        type="button"
                        class="home-row group flex w-full cursor-pointer items-center gap-2.5 border-0 bg-transparent px-1.5 py-1.5 text-left"
                        @click="openArticle(item)"
                    >
                        <img
                            v-if="item.image_url"
                            :src="item.image_url"
                            alt=""
                            class="h-9 w-14 shrink-0 rounded-[var(--radius-sm)] object-cover"
                            loading="lazy"
                        />
                        <span
                            v-else
                            class="h-9 w-14 shrink-0 rounded-[var(--radius-sm)] bg-[var(--home-widget-tint)]"
                        />
                        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span
                                class="line-clamp-2 text-[0.8125rem] font-semibold leading-snug text-[var(--color-text-primary)]"
                            >
                                {{ item.title }}
                            </span>
                            <span
                                v-if="newsDateLabel(item)"
                                class="truncate text-xs text-[var(--color-text-tertiary)]"
                            >
                                {{ newsDateLabel(item) }}
                            </span>
                        </span>
                    </button>
                </li>
            </ul>
        </div>
    </HomeWidgetFrame>
</template>

<style scoped>
.home-news-featured {
    @apply flex w-full cursor-pointer flex-col gap-1.5 border-0 bg-transparent p-1.5 text-left;
}

.home-news-cover {
    @apply block aspect-[16/9] w-full overflow-hidden rounded-[var(--radius-md)];
    background: var(--home-widget-tint);
}

.home-news-cover img {
    @apply block size-full object-cover;
    transition: transform 240ms ease;
}

.home-news-featured:hover .home-news-cover img {
    transform: scale(1.03);
}

@media (prefers-reduced-motion: reduce) {
    .home-news-cover img,
    .home-news-featured:hover .home-news-cover img {
        transform: none;
        transition: none;
    }
}
</style>
