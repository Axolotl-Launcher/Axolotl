<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, onBeforeUnmount, onMounted, onScopeDispose, ref, watch } from 'vue'

import { usePreview } from '../../../../lib/state'
import { messages } from '../../messages'
import MarkdownBody from './MarkdownBody.vue'

/**
 * Port of apps/app-frontend/src/components/ui/ResourceReleaseChangelog.vue for a Modrinth
 * release of the snapshot. The changelog is read from the snapshot's version; lazy rows wait
 * until they scroll into view. Translating needs the app's translation service, which the
 * window cannot reach, so a request ends in the component's own failure line.
 */
const props = withDefaults(
    defineProps<{ releaseId: string; changelog: string | null | undefined; lazy?: boolean }>(),
    { lazy: false },
)

const { formatMessage } = useVIntl()
const preview = usePreview()
const container = ref<HTMLElement | null>(null)
const activated = ref(!props.lazy)
const text = ref<string | null>(null)
const loading = ref(false)
const translating = ref(false)
const translationFailed = ref(false)
const active = ref(false)
const empty = computed(() => !text.value?.trim())
let cancelLoad: (() => void) | null = null
let cancelTranslation: (() => void) | null = null
let observer: IntersectionObserver | null = null

function resetTranslation() {
    cancelTranslation?.()
    cancelTranslation = null
    translating.value = false
    translationFailed.value = false
    active.value = false
}

function translateBody() {
    if (text.value === null || empty.value || translating.value) return
    resetTranslation()
    translating.value = true
    cancelTranslation = preview.clock.after(1200, () => {
        cancelTranslation = null
        translating.value = false
        translationFailed.value = true
    })
}

function load() {
    cancelLoad?.()
    resetTranslation()
    loading.value = true
    text.value = null
    cancelLoad = preview.clock.after(160, () => {
        cancelLoad = null
        text.value = props.changelog ?? ''
        loading.value = false
    })
}

watch(
    () => [props.releaseId, activated.value] as const,
    () => {
        if (activated.value) load()
    },
    { immediate: true },
)

onMounted(() => {
    if (activated.value || !container.value) return
    observer = new IntersectionObserver((entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
            activated.value = true
            observer?.disconnect()
            observer = null
        }
    })
    observer.observe(container.value)
})

onBeforeUnmount(() => observer?.disconnect())
onScopeDispose(() => {
    cancelLoad?.()
    resetTranslation()
})
</script>

<template>
    <div ref="container" class="flex min-w-0 flex-col gap-3" :aria-busy="loading">
        <div v-if="!activated" class="h-12 rounded bg-surface-3" />
        <p v-if="loading" class="m-0">{{ formatMessage(commonMessages.loadingLabel) }}</p>
        <p
            v-else-if="activated && text !== null && empty"
            class="m-0 text-[var(--color-text-tertiary)]"
        >
            {{ formatMessage(messages.changelogEmpty) }}
        </p>
        <template v-else-if="text">
            <div class="flex flex-wrap items-center gap-2">
                <Button
                    size="sm"
                    @click="translating || active ? resetTranslation() : translateBody()"
                >
                    {{
                        formatMessage(
                            translating
                                ? commonMessages.cancelButton
                                : active
                                  ? messages.changelogOriginal
                                  : messages.changelogTranslate,
                        )
                    }}
                </Button>
                <span v-if="translationFailed">{{
                    formatMessage(messages.changelogTranslationFailed)
                }}</span>
            </div>
            <MarkdownBody :source="text" />
        </template>
    </div>
</template>
