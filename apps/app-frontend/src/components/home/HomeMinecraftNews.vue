<script setup lang="ts">
import { ExternalIcon, NewspaperIcon, UpdatedIcon } from '@modrinth/assets'
import {
    Button,
    commonMessages,
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
    errorTitle: { id: 'app.home.news.error-title', defaultMessage: "Couldn't load the news" },
    emptyTitle: { id: 'app.home.news.empty-title', defaultMessage: 'No news right now' },
    offlineTitle: {
        id: 'app.home.news.offline-title',
        defaultMessage: 'News shows up when you are online',
    },
    openSite: { id: 'app.home.news.open-site', defaultMessage: 'Open minecraft.net' },
})

const NEWS_SITE = 'https://www.minecraft.net/en-us/articles'

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
const loading = computed(() => !offline.value && newsQuery.isPending.value)
const featured = computed(() => newsItems.value[0] ?? null)
const rest = computed(() => newsItems.value.slice(1))

/**
 * The card stays when there is nothing to show, and says why: offline, a
 * failed request (with a retry), or simply no articles.
 */
const emptyState = computed(() => {
    if (loading.value || newsItems.value.length > 0) return null
    if (offline.value) return 'offline'
    if (newsQuery.isError.value) return 'error'
    return 'empty'
})
const emptyText = computed(() => {
    switch (emptyState.value) {
        case 'offline':
            return formatMessage(messages.offlineTitle)
        case 'error':
            return formatMessage(messages.errorTitle)
        case 'empty':
            return formatMessage(messages.emptyTitle)
        default:
            return null
    }
})

async function openSite() {
    try {
        await openUrl(NEWS_SITE)
    } catch (error) {
        handleError(error)
    }
}

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
        class="home-news"
        variant="sidebar"
        :title="formatMessage(messages.news)"
        :icon="NewspaperIcon"
        :loading="loading"
        :skeleton-rows="NEWS_SKELETON_COUNT"
        :empty-text="emptyText"
        :empty-icon="NewspaperIcon"
        :empty-tone="emptyState === 'error' ? 'warning' : 'neutral'"
    >
        <template v-if="emptyState === 'error' || emptyState === 'empty'" #empty-action>
            <Button
                v-if="emptyState === 'error'"
                size="sm"
                type="quiet"
                color="brand"
                @click="newsQuery.refetch()"
            >
                <UpdatedIcon aria-hidden="true" />
                {{ formatMessage(commonMessages.retryButton) }}
            </Button>
            <Button size="sm" type="quiet" @click="openSite">
                <ExternalIcon aria-hidden="true" />
                {{ formatMessage(messages.openSite) }}
            </Button>
        </template>
        <div class="flex min-w-0 flex-col gap-1">
            <button
                v-if="featured"
                v-tooltip="formatMessage(messages.openArticle)"
                type="button"
                class="home-news-featured group"
                :class="{ 'has-cover': featured.image_url }"
                @click="openArticle(featured)"
            >
                <span v-if="featured.image_url" class="home-news-cover" aria-hidden="true">
                    <img :src="featured.image_url" alt="" loading="lazy" />
                </span>
                <span class="home-news-featured-copy">
                    <span class="home-news-featured-title">{{ featured.title }}</span>
                    <span v-if="newsDateLabel(featured)" class="home-news-featured-date">
                        {{ newsDateLabel(featured) }}
                        <ExternalIcon
                            class="size-3 opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
                            aria-hidden="true"
                        />
                    </span>
                </span>
            </button>
            <ul class="home-news-list">
                <li v-for="item in rest" :key="`${item.date ?? ''}:${item.title}`" class="min-w-0">
                    <button
                        v-tooltip="formatMessage(messages.openArticle)"
                        type="button"
                        class="home-news-row home-row group"
                        @click="openArticle(item)"
                    >
                        <img
                            v-if="item.image_url"
                            :src="item.image_url"
                            alt=""
                            class="home-news-thumb"
                            loading="lazy"
                        />
                        <span v-else class="home-news-thumb" />
                        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span class="home-news-row-title">{{ item.title }}</span>
                            <span
                                v-if="newsDateLabel(item)"
                                class="truncate text-[0.6875rem] font-semibold text-[var(--color-text-tertiary)]"
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
/*
 * The lead story is a cover: the widget's label sits above it, its picture
 * runs from edge to edge of the card, and the headline and date sit under the
 * picture on the card's own fill. No text is laid over the picture, so the
 * artwork's own lettering never fights the headline, in either theme. The rest
 * are compact rows.
 */
.home-news-featured {
    @apply relative flex w-full min-w-0 cursor-pointer flex-col gap-1 border-0 bg-transparent p-1.5 text-left;
    border-radius: var(--radius-md);
}

.home-news-featured:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 3px var(--color-brand-shadow);
}

.home-news-featured.has-cover {
    @apply -mx-4 mb-1 w-auto gap-2.5 p-0;
    border-radius: 0;
}

.home-news-cover {
    @apply relative block aspect-video w-full overflow-hidden;
    background: var(--home-widget-tint);
}

.home-news-cover img {
    @apply block size-full object-cover;
    transition: transform 240ms ease;
}

.home-news-featured:hover .home-news-cover img {
    transform: scale(1.03);
}

.home-news-featured-copy {
    @apply flex min-w-0 flex-col gap-1;
}

.home-news-featured.has-cover .home-news-featured-copy {
    @apply px-4;
}

.home-news-featured-title {
    @apply line-clamp-3 text-base font-extrabold leading-snug text-[var(--color-text-primary)];
}

.home-news-featured.has-cover .home-news-featured-title {
    @apply line-clamp-2;
}

.home-news-featured-date {
    @apply flex items-center gap-1 text-xs font-semibold text-[var(--color-text-tertiary)];
}

.home-news-list {
    @apply m-0 flex list-none flex-col p-0;
}

.home-news-row {
    @apply flex w-full cursor-pointer items-center gap-3 border-0 bg-transparent px-1.5 py-1.5 text-left;
}

.home-news-thumb {
    @apply block h-10 w-16 shrink-0 object-cover;
    border-radius: var(--radius-sm);
    background: var(--home-widget-tint);
}

.home-news-row-title {
    @apply line-clamp-2 text-[0.8125rem] font-semibold leading-snug text-[var(--color-text-primary)];
}

@media (prefers-reduced-motion: reduce) {
    .home-news-cover img,
    .home-news-featured:hover .home-news-cover img {
        transform: none;
        transition: none;
    }
}
</style>
