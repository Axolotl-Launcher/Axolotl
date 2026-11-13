<script setup lang="ts">
import './home-widget.css'

import type { Component } from 'vue'

/*
 * The shared widget layout: header (accent icon, title, badge, actions), then
 * a loading skeleton, an empty state or the body. The surface belongs to the
 * container (a dashboard tile or the sidebar), so the frame only reads the
 * tokens from home-widget.css and looks the same in both.
 */

withDefaults(
    defineProps<{
        title: string
        icon?: Component
        /** Small figure beside the title, such as a count or a total. */
        badge?: string | number | null
        loading?: boolean
        /** Shown in place of the body when set and not loading. */
        emptyText?: string | null
        emptyIcon?: Component
        /** Rows the loading skeleton draws. */
        skeletonRows?: number
        /** Compact header for 1-row widgets. */
        dense?: boolean
    }>(),
    {
        icon: undefined,
        badge: null,
        loading: false,
        emptyText: null,
        emptyIcon: undefined,
        skeletonRows: 3,
        dense: false,
    },
)
</script>

<template>
    <section class="home-widget-frame home-surface-bare" :class="{ 'is-dense': dense }">
        <header class="home-widget-frame-header">
            <span v-if="icon" class="home-widget-frame-icon" aria-hidden="true">
                <component :is="icon" />
            </span>
            <h2 class="home-widget-frame-title">{{ title }}</h2>
            <span v-if="badge !== null && badge !== ''" class="home-widget-frame-badge">
                {{ badge }}
            </span>
            <div v-if="$slots.actions" class="home-widget-frame-actions">
                <slot name="actions" />
            </div>
        </header>

        <div v-if="loading" class="home-widget-frame-body" aria-hidden="true">
            <div v-for="row in skeletonRows" :key="row" class="flex items-center gap-3 px-1 py-1.5">
                <div class="home-skeleton size-9 shrink-0" />
                <div class="flex min-w-0 flex-1 flex-col gap-1.5">
                    <div class="home-skeleton h-3" :style="{ width: `${85 - row * 12}%` }" />
                    <div class="home-skeleton h-2.5 w-2/5" />
                </div>
            </div>
        </div>
        <div v-else-if="emptyText" class="home-widget-frame-empty">
            <span v-if="emptyIcon" class="home-widget-frame-empty-icon" aria-hidden="true">
                <component :is="emptyIcon" />
            </span>
            <p class="m-0">{{ emptyText }}</p>
            <slot name="empty-action" />
        </div>
        <div v-else class="home-widget-frame-body">
            <slot />
        </div>
    </section>
</template>

<style scoped>
.home-widget-frame {
    @apply flex min-h-0 min-w-0 flex-col gap-3 overflow-hidden;
    container-type: inline-size;
}

.home-widget-frame.is-dense {
    @apply gap-2;
}

.home-widget-frame-header {
    @apply flex h-7 min-w-0 flex-none items-center gap-2;
}

.home-widget-frame-icon {
    @apply flex size-7 shrink-0 items-center justify-center rounded-[var(--radius-sm)] text-brand;
    background: var(--home-widget-accent-tint);
}

.home-widget-frame-icon :deep(svg) {
    @apply size-4;
}

.home-widget-frame-title {
    @apply m-0 min-w-0 truncate text-[0.9375rem] font-bold leading-tight text-[var(--color-text-primary)];
}

.home-widget-frame-badge {
    @apply shrink-0 rounded-full px-1.5 text-xs font-bold leading-5 text-[var(--color-text-tertiary)];
    background: var(--home-widget-tint);
}

.home-widget-frame-actions {
    @apply ml-auto flex shrink-0 items-center gap-0.5;
}

.home-widget-frame-body {
    @apply flex min-h-0 min-w-0 flex-1 flex-col;
}

.home-widget-frame-empty {
    @apply m-auto flex max-w-[18rem] flex-col items-center gap-2.5 px-2 text-center text-[0.8125rem] leading-snug text-[var(--color-text-tertiary)];
}

.home-widget-frame-empty-icon {
    @apply flex size-10 items-center justify-center rounded-full text-[var(--color-text-tertiary)];
    background: var(--home-widget-tint);
}

.home-widget-frame-empty-icon :deep(svg) {
    @apply size-5;
}

/* A one-row tile keeps the empty state to its text and its next step. */
.home-widget-frame.is-dense .home-widget-frame-empty {
    @apply gap-2 text-xs;
}

.home-widget-frame.is-dense .home-widget-frame-empty-icon {
    display: none;
}

.home-widget-frame.is-dense .home-widget-frame-empty p {
    @apply line-clamp-2;
}

/* Narrow columns drop the icon before they cut the title. */
@container (max-width: 12rem) {
    .home-widget-frame-icon {
        display: none;
    }
}
</style>
