<script setup lang="ts">
import { computed } from 'vue'

import { renderMarkdown } from '../../markdown'

/**
 * A rendered description or changelog (`.markdown-body`), with the omorphia `.markdown-body`
 * rules of packages/assets/styles/classes.scss, which the website does not load. Links to
 * other sites open in a new tab; in-page anchors scroll inside the launcher window.
 */
const props = defineProps<{ source: string }>()

const html = computed(() => renderMarkdown(props.source))

function onClick(event: MouseEvent) {
    const anchor = (event.target as HTMLElement | null)?.closest('a')
    if (!anchor) return
    const href = anchor.getAttribute('href') ?? ''
    if (href.startsWith('#')) {
        event.preventDefault()
        const id = decodeURIComponent(href.slice(1))
        const container = event.currentTarget as HTMLElement
        container
            .querySelector(`[id="${CSS.escape(id)}"]`)
            ?.scrollIntoView({ block: 'start', behavior: 'smooth' })
        return
    }
    if (!/^(?:https?:|mailto:|tel:|ftp:)/i.test(href)) event.preventDefault()
}
</script>

<template>
    <!-- eslint-disable-next-line vue/no-v-html -->
    <div class="markdown-body" @click="onClick" v-html="html" />
</template>

<style scoped>
.markdown-body :deep(h1:first-child) {
    margin-block-start: 0;
    padding-block-start: 0;
}

.markdown-body :deep(:is(blockquote, details, dl, ol, p, code, pre, table, ul)) {
    margin-top: 0;
    margin-bottom: 16px;
}

.markdown-body :deep(:is(li, p)) {
    padding: 0;
    line-height: 1.5;
}

.markdown-body :deep(h1) {
    display: block;
}

.markdown-body :deep(:is(h1, h2, h3)) {
    color: var(--color-contrast);
}

.markdown-body :deep(:is(h1, h2)) {
    padding: 10px 0 5px;
    border-bottom: 1px solid var(--color-divider);
}

.markdown-body :deep(:is(h1, h2, h3, h4, h5, h6, li, p)) {
    word-wrap: break-word;
    overflow-wrap: anywhere;
}

.markdown-body :deep(blockquote) {
    padding: 0 1em;
    color: var(--color-base);
    border-left: 0.25em solid var(--surface-4);
    margin-inline: 0;
}

.markdown-body :deep(a) {
    cursor: pointer;
    color: var(--color-blue);
}

.markdown-body :deep(a:is(:focus-visible, :hover)) {
    filter: brightness(1.2);
    text-decoration: none;
}

.markdown-body :deep(a:active) {
    filter: brightness(1.1);
    text-decoration: none;
}

.markdown-body :deep(a:active > img) {
    scale: 0.98;
}

.markdown-body :deep(img) {
    max-width: 100%;
    display: inline-block;
}

.markdown-body :deep(img:not([height])) {
    height: auto;
}

.markdown-body :deep(pre) {
    margin-top: 1rem;
    padding: 14px;
    border-radius: var(--radius-sm);
    background-color: var(--surface-4);
    overflow-x: auto;
}

.markdown-body :deep(pre code) {
    font-size: 80%;
    padding: 0;
    border-radius: 0;
}

.markdown-body :deep(code) {
    padding: 0.2em 0.4em;
    font-size: 80%;
    border-radius: var(--radius-sm);
    background-color: var(--surface-4);
    color: var(--color-contrast);
}

.markdown-body :deep(hr) {
    margin: 20px 0;
    color: var(--surface-4);
}

.markdown-body :deep(table) {
    display: block;
    width: max-content;
    max-width: 100%;
    overflow: auto;
    border-collapse: separate;
    border-spacing: 0;
    line-height: 1.5;
    border: 0.1rem solid var(--surface-4);
    border-radius: var(--radius-xl);
}

.markdown-body :deep(table th) {
    font-weight: 600;
    background-color: var(--surface-3);
}

.markdown-body :deep(table tr) {
    background-color: var(--surface-1-5);
}

.markdown-body :deep(table :is(td, th)) {
    padding: 0.4rem 0.85rem;
}

.markdown-body :deep(table tr:nth-child(2n)) {
    background-color: var(--surface-1);
}

.markdown-body :deep(table :is(td, th):not(:last-of-type)) {
    border-right: 0.1rem solid var(--surface-4);
}

.markdown-body :deep(table :is(tr:not(:last-of-type) td, th)) {
    border-bottom: 0.1rem solid var(--surface-4);
}

.markdown-body :deep(details) {
    border: 0.15rem solid var(--surface-4);
    border-radius: var(--radius-sm);
    padding: 0.5rem 0.5rem 0;
}

.markdown-body :deep(details summary) {
    font-weight: bold;
    margin: -0.5rem -0.5rem 0;
    padding: 0.5rem 0.8rem;
    cursor: pointer;
    background-color: var(--surface-4);
    border-radius: var(--radius-xs);
}

.markdown-body :deep(details summary:hover) {
    filter: brightness(0.85);
}

.markdown-body :deep(details[open]) {
    padding: 0.5rem;
}

.markdown-body :deep(details[open] summary) {
    margin-bottom: 0.5rem;
    border-radius: var(--radius-xs) var(--radius-xs) 0 0;
}

.markdown-body :deep(details[open] > :last-child:not(summary)) {
    margin-bottom: 0 !important;
}

.markdown-body :deep(details[open] > details:not([open]) summary) {
    margin: -0.5em -0.5em 0;
    border-radius: var(--radius-xs);
}

.markdown-body > :deep(:last-child) {
    margin-bottom: 0 !important;
}

.markdown-body :deep(ul) {
    padding-left: 1.25rem;
}
</style>
