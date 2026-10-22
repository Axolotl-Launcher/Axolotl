<script setup lang="ts">
import {
    Button,
    Card,
    commonMessages,
    defineMessages,
    useFormatDateTime,
    useVIntl,
} from '@modrinth/ui'
import { onScopeDispose, ref, watch } from 'vue'

import ResourceReleaseChangelog from '@/components/ui/ResourceReleaseChangelog.vue'
import {
    type ResourceRelease,
    resourceReleases,
    type SourceId,
    sourceResource,
} from '@/helpers/resources'

const props = withDefaults(defineProps<{ project: { id: string | number }; source?: SourceId }>(), {
    source: 'modrinth',
})
const { formatMessage } = useVIntl()
const formatDate = useFormatDateTime()
const releases = ref<ResourceRelease[]>([])
const cursor = ref<string | null>(null)
const hasMore = ref(false)
const exhausted = ref(false)
const loading = ref(false)
const failed = ref(false)
let controller: AbortController | null = null
const messages = defineMessages({
    title: { id: 'app.resources.changelog.title', defaultMessage: 'Changelog' },
    empty: {
        id: 'app.resources.changelog.no-releases',
        defaultMessage: 'No releases are available.',
    },
    failed: {
        id: 'app.resources.changelog.list-failed',
        defaultMessage: 'Releases could not be loaded.',
    },
    more: { id: 'app.resources.changelog.more', defaultMessage: 'Load more releases' },
    exhausted: {
        id: 'app.resources.changelog.exhausted',
        defaultMessage: 'This source’s release pagination limit has been reached.',
    },
})

async function load(reset = false) {
    if (loading.value && !reset) return
    controller?.abort()
    const request = new AbortController()
    controller = request
    loading.value = true
    failed.value = false
    if (reset) {
        releases.value = []
        cursor.value = null
        hasMore.value = false
        exhausted.value = false
    }
    try {
        const page = await resourceReleases(
            sourceResource(props.source, String(props.project.id)),
            undefined,
            cursor.value,
            undefined,
            request.signal,
        )
        request.signal.throwIfAborted()
        const known = new Set(releases.value.map((release) => release.reference.releaseId))
        releases.value.push(
            ...page.entries.filter((release) => !known.has(release.reference.releaseId)),
        )
        cursor.value = page.cursor
        hasMore.value = page.hasMore
        exhausted.value = page.exhaustedSources.length > 0
    } catch {
        if (!request.signal.aborted) failed.value = true
    } finally {
        if (controller === request) loading.value = false
    }
}
watch(
    () => [props.source, props.project.id],
    () => void load(true),
    { immediate: true },
)
onScopeDispose(() => controller?.abort())
</script>

<template>
    <div class="flex min-w-0 flex-col gap-3">
        <h2 class="m-0 text-lg font-semibold">{{ formatMessage(messages.title) }}</h2>
        <Card class="!p-0">
            <article
                v-for="release in releases"
                :key="release.reference.releaseId"
                class="border-b border-surface-4 p-5 last:border-b-0"
            >
                <div class="flex min-w-0 items-start gap-3">
                    <span class="mt-2 h-3 w-3 shrink-0 rounded-full bg-brand" />
                    <span class="flex min-w-0 flex-wrap items-baseline gap-x-3 gap-y-1">
                        <span class="min-w-0 break-words text-lg font-semibold">{{
                            release.version
                        }}</span>
                        <span class="text-sm font-normal text-[var(--color-text-tertiary)]"
                            >{{ source === 'curseforge' ? 'CurseForge' : 'Modrinth'
                            }}<template v-if="release.publishedAt">
                                · {{ formatDate(new Date(release.publishedAt)) }}</template
                            ></span
                        >
                    </span>
                </div>
                <ResourceReleaseChangelog
                    :key="release.reference.releaseId"
                    lazy
                    class="ml-1 mt-3 border-l-2 border-surface-4 pl-6"
                    :release="release.reference"
                />
            </article>
        </Card>
        <p v-if="loading" class="m-0">{{ formatMessage(commonMessages.loadingLabel) }}</p>
        <div v-else-if="failed" class="flex flex-wrap items-center gap-2">
            <span>{{ formatMessage(messages.failed) }}</span>
            <Button @click="load()">{{ formatMessage(commonMessages.retryButton) }}</Button>
        </div>
        <p v-else-if="!releases.length" class="m-0">{{ formatMessage(messages.empty) }}</p>
        <Button v-else-if="hasMore" class="self-start" @click="load()">{{
            formatMessage(messages.more)
        }}</Button>
        <p v-if="exhausted" class="m-0">{{ formatMessage(messages.exhausted) }}</p>
    </div>
</template>
