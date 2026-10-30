<script setup lang="ts">
import { ProjectPageSkeleton } from '@modrinth/ui'
import { configuredXss, renderHighlightedString } from '@modrinth/utils'
import { computed, onScopeDispose, ref, watch } from 'vue'

import { runAfterPageTransitionSettle } from '@/helpers/page-transition'
import {
    prepareDescription,
    renderTranslatedDescription,
    type TranslationMode,
    type TranslationStyle,
} from '@/helpers/translation'

const props = defineProps<{
    description: string
    active: boolean
    translations: Record<string, string>
    mode: TranslationMode
    style: TranslationStyle
    format?: 'markdown' | 'html'
}>()

const renderedDescription = ref<string | null>(null)
let renderVersion = 0
watch(
    () => [
        props.description,
        props.format,
        props.active,
        props.translations,
        props.mode,
        props.style,
    ],
    (values, previous) => {
        const version = ++renderVersion
        if (!previous || values[0] !== previous[0] || values[1] !== previous[1])
            renderedDescription.value = null
        const { description, format, active, translations, mode, style } = props
        runAfterPageTransitionSettle(() => {
            if (version !== renderVersion) return
            const rendered = !active
                ? format === 'html'
                    ? configuredXss.process(description ?? '')
                    : renderHighlightedString(description ?? '')
                : renderTranslatedDescription(
                      prepareDescription(description, format),
                      translations,
                      mode,
                      style,
                  )
            const template = document.createElement('template')
            template.innerHTML = rendered
            for (const image of template.content.querySelectorAll('img')) {
                image.loading = 'lazy'
                image.decoding = 'async'
            }
            renderedDescription.value = template.innerHTML
        })
    },
    { immediate: true, deep: true },
)
onScopeDispose(() => {
    renderVersion++
})

const translationOnlyClass = computed(() =>
    props.active && props.mode === 'translation-only'
        ? ['ax-translation-only', `ax-translation-style-${props.style}`]
        : [],
)
</script>

<template>
    <ProjectPageSkeleton v-if="renderedDescription === null" variant="description" />
    <!-- eslint-disable-next-line vue/no-v-html -->
    <div v-else class="markdown-body" :class="translationOnlyClass" v-html="renderedDescription" />
</template>

<style scoped>
:deep(.ax-translation-block) {
    margin-block: 0.5rem 1rem;
    animation: translation-float-in 0.5s ease-out both;
}

:deep(.ax-translation-block > :first-child) {
    margin-top: 0;
}

:deep(.ax-translation-block > :last-child) {
    margin-bottom: 0;
}

:deep(.ax-translation-style-weakened) {
    color: var(--color-text-tertiary) !important;
}

:deep(.ax-translation-style-blur) {
    filter: blur(4px);
    opacity: 0.75;
    transition:
        filter 0.1s ease-in-out,
        opacity 0.1s ease-in-out;
}

:deep(.ax-translation-style-blur:hover) {
    filter: blur(0);
    opacity: 1;
}

:deep(.ax-translation-style-blockquote) {
    padding: 4px 0 4px 8px;
    border-left: 4px solid var(--color-brand);
}

:deep(.ax-translation-style-dashed-line) {
    text-decoration: underline dashed var(--color-brand) !important;
    text-underline-offset: 5px;
}

:deep(.ax-translation-style-border) {
    padding: 2px 4px;
    border: 1px solid var(--color-brand);
    border-radius: 4px;
}

:deep(.ax-translation-style-text-color) {
    color: oklch(0.693 0.17 162.48) !important;
}

:deep(.ax-translation-style-background) {
    padding: 2px 4px;
    border-radius: 4px;
    background-color: color-mix(
        in srgb,
        var(--color-brand) var(--opacity-ratio-keep-15),
        transparent
    );
}

.ax-translation-only.ax-translation-style-weakened {
    color: var(--color-text-tertiary) !important;
}

.ax-translation-only.ax-translation-style-blur {
    filter: blur(4px);
    opacity: 0.75;
    transition:
        filter 0.1s ease-in-out,
        opacity 0.1s ease-in-out;
}

.ax-translation-only.ax-translation-style-blur:hover {
    filter: blur(0);
    opacity: 1;
}

.ax-translation-only.ax-translation-style-blockquote {
    padding: 4px 0 4px 8px;
    border-left: 4px solid var(--color-brand);
}

.ax-translation-only.ax-translation-style-dashed-line {
    text-decoration: underline dashed var(--color-brand) !important;
    text-underline-offset: 5px;
}

.ax-translation-only.ax-translation-style-border {
    padding: 2px 4px;
    border: 1px solid var(--color-brand);
    border-radius: 4px;
}

.ax-translation-only.ax-translation-style-text-color {
    color: oklch(0.693 0.17 162.48) !important;
}

.ax-translation-only.ax-translation-style-background {
    padding: 2px 4px;
    border-radius: 4px;
    background-color: color-mix(
        in srgb,
        var(--color-brand) var(--opacity-ratio-keep-15),
        transparent
    );
}

@keyframes translation-float-in {
    from {
        opacity: 0;
        transform: translateY(12px);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}
</style>
