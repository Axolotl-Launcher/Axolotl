<script setup lang="ts">
import { ref } from 'vue'

/**
 * components/ui/settings/storage/StorageDisclosure.vue: a native `<details>`
 * whose summary slot receives the open state. The content is rendered from the
 * first time the disclosure opens (a closed `<details>` shows nothing), so the
 * thousands of rows of a full scan only cost their DOM once they are revealed.
 */
withDefaults(defineProps<{ expandable?: boolean }>(), { expandable: true })
const open = ref(false)
const rendered = ref(false)
function toggle(event: Event) {
    open.value = (event.currentTarget as HTMLDetailsElement).open
    if (open.value) rendered.value = true
}
</script>

<template>
    <details v-if="expandable" class="storage-disclosure" @toggle="toggle">
        <summary
            class="storage-disclosure-summary cursor-pointer select-none list-none rounded-lg outline-offset-2 transition-colors duration-150 focus-visible:outline-brand active:bg-surface-4"
        >
            <slot name="summary" :open="open" />
        </summary>
        <slot v-if="rendered" />
    </details>
    <div v-else><slot name="summary" :open="false" /></div>
</template>

<style scoped>
.storage-disclosure-summary::-webkit-details-marker {
    display: none;
}
.storage-disclosure-summary::marker {
    content: none;
}
@supports (interpolate-size: allow-keywords) {
    .storage-disclosure {
        interpolate-size: allow-keywords;
    }
    .storage-disclosure::details-content {
        block-size: 0;
        opacity: 0;
        overflow: clip;
        transition:
            block-size 180ms cubic-bezier(0.23, 1, 0.32, 1),
            opacity 150ms ease,
            content-visibility 180ms;
        transition-behavior: allow-discrete;
    }
    .storage-disclosure[open]::details-content {
        block-size: auto;
        opacity: 1;
    }
}
@media (prefers-reduced-motion: reduce) {
    .storage-disclosure::details-content {
        transition:
            opacity 100ms ease,
            content-visibility 100ms;
    }
}
</style>
