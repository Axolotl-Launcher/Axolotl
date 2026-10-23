<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { onScopeDispose, ref, shallowRef, watch } from 'vue'

import type { ProjectDetail, ProjectVersion } from '../../../../data'
import { usePreview } from '../../../../lib/state'
import { messages } from '../../messages'
import ReleaseChangelog from './ReleaseChangelog.vue'

/**
 * The Changelog tab (apps/app-frontend/src/pages/project/Changelog.vue): the project's
 * releases, newest first, each with its lazily loaded changelog. The snapshot keeps the
 * latest releases of a project; when it published more than that, the list ends with the
 * page's pagination-limit notice instead of a "Load more releases" button.
 */
const props = defineProps<{ project: ProjectDetail; versions: ProjectVersion[] }>()

const PAGE_SIZE = 20
const { formatMessage } = useVIntl()
const formatDate = useFormatDateTime()
const preview = usePreview()
const releases = shallowRef<ProjectVersion[]>([])
const hasMore = ref(false)
const exhausted = ref(false)
const loading = ref(false)
let cancelLoad: (() => void) | null = null

function load(reset = false) {
    if (loading.value && !reset) return
    cancelLoad?.()
    loading.value = true
    if (reset) {
        releases.value = []
        hasMore.value = false
        exhausted.value = false
    }
    cancelLoad = preview.clock.after(reset ? 240 : 320, () => {
        cancelLoad = null
        const next = props.versions.slice(0, releases.value.length + PAGE_SIZE)
        releases.value = next
        hasMore.value = next.length < props.versions.length
        exhausted.value =
            !hasMore.value && (props.project.versions_total ?? next.length) > next.length
        loading.value = false
    })
}

watch(
    () => props.project.id,
    () => load(true),
    { immediate: true },
)
onScopeDispose(() => cancelLoad?.())
</script>

<template>
    <div class="flex min-w-0 flex-col gap-3">
        <h2 class="m-0 text-lg font-semibold">{{ formatMessage(messages.changelogTitle) }}</h2>
        <div
            class="relative mb-[var(--gap-md)] rounded-[var(--radius-lg)] border border-solid border-surface-4 bg-surface-3 !p-0"
        >
            <article
                v-for="release in releases"
                :key="release.id"
                class="border-b border-surface-4 p-5 last:border-b-0"
            >
                <div class="flex min-w-0 items-start gap-3">
                    <span class="mt-2 h-3 w-3 shrink-0 rounded-full bg-brand" />
                    <span class="flex min-w-0 flex-wrap items-baseline gap-x-3 gap-y-1">
                        <span class="min-w-0 break-words text-lg font-semibold">{{
                            release.version_number
                        }}</span>
                        <span class="text-sm font-normal text-[var(--color-text-tertiary)]"
                            >Modrinth<template v-if="release.date_published">
                                · {{ formatDate(new Date(release.date_published)) }}</template
                            ></span
                        >
                    </span>
                </div>
                <ReleaseChangelog
                    :key="release.id"
                    lazy
                    class="ml-1 mt-3 border-l-2 border-surface-4 pl-6"
                    :release-id="release.id"
                    :changelog="release.changelog"
                />
            </article>
        </div>
        <p v-if="loading" class="m-0">{{ formatMessage(commonMessages.loadingLabel) }}</p>
        <p v-else-if="!releases.length" class="m-0">
            {{ formatMessage(messages.changelogNoReleases) }}
        </p>
        <Button v-else-if="hasMore" class="self-start" @click="load()">{{
            formatMessage(messages.changelogMore)
        }}</Button>
        <p v-if="exhausted" class="m-0">{{ formatMessage(messages.changelogExhausted) }}</p>
    </div>
</template>
