<script setup lang="ts">
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'

/**
 * A development-team avatar (apps/app-frontend/src/components/ui/settings/
 * TeamMemberAvatar.vue). The app shows the bundled avatar first and swaps in a
 * configured remote one once it decodes; the preview never fetches remote
 * images, so the bundled avatar stays, as in the app when the remote one does
 * not arrive in time.
 */
withDefaults(
    defineProps<{
        name: string
        src: string
        size?: string
    }>(),
    {
        size: '4rem',
    },
)
</script>

<template>
    <Avatar :src="src" :alt="name" :size="size" circle no-shadow />
</template>
