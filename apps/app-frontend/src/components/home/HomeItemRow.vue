<script setup lang="ts">
import './home-widget.css'

import { PlayIcon, SpinnerIcon, StopCircleIcon } from '@modrinth/assets'
import { Avatar } from '@modrinth/ui'
import { type Component, computed } from 'vue'

import InstanceIcon from '@/components/ui/InstanceIcon.vue'

import type { HomeItemVariant } from './home-utils'

/**
 * One launchable thing in a Home list: an instance, a world or a server. The
 * name leads; the second line says where and when; the action is always the
 * same play (or stop) button, so every list reads the same way.
 *
 * `row` is a plain list line. `card` is the same line on a tile of its own,
 * for a strip of items in a wide, one-row widget. `tile` puts a large icon on
 * top like a cover and floats the play button over its corner on hover.
 */
const props = withDefaults(
    defineProps<{
        title: string
        /** Where the row leads. The whole row is clickable; the actions stay separate. */
        to?: string | null
        meta?: string | null
        metaIcon?: Component | null
        /** A small label before the meta line, such as "Local". */
        tag?: string | null
        instance?: { id: string; iconPath?: string | null; loader?: string | null } | null
        imageSrc?: string | null
        imageTint?: string | null
        /** A dot on the icon's corner. */
        status?: 'online' | 'offline' | 'pending' | null
        /** Fades the icon, for a server that does not answer. */
        dimmed?: boolean
        running?: boolean
        starting?: boolean
        playDisabled?: boolean
        /** Disables the Stop button, e.g. while a server is already shutting down. */
        stopDisabled?: boolean
        playLabel: string
        stopLabel: string
        /** Hides the play/stop button, for rows that only link somewhere. */
        hideAction?: boolean
        variant?: HomeItemVariant
    }>(),
    {
        to: null,
        meta: null,
        metaIcon: null,
        tag: null,
        instance: null,
        imageSrc: null,
        imageTint: null,
        status: null,
        dimmed: false,
        running: false,
        starting: false,
        playDisabled: false,
        stopDisabled: false,
        hideAction: false,
        variant: 'row',
    },
)

defineEmits<{
    play: []
    stop: []
}>()

const iconSize = computed(() => (props.variant === 'tile' ? '52px' : '32px'))
</script>

<template>
    <div
        class="home-item-row home-row group"
        :class="[`is-${variant}`, { 'is-running': running, 'is-dimmed': dimmed }]"
    >
        <span class="home-item-row-icon">
            <slot v-if="$slots.icon" name="icon" :size="iconSize" />
            <InstanceIcon
                v-else-if="instance"
                :icon-path="instance.iconPath"
                :instance-id="instance.id"
                :loader="instance.loader"
                :size="iconSize"
            />
            <Avatar
                v-else
                :src="imageSrc ?? undefined"
                :tint-by="imageTint ?? title"
                :size="iconSize"
            />
            <span v-if="running" class="home-item-row-dot is-running" aria-hidden="true" />
            <span
                v-else-if="status"
                class="home-item-row-dot"
                :class="`is-${status}`"
                aria-hidden="true"
            />
        </span>
        <span class="home-item-row-copy">
            <router-link v-if="to" :to="to" class="home-item-row-title home-item-row-link">
                {{ title }}
            </router-link>
            <span v-else class="home-item-row-title">{{ title }}</span>
            <span v-if="meta || tag || $slots['meta-prefix']" class="home-item-row-meta">
                <slot name="meta-prefix" />
                <span v-if="tag" class="home-chip shrink-0 !px-1.5 !text-[10px] !leading-4">{{
                    tag
                }}</span>
                <component
                    :is="metaIcon"
                    v-if="metaIcon"
                    class="size-3 shrink-0"
                    aria-hidden="true"
                />
                <span class="truncate">{{ meta }}</span>
            </span>
            <slot name="extra" />
        </span>
        <span class="home-item-row-actions">
            <slot name="actions" />
            <template v-if="!hideAction">
                <button
                    v-if="running"
                    v-tooltip="stopLabel"
                    type="button"
                    class="home-item-row-play is-stop"
                    :aria-label="stopLabel"
                    :disabled="stopDisabled"
                    @click="$emit('stop')"
                >
                    <StopCircleIcon aria-hidden="true" />
                </button>
                <button
                    v-else
                    v-tooltip="playLabel"
                    type="button"
                    class="home-item-row-play"
                    :aria-label="playLabel"
                    :disabled="starting || playDisabled"
                    @click="$emit('play')"
                >
                    <SpinnerIcon v-if="starting" class="animate-spin" aria-hidden="true" />
                    <PlayIcon v-else aria-hidden="true" />
                </button>
            </template>
        </span>
    </div>
