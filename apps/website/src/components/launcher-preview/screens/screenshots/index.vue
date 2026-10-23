<script setup lang="ts">
import ScreenshotsPage from './ScreenshotsPage.vue'

/**
 * The app's `/screenshots` route (apps/app-frontend/src/pages/Screenshots.vue):
 * the page fills the content pane and keeps its own 1.5rem padding.
 */
defineOptions({ name: 'ScreenshotsPage' })
</script>

<template>
    <div class="box-border h-full p-6">
        <ScreenshotsPage show-heading />
    </div>
</template>
