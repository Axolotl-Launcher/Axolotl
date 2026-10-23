<script setup lang="ts">
import { useRoute } from 'vue-router'

import ScreenshotsPage from '../screenshots/ScreenshotsPage.vue'

/**
 * Port of `apps/app-frontend/src/pages/instance/Screenshots.vue`: the shared
 * screenshots page, narrowed to the instance's own captures.
 */
defineOptions({ inheritAttrs: false })

const route = useRoute()
</script>

<template>
    <div class="box-border h-full p-6">
        <ScreenshotsPage :instance-id="String(route.params.id)" show-heading />
    </div>
</template>
