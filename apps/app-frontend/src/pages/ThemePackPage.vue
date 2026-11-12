<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import { dispatchThemePackRequest, parseThemePackRequest } from '@/helpers/theme-pack-bridge'
import { getThemePack, resolveThemePackPage, type ThemePackManifest } from '@/helpers/theme-packs'

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
const frame = ref<HTMLIFrameElement | null>(null)
const url = ref<string | null>(null)
const error = ref<string | null>(null)
const manifest = ref<ThemePackManifest | null>(null)
let disposed = false

function handleMessage(event: MessageEvent) {
    const target = frame.value?.contentWindow
    if (!target || event.source !== target) return
    const request = parseThemePackRequest(event.data)
    if (!request || !manifest.value) return
    void dispatchThemePackRequest(String(route.params.packId), manifest.value, request).then(
        (response) => {
            target.postMessage(response, '*')
        },
    )
}

async function load() {
    const packId = String(route.params.packId ?? '')
    const pageId = String(route.params.pageId ?? '')
    error.value = null
    url.value = null
    manifest.value = null
    if (!packId || !pageId) {
        error.value = formatMessage(messages.missing)
        return
    }
    try {
        const [pack, page] = await Promise.all([
            getThemePack(packId),
            resolveThemePackPage(packId, pageId),
        ])
        if (disposed) return
        if (!pack) {
            error.value = formatMessage(messages.missing)
            return
        }
        manifest.value = pack.manifest
        url.value = page.url
    } catch (cause) {
        if (disposed) return
        error.value = cause instanceof Error ? cause.message : formatMessage(messages.error)
    }
}

onMounted(() => {
    window.addEventListener('message', handleMessage)
    void load()
})
onBeforeUnmount(() => {
    disposed = true
    window.removeEventListener('message', handleMessage)
})
watch(
    () => [route.params.packId, route.params.pageId],
    () => {
        void load()
    },
)
</script>

<template>
    <div class="flex h-full w-full items-center justify-center">
        <div
            v-if="error"
            class="max-w-lg rounded-xl border border-solid border-divider bg-bg-raised p-6 text-center"
        >
            <p class="m-0 text-lg font-bold text-contrast">
                {{ formatMessage(messages.unavailable) }}
            </p>
            <p class="mb-0 mt-2 text-sm text-secondary">{{ error }}</p>
        </div>
        <iframe
            v-else-if="url"
            ref="frame"
            :src="url"
            sandbox="allow-scripts"
            referrerpolicy="no-referrer"
            class="h-full w-full border-0"
            :title="formatMessage(messages.title)"
        />
        <p v-else class="text-secondary">{{ formatMessage(messages.loading) }}</p>
    </div>
</template>
