<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import {
    closeThemePackPage,
    getThemePack,
    openThemePackPage,
    resolveThemePackPage,
    setThemePackPageBounds,
} from '@/helpers/theme-packs'

const messages = defineMessages({
    loading: { id: 'app.theme-suite.page.loading', defaultMessage: 'Loading theme page…' },
    unavailable: {
        id: 'app.theme-suite.page.unavailable',
        defaultMessage: 'Theme page unavailable',
    },
    missing: {
        id: 'app.theme-suite.page.missing',
        defaultMessage: 'The theme pack or page is not installed.',
    },
    error: {
        id: 'app.theme-suite.page.error',
        defaultMessage: 'The theme page could not be loaded.',
    },
    title: {
        id: 'app.theme-suite.page.title',
        defaultMessage: 'Theme pack page',
    },
})

const { formatMessage } = useVIntl()
const route = useRoute()
const host = ref<HTMLElement | null>(null)
const error = ref<string | null>(null)
const activePackId = ref<string | null>(null)
let pageLabel: string | null = null
let disposed = false
let loadToken = 0
let resizeObserver: ResizeObserver | null = null
let boundsFrame = 0
let opening = false
let pendingOpen = false

function pageBounds() {
    const rect = host.value?.getBoundingClientRect()
    if (!rect) return null
    return {
        x: Math.round(rect.left),
        y: Math.round(rect.top),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
    }
}

function syncBounds() {
    if (!pageLabel) return
    const bounds = pageBounds()
    if (!bounds || bounds.width <= 0 || bounds.height <= 0) return
    void setThemePackPageBounds(pageLabel, bounds).catch(() => undefined)
}

function scheduleBoundsSync() {
    if (boundsFrame) return
    boundsFrame = requestAnimationFrame(() => {
        boundsFrame = 0
        if (pageLabel) {
            syncBounds()
        } else if (activePackId.value && !error.value && !opening) {
            void openPage()
        }
    })
}

async function closePage() {
    const label = pageLabel
    pageLabel = null
    if (label) {
        await closeThemePackPage(label).catch(() => undefined)
    }
}

async function openPage() {
    const token = loadToken
    const packId = activePackId.value
    const pageId = String(route.params.pageId ?? '')
    if (!packId || !pageId || disposed) return
    const bounds = pageBounds()
    if (!bounds || bounds.width <= 0 || bounds.height <= 0) return
    if (opening) {
        pendingOpen = true
        return
    }
    opening = true
    try {
        await closePage()
        if (disposed || token !== loadToken) return
        const label = await openThemePackPage(packId, pageId, bounds)
        if (disposed || token !== loadToken) {
            await closeThemePackPage(label).catch(() => undefined)
            return
        }
        pageLabel = label
    } catch (cause) {
        if (disposed || token !== loadToken) return
        error.value = cause instanceof Error ? cause.message : formatMessage(messages.error)
    } finally {
        opening = false
        if (pendingOpen) {
            pendingOpen = false
            void openPage()
        }
    }
}

async function load() {
    const token = ++loadToken
    const packId = String(route.params.packId ?? '')
    const pageId = String(route.params.pageId ?? '')
    error.value = null
    activePackId.value = null
    await closePage()
    if (!packId || !pageId) {
        error.value = formatMessage(messages.missing)
        return
    }
    try {
        const [pack] = await Promise.all([
            getThemePack(packId),
            resolveThemePackPage(packId, pageId),
        ])
        if (disposed || token !== loadToken) return
        if (!pack) {
            error.value = formatMessage(messages.missing)
            return
        }
        activePackId.value = packId
        await nextTick()
        await openPage()
    } catch (cause) {
        if (disposed || token !== loadToken) return
        error.value = cause instanceof Error ? cause.message : formatMessage(messages.error)
    }
}

onMounted(() => {
    if (host.value) {
        resizeObserver = new ResizeObserver(scheduleBoundsSync)
        resizeObserver.observe(host.value)
    }
    window.addEventListener('resize', scheduleBoundsSync)
    window.addEventListener('scroll', scheduleBoundsSync, true)
    void load()
})
onBeforeUnmount(() => {
    disposed = true
    resizeObserver?.disconnect()
    window.removeEventListener('resize', scheduleBoundsSync)
    window.removeEventListener('scroll', scheduleBoundsSync, true)
    if (boundsFrame) cancelAnimationFrame(boundsFrame)
    void closePage()
})
watch(
    () => [route.params.packId, route.params.pageId],
    () => {
        void load()
    },
)
</script>

<template>
    <div class="relative h-full w-full">
        <div v-if="error" class="flex h-full w-full items-center justify-center">
            <div
                class="max-w-lg rounded-xl border border-solid border-divider bg-bg-raised p-6 text-center"
            >
                <p class="m-0 text-lg font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.unavailable) }}
                </p>
                <p class="mb-0 mt-2 text-sm text-[var(--color-text-tertiary)]">{{ error }}</p>
            </div>
        </div>
        <template v-else>
            <div ref="host" class="h-full w-full" />
            <p
                v-if="!activePackId"
                class="pointer-events-none absolute inset-0 m-0 flex items-center justify-center text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.loading) }}
            </p>
        </template>
    </div>
</template>
