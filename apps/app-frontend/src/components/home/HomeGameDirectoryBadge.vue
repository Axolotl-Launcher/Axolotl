<script setup lang="ts">
import { FolderIcon } from '@modrinth/assets'
import { useElementSize } from '@vueuse/core'
import { ref } from 'vue'

import GameDirectoryBadge from '@/components/ui/instance/GameDirectoryBadge.vue'
import type { GameDirectoryInfo } from '@/helpers/game-directory'

import HomeScrollingText from './HomeScrollingText.vue'

defineProps<{
    directory: GameDirectoryInfo
}>()

const badge = ref<HTMLElement>()
const expanded = ref(false)
const { width } = useElementSize(badge)
</script>

<template>
    <span
        ref="badge"
        class="z-[1] inline-flex min-w-0 max-w-[45%] shrink-0"
        @mouseenter="expanded = true"
        @mouseleave="expanded = false"
    >
        <GameDirectoryBadge :directory="directory" />
        <!-- The metadata line is the containing block; the badge keeps its place in the row. -->
        <span
            v-if="directory.path"
            class="home-directory-reveal absolute inset-y-0 left-0 inline-flex min-w-0 items-center gap-1 overflow-hidden rounded-md bg-[var(--surface-4-opaque)] px-1.5 py-0.5 text-xs font-medium leading-4 text-[var(--color-text-tertiary)]"
            :class="{ 'is-expanded': expanded }"
            :style="{ '--badge-width': `${width}px` }"
            :title="directory.path"
            aria-hidden="true"
        >
            <FolderIcon class="size-3 shrink-0" />
            <HomeScrollingText :text="directory.path" :active="expanded" pause-at="end" />
        </span>
    </span>
</template>

<style scoped>
.home-directory-reveal {
    pointer-events: none;
    width: var(--badge-width);
    opacity: 0;
    transition:
        width 240ms cubic-bezier(0.22, 1, 0.36, 1),
        opacity 160ms ease;
}

.home-directory-reveal.is-expanded {
    pointer-events: auto;
    width: 100%;
    opacity: 1;
}

@media (prefers-reduced-motion: reduce) {
    .home-directory-reveal {
        transition: none;
    }
}
</style>
