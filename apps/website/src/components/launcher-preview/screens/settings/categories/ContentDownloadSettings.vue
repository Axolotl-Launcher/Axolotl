<script setup lang="ts">
import AppearanceSettings from './AppearanceSettings.vue'
import ResourceManagementSettings from './ResourceManagementSettings.vue'

/**
 * Content & downloads, ported from `ContentDownloadSettings.vue`: the content
 * behavior block of the appearance settings followed by the resource
 * management sections.
 */
</script>

<template>
    <div class="flex flex-col gap-6">
        <AppearanceSettings scope="content-downloads" />
        <ResourceManagementSettings scope="content-downloads" />
    </div>
</template>
