<script setup lang="ts">
import './home-widget.css'

import { PlayIcon, SpinnerIcon, StopCircleIcon } from '@modrinth/assets'
import { Avatar } from '@modrinth/ui'
import type { Component } from 'vue'

import InstanceIcon from '@/components/ui/InstanceIcon.vue'

/**
 * One launchable thing in a Home list: an instance, a world or a server. The
 * name leads; the second line says where and when; the action on the right is
 * always the same play (or stop) button, so every list reads the same way.
 */
withDefaults(
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
        running?: boolean
        starting?: boolean
        playDisabled?: boolean
        playLabel: string
        stopLabel: string
        /** Hides the play/stop button, for rows that only link somewhere. */
        hideAction?: boolean
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
        running: false,
        starting: false,
        playDisabled: false,
        hideAction: false,
    },
)

defineEmits<{
    play: []
    stop: []
}>()
</script>

<template>
    <div class="home-item-row home-row group" :class="{ 'is-running': running }">
        <span class="home-item-row-icon">
            <slot v-if="$slots.icon" name="icon" />
            <InstanceIcon
                v-else-if="instance"
                :icon-path="instance.iconPath"
                :instance-id="instance.id"
                :loader="instance.loader"
                size="36px"
            />
            <Avatar v-else :src="imageSrc ?? undefined" :tint-by="imageTint ?? title" size="36px" />
            <span v-if="running" class="home-item-row-dot is-running" aria-hidden="true" />
            <span
                v-else-if="status"
                class="home-item-row-dot"
                :class="`is-${status}`"
                aria-hidden="true"
            />
        </span>
        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
            <router-link v-if="to" :to="to" class="home-item-row-title home-item-row-link">
                {{ title }}
            </router-link>
            <span v-else class="home-item-row-title">{{ title }}</span>
            <span
                v-if="meta || tag || $slots['meta-prefix']"
                class="flex min-w-0 items-center gap-1 text-xs text-[var(--color-text-tertiary)]"
            >
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

.home-item-row-title {
    @apply min-w-0 truncate text-sm font-semibold leading-snug text-[var(--color-text-primary)] no-underline;
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
        transform 120ms ease;
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

@media (prefers-reduced-motion: reduce) {
    .home-item-row-play {
        transition: none;
    }

    .home-item-row-play:active:not(:disabled) {
        transform: none;
    }
}
</style>