</template>

<style scoped>
.home-item-row {
    @apply relative flex min-w-0 items-center gap-2.5 px-2 py-1.5;
}

.home-item-row-icon {
    @apply relative flex shrink-0;
}

.home-item-row-icon :deep(img),
.home-item-row-icon :deep(.avatar) {
    border-radius: var(--radius-md);
    transition:
        filter 160ms ease,
        opacity 160ms ease;
}

/* A server that does not answer: its picture goes grey, so it reads as off. */
.home-item-row.is-dimmed .home-item-row-icon :deep(img),
.home-item-row.is-dimmed .home-item-row-icon :deep(.avatar) {
    filter: grayscale(1);
    opacity: 0.55;
}

.home-item-row-dot {
    @apply absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full;
    box-shadow: 0 0 0 2px var(--home-widget-fill, transparent);
}

.home-item-row-dot.is-online,
.home-item-row-dot.is-running {
    @apply bg-brand-green;
}

.home-item-row-dot.is-offline {
    @apply bg-brand-red;
}

.home-item-row-dot.is-pending {
    @apply bg-[var(--color-text-tertiary)];
}

.home-item-row-copy {
    @apply flex min-w-0 flex-1 flex-col gap-0.5;
}

/*
 * A plain row is kept low (32px icon, two tight lines), so a one-row widget
 * holds two whole rows and a tall one a whole number of them.
 */
.home-item-row.is-row {
    @apply py-1;
}

.home-item-row.is-row .home-item-row-title {
    @apply leading-tight;
}

.home-item-row-title {
    @apply min-w-0 truncate text-sm font-semibold leading-snug text-[var(--color-text-primary)] no-underline;
}

.home-item-row-meta {
    @apply flex min-w-0 items-center gap-1 text-xs text-[var(--color-text-tertiary)];
}

/* The title link covers the row, so the whole row opens the item. */
.home-item-row-link::after {
    position: absolute;
    inset: 0;
    border-radius: var(--radius-md);
    content: '';
}

.home-item-row-link:focus-visible {
    outline: none;
}

.home-item-row-link:focus-visible::after {
    box-shadow: 0 0 0 3px var(--color-brand-shadow);
}

.home-item-row-actions {
    @apply relative z-[1] flex shrink-0 items-center gap-1;
}

.home-item-row-play {
    @apply flex size-8 cursor-pointer items-center justify-center rounded-full border-0 p-0 text-brand;
    background: var(--home-widget-accent-tint);
    transition:
        background-color 120ms ease,
        color 120ms ease,
        transform 120ms ease,
        opacity 120ms ease;
}

.home-item-row-play svg {
    @apply size-4;
}

.home-item-row-play:hover:not(:disabled),
.home-item-row-play:focus-visible {
    @apply bg-brand text-[var(--color-accent-contrast)];
    outline: none;
}

.home-item-row-play:focus-visible {
    box-shadow: 0 0 0 3px var(--color-brand-shadow);
}

.home-item-row-play:active:not(:disabled) {
    transform: scale(0.94);
}

.home-item-row-play:disabled {
    @apply cursor-not-allowed opacity-50;
}

.home-item-row-play.is-stop {
    @apply bg-highlight-red text-brand-red;
}

.home-item-row-play.is-stop:hover,
.home-item-row-play.is-stop:focus-visible {
    @apply bg-brand-red text-white;
}

/*
 * Card: the row on a tile of its own, side by side with the others. It is
 * kept low, so a one-row widget shows two rows of cards whole.
 */
.home-item-row.is-card {
    @apply gap-2.5 px-2 py-1;
    background: var(--home-widget-tint);
    box-shadow: inset 0 0 0 1px var(--home-item-edge);
}

.home-item-row.is-card .home-item-row-copy {
    @apply gap-0;
}

.home-item-row.is-card .home-item-row-title {
    @apply text-[0.8125rem] leading-tight;
}

.home-item-row.is-card .home-item-row-play {
    @apply size-7;
}

.home-item-row.is-card .home-item-row-play svg {
    @apply size-3.5;
}

.home-item-row.is-card:hover,
.home-item-row.is-card:focus-within {
    --home-item-edge: color-mix(in srgb, var(--color-text-primary) 14%, transparent);
    background: var(--home-widget-tint-strong);
}

