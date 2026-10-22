<script setup lang="ts">
import { ref } from 'vue'

withDefaults(defineProps<{ expandable?: boolean }>(), { expandable: true })
const open = ref(false)
function toggle(event: Event) {
    open.value = (event.currentTarget as HTMLDetailsElement).open
}
</script>

<template>
    <details v-if="expandable" class="storage-disclosure" @toggle="toggle">
        <summary
            class="storage-disclosure-summary list-none cursor-pointer select-none rounded-lg outline-offset-2 transition-colors duration-150 active:bg-surface-4 focus-visible:outline-brand"
        >
            <slot name="summary" :open="open" />
        </summary>
        <slot />
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
