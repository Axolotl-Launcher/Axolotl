<script setup lang="ts">
import type { ProjectDetail } from '../../../../data'
import MarkdownBody from './MarkdownBody.vue'

/**
 * The Description tab (apps/app-frontend/src/pages/project/Description.vue): the project body
 * in a `Card`, through `ResourceDescription` and `TranslatedProjectDescription`. A Modrinth
 * project has a single description source, so no source switcher is shown.
 */
defineProps<{ project: ProjectDetail }>()
</script>

<template>
    <div
        class="project-description-card relative mb-[var(--gap-md)] min-h-[var(--font-size-2xl)] rounded-[var(--radius-lg)] border border-solid border-surface-4 bg-surface-3 p-4 outline outline-2 outline-transparent"
    >
        <div class="flex flex-col gap-3" :aria-busy="false">
            <MarkdownBody :source="project.body ?? ''" />
        </div>
    </div>
</template>

<style scoped>
/* packages/assets/styles/classes.scss `.universal-body` (the `.card` body rules). */
.project-description-card :deep(:is(ul, ol) :is(ul, ol)) {
    margin-bottom: 0;
}

.project-description-card > :deep(:first-child) {
    margin-block-start: 0;
}

.project-description-card > :deep(:last-child) {
    margin-block-end: 0;
}
</style>
