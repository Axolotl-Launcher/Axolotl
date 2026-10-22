<script setup lang="ts">
import { Button, commonMessages, defineMessages, useVIntl } from '@modrinth/ui'
import { useElementVisibility } from '@vueuse/core'
import { computed, onScopeDispose, ref, watch } from 'vue'

import { changelogTranslationRequest } from '@/helpers/resource-changelog'
import { resourceChangelog, type ResourceText, type SourceReleaseRef } from '@/helpers/resources'
import {
    getTranslationSettings,
    prepareDescription,
    translate,
    translateInBatches,
    type TranslationMode,
    type TranslationStyle,
    validateTranslatedDescription,
} from '@/helpers/translation'

import TranslatedProjectDescription from './TranslatedProjectDescription.vue'

const props = withDefaults(defineProps<{ release: SourceReleaseRef; lazy?: boolean }>(), {
    lazy: false,
})
const container = ref<HTMLElement | null>(null)
const visible = useElementVisibility(container)
const activated = ref(!props.lazy)
watch(visible, (value) => {
    if (value) activated.value = true
})
const { formatMessage, locale } = useVIntl()
const text = ref<ResourceText | null>(null)
const loading = ref(false)
const failed = ref(false)
const translating = ref(false)
const translationFailed = ref(false)
const active = ref(false)
const translations = ref<Record<string, string>>({})
const mode = ref<TranslationMode>('bilingual')
const style = ref<TranslationStyle>('weakened')
let controller: AbortController | null = null
let translationController: AbortController | null = null
const empty = computed(() => !text.value?.body.trim())
const supported = computed(() => ['modrinth', 'curseforge'].includes(props.release.project.source))
const messages = defineMessages({
    unsupported: {
        id: 'app.resources.changelog.unsupported',
        defaultMessage: 'This source does not provide release changelogs.',
    },
    empty: {
        id: 'app.resources.changelog.empty',
        defaultMessage: 'No changelog was provided for this release.',
    },
    failed: {
        id: 'app.resources.changelog.failed',
        defaultMessage: 'The changelog could not be loaded.',
    },
    translate: { id: 'app.resources.changelog.translate', defaultMessage: 'Translate changelog' },
    original: { id: 'app.resources.changelog.original', defaultMessage: 'Show original' },
    translationFailed: {
        id: 'app.resources.changelog.translation-failed',
        defaultMessage: 'Translation failed. The original changelog is still available.',
    },
})

function resetTranslation() {
    translationController?.abort()
    translationController = null
    translating.value = false
    translationFailed.value = false
    active.value = false
    translations.value = {}
}

async function translateBody(automatic = false) {
    if (!text.value || empty.value || translating.value) return
    resetTranslation()
    const request = new AbortController()
    translationController = request
    translating.value = true
    const original = text.value
    const reference = props.release
    try {
        const settings = await getTranslationSettings()
        request.signal.throwIfAborted()
        if (automatic && !settings.auto_translate) return
        const language = settings.target_language.trim() || locale.value
        const prepared =
            original.format === 'plain' ? null : prepareDescription(original.body, original.format)
        const segments = prepared?.segments ?? [
            { id: 'body', text: original.body, format: 'plain' as const },
        ]
        const result = await translateInBatches(
            changelogTranslationRequest(reference, original, language, segments),
            undefined,
            async (batch) => {
                request.signal.throwIfAborted()
                const response = await translate(batch)
                request.signal.throwIfAborted()
                return response
            },
        )
        request.signal.throwIfAborted()
        const translated = Object.fromEntries(
            result.segments.map((segment) => [segment.id, segment.text]),
        )
        if (prepared) validateTranslatedDescription(prepared, translated)
        mode.value = settings.mode
        style.value = settings.style
        translations.value = translated
        active.value = true
    } catch {
        if (!request.signal.aborted) translationFailed.value = true
    } finally {
        if (translationController === request) translating.value = false
    }
}

async function load() {
    controller?.abort()
    resetTranslation()
    const request = new AbortController()
    controller = request
    loading.value = true
    failed.value = false
    text.value = null
    if (!supported.value) {
        loading.value = false
        return
    }
    try {
        const result = await resourceChangelog(props.release, request.signal)
        request.signal.throwIfAborted()
        if (result.source !== props.release.project.source)
            throw new Error(formatMessage(messages.failed))
        text.value = result
        void translateBody(true)
    } catch {
        if (!request.signal.aborted) failed.value = true
    } finally {
        if (controller === request) loading.value = false
    }
}

watch(
    () => [
        props.release.project.source,
        props.release.project.projectId,
        props.release.releaseId,
        activated.value,
    ],
    () => {
        if (activated.value) void load()
    },
    { immediate: true },
)
watch(locale, () => {
    resetTranslation()
    void translateBody(true)
})
onScopeDispose(() => {
    controller?.abort()
    resetTranslation()
})
</script>

<template>
    <div ref="container" class="flex min-w-0 flex-col gap-3" :aria-busy="loading">
        <div v-if="!activated" class="h-12 rounded bg-surface-3" />
        <p v-if="!supported" class="m-0">{{ formatMessage(messages.unsupported) }}</p>
        <p v-else-if="loading" class="m-0">{{ formatMessage(commonMessages.loadingLabel) }}</p>
        <div v-else-if="failed" class="flex flex-wrap items-center gap-2">
            <span>{{ formatMessage(messages.failed) }}</span>
            <Button @click="load">{{ formatMessage(commonMessages.retryButton) }}</Button>
        </div>
        <p v-else-if="activated && empty" class="m-0 text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.empty) }}
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
                                  ? messages.original
                                  : messages.translate,
                        )
                    }}
                </Button>
                <span v-if="translationFailed">{{
                    formatMessage(messages.translationFailed)
                }}</span>
            </div>
            <template v-if="text.format === 'plain'">
                <p v-if="!active || mode === 'bilingual'" class="m-0 whitespace-pre-wrap">
                    {{ text.body }}
                </p>
                <p v-if="active" class="m-0 whitespace-pre-wrap">{{ translations.body }}</p>
            </template>
            <TranslatedProjectDescription
                v-else
                :description="text.body"
                :format="text.format"
                :active="active"
                :translations="translations"
                :mode="mode"
                :style="style"
            />
        </template>
    </div>
</template>
