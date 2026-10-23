<script setup lang="ts">
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import NewspaperIcon from '@modrinth/assets/icons/newspaper.svg?component'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed } from 'vue'

import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import type { HomeNewsItem } from '../state'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const runtime = useHomeRuntime()
const formatDate = useFormatDateTime({ dateStyle: 'medium' })
const messages = homeMessages

const NEWS_SKELETON_COUNT = 4

void runtime.loadNews()

const htmlDecoder = document.createElement('textarea')
const newsItems = computed(() =>
    (runtime.news.value ?? []).map((item) => ({
        ...item,
        title: decodeHtmlEntities(item.title),
    })),
)
const loading = computed(() => runtime.news.value === null)

const visible = computed(() => loading.value || newsItems.value.length > 0)

function newsDateLabel(item: HomeNewsItem): string | null {
    if (!item.date) return null
    const parsed = new Date(item.date)
    return Number.isNaN(parsed.getTime()) ? null : formatDate(parsed)
}

function decodeHtmlEntities(value: string): string {
    htmlDecoder.innerHTML = value
    return htmlDecoder.value
}

/** `openUrl`: the desktop app hands the article to the system browser. */
function openArticle(item: HomeNewsItem) {
    try {
        window.open(item.read_more_url, '_blank', 'noopener,noreferrer')
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
                {{ formatMessage(messages.newsTitle) }}
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
                    v-tooltip="formatMessage(messages.newsOpenArticle)"
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
                        decoding="async"
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