/*
 * Tile: the icon is the cover, the name and details sit under it, and the
 * play button floats over the top corner when the tile is pointed at or
 * focused. A running item keeps its stop button showing.
 */
/*
 * Cards and tiles are a step up from the widget: a hairline edge and, on
 * tiles, a light top that fades down, so they read as objects on the card
 * rather than flat patches. Pointing at one lifts it. In the light theme the
 * step is a raised white face with a soft shadow instead of a grey patch.
 */
.home-item-row.is-card,
.home-item-row.is-tile {
    --home-item-edge: color-mix(in srgb, var(--color-text-primary) 7%, transparent);
    --home-item-lift: 0 0 0 0 transparent;
}

.home-item-row.is-tile {
    @apply h-full flex-col items-start gap-2.5 p-3;
    background: linear-gradient(
        to bottom,
        var(--home-widget-tint-strong),
        var(--home-widget-tint) 55%
    );
    box-shadow:
        inset 0 0 0 1px var(--home-item-edge),
        var(--home-item-lift);
    transition:
        background-color 120ms ease,
        box-shadow 160ms ease,
        transform 160ms ease;
}

.home-item-row.is-tile:hover,
.home-item-row.is-tile:focus-within {
    --home-item-edge: color-mix(in srgb, var(--color-text-primary) 14%, transparent);
    background: var(--home-widget-tint-strong);
}

:global(.light-mode .home-item-row:is(.is-card, .is-tile)) {
    --home-item-edge: color-mix(in srgb, var(--color-text-primary) 9%, transparent);
    background: color-mix(in srgb, var(--surface-4-opaque) 80%, transparent);
}

:global(.light-mode .home-item-row.is-tile) {
    --home-item-lift: 0 1px 2px color-mix(in srgb, var(--color-text-primary) 8%, transparent);
}

:global(.light-mode .home-item-row.is-tile:is(:hover, :focus-within)) {
    --home-item-edge: color-mix(in srgb, var(--color-brand) 35%, transparent);
    --home-item-lift: 0 6px 16px color-mix(in srgb, var(--color-text-primary) 10%, transparent);
    background: var(--surface-4-opaque);
}

:global(.light-mode .home-item-row.is-card:is(:hover, :focus-within)) {
    background: var(--surface-4-opaque);
}

.home-item-row.is-tile:hover {
    transform: translateY(-2px);
}

.home-item-row.is-tile .home-item-row-icon :deep(img),
.home-item-row.is-tile .home-item-row-icon :deep(.avatar) {
    border-radius: var(--radius-lg);
    box-shadow: 0 4px 14px color-mix(in srgb, var(--surface-1-opaque) 35%, transparent);
}

.home-item-row.is-tile .home-item-row-copy {
    @apply w-full flex-none;
}

.home-item-row.is-tile .home-item-row-title {
    @apply text-[0.9375rem] font-bold;
}

.home-item-row.is-tile .home-item-row-actions {
    @apply absolute right-2 top-2;
}

.home-item-row.is-tile .home-item-row-play {
    @apply size-9 bg-brand text-[var(--color-accent-contrast)];
    box-shadow: 0 4px 12px color-mix(in srgb, var(--color-brand) 40%, transparent);
}

.home-item-row.is-tile .home-item-row-play.is-stop {
    @apply bg-brand-red text-white;
    box-shadow: none;
}

/*
 * Only a mouse can point at a tile first, so only there do the actions wait
 * for it. On a touch screen (no hover, or a coarse pointer) they always show.
 */
@media (hover: hover) and (pointer: fine) {
    .home-item-row.is-tile:not(.is-running) .home-item-row-actions {
        opacity: 0;
        transform: translateY(3px) scale(0.92);
        transition:
            opacity 140ms ease,
            transform 140ms ease;
    }

    .home-item-row.is-tile:hover .home-item-row-actions,
    .home-item-row.is-tile:focus-within .home-item-row-actions {
        opacity: 1;
        transform: none;
    }
}

@media (prefers-reduced-motion: reduce) {
    .home-item-row-play,
    .home-item-row.is-tile,
    .home-item-row.is-tile .home-item-row-actions,
    .home-item-row-icon :deep(img),
    .home-item-row-icon :deep(.avatar) {
        transition: none;
    }

    .home-item-row-play:active:not(:disabled),
    .home-item-row.is-tile:hover {
        transform: none;
    }
}
</style>
