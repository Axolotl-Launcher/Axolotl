<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'

defineProps<{
    eyebrow: string
    title: string
    description: string
    updatedAt: string
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    updated: {
        id: 'axolotl-site.legal.updated',
        defaultMessage: 'Last updated: {updatedAt}',
    },
})
</script>

<template>
    <article class="legal-document">
        <header class="legal-header">
            <span class="section-eyebrow">{{ eyebrow }}</span>
            <h1>{{ title }}</h1>
            <p>{{ description }}</p>
            <p class="updated-at">{{ formatMessage(messages.updated, { updatedAt }) }}</p>
        </header>

        <div class="legal-content">
            <slot />
        </div>
    </article>
</template>

<style scoped lang="scss">
.legal-document {
    width: min(52rem, calc(100% - 2rem));
    margin: 0 auto;
    padding: 4rem 0 5rem;
}

.legal-header {
    max-width: 44rem;
    margin-bottom: 2.5rem;

    h1 {
        margin: 0.5rem 0 0;
        color: var(--color-contrast);
        font-size: 2.25rem;
        line-height: 1.15;
    }

    p {
        margin: 1rem 0 0;
        color: var(--color-secondary);
        line-height: 1.65;
    }

    .updated-at {
        font-size: 0.875rem;
    }
}

.legal-content {
    padding: clamp(1.25rem, 3vw, 2.25rem);
    border: 1px solid var(--surface-5);
    border-radius: 0.75rem;
    background: var(--surface-4);
    color: var(--color-base);
    line-height: 1.8;

    :deep(h2) {
        margin: 2.25rem 0 0.75rem;
        color: var(--color-contrast);
        font-size: 1.25rem;
        line-height: 1.35;
    }

    :deep(h2:first-child) {
        margin-top: 0;
    }

    :deep(p) {
        margin: 0.75rem 0;
    }

    :deep(ul) {
        margin: 0.75rem 0;
        padding-left: 1.5rem;
    }

    :deep(li + li) {
        margin-top: 0.4rem;
    }

    :deep(a) {
        color: var(--color-brand);
        text-underline-offset: 0.18rem;
    }

    :deep(strong) {
        color: var(--color-contrast);
    }
}

@media (max-width: 600px) {
    .legal-document {
        padding: 2.5rem 0 3rem;
    }

    .legal-header h1 {
        font-size: 1.875rem;
    }
}
</style>
