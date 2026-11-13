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
        /** Shown in place of the body when set and not loading: what is missing, briefly. */
        emptyText?: string | null
        /** A second line: why it is empty or how to fill it. Dropped where there is no room. */
        emptyHint?: string | null
        emptyIcon?: Component
        /** `warning` for a failure (could not load), `neutral` for "nothing here yet". */
        emptyTone?: 'neutral' | 'warning'
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
        emptyHint: null,
        emptyIcon: undefined,
        emptyTone: 'neutral',
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
        <div
            v-else-if="emptyText"
            class="home-widget-frame-empty"
            :class="{ 'has-action': $slots['empty-action'] }"
            :data-tone="emptyTone"
        >
            <span class="home-widget-frame-empty-art" aria-hidden="true">
                <component :is="emptyIcon" v-if="emptyIcon" />
            </span>
            <div class="home-widget-frame-empty-copy">
                <p class="home-widget-frame-empty-title">{{ emptyText }}</p>
                <p v-if="emptyHint" class="home-widget-frame-empty-hint">{{ emptyHint }}</p>
            </div>
            <div v-if="$slots['empty-action']" class="home-widget-frame-empty-action">
                <slot name="empty-action" />
            </div>
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

/*
 * The empty state is quiet on purpose: one line icon in a hairline ring, a
 * plain title, an optional hint and one ghost action. No tiles, no fills, no
 * blur, so a page of empty cards reads calmer than a page with data. Only the
 * action carries the accent; a failure colours the icon, nothing else.
 *
 * Tall cards centre it. One-row cards lay it on one line: icon and title on
 * the left, the action on the right on the title's baseline. A one-row tile
 * too narrow for that keeps the icon, the title and the action, centred.
 */
.home-widget-frame-empty {
    @apply m-auto flex w-full min-w-0 max-w-[18rem] flex-col items-center gap-2 py-1 text-center;
}

.home-widget-frame-empty-art {
    @apply flex size-9 shrink-0 items-center justify-center rounded-full text-[var(--color-text-tertiary)];
    border: 1px solid var(--home-widget-edge);
}

.home-widget-frame-empty-art :deep(svg) {
    @apply size-[1.125rem];
}

.home-widget-frame-empty[data-tone='warning'] .home-widget-frame-empty-art {
    @apply text-brand-orange;
}

.home-widget-frame-empty-copy {
    @apply flex min-w-0 flex-col gap-0.5;
}

.home-widget-frame-empty-title {
    @apply m-0 text-[0.8125rem] font-medium leading-snug text-[var(--color-text-default)];
}

.home-widget-frame-empty-hint {
    @apply m-0 text-xs leading-snug text-[var(--color-text-tertiary)];
}

.home-widget-frame-empty-action {
    @apply flex min-w-0 flex-wrap items-center justify-center gap-1;
}

/* One row: icon and text on the left, the action on the right, on one baseline. */
.home-widget-frame.is-dense .home-widget-frame-empty {
    @apply my-auto grid max-w-none items-baseline gap-x-2.5 gap-y-0.5 py-0 text-left;
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-areas:
        'art copy action'
        '. copy .';
}

.home-widget-frame.is-dense .home-widget-frame-empty-art {
    @apply size-auto self-center rounded-none border-0;
    grid-area: art;
}

.home-widget-frame.is-dense .home-widget-frame-empty-copy {
    grid-area: copy;
}

.home-widget-frame.is-dense .home-widget-frame-empty-hint {
    @apply line-clamp-2;
}

.home-widget-frame.is-dense .home-widget-frame-empty-action {
    @apply flex-nowrap justify-end;
    grid-area: action;
}

@container (max-width: 24rem) {
    .home-widget-frame.is-dense .home-widget-frame-empty {
        @apply m-auto flex w-auto flex-col items-center gap-1.5 text-center;
    }

    .home-widget-frame.is-dense .home-widget-frame-empty-hint {
        display: none;
    }

    .home-widget-frame.is-dense .home-widget-frame-empty-title {
        @apply line-clamp-2;
    }

    .home-widget-frame.is-dense .home-widget-frame-empty-action {
        justify-content: center;
    }
}

/* Narrow columns drop the icon before they cut the title. */
@container (max-width: 12rem) {
    .home-widget-frame-icon {
        display: none;
    }
}
</style>
