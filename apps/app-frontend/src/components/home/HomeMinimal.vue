<script setup lang="ts">
import HomeGreeting from '@/components/home/HomeGreeting.vue'
import HomeLaunchCard from '@/components/home/HomeLaunchCard.vue'
import type { GameInstance } from '@/helpers/types'

defineProps<{
    instances: GameInstance[]
    playerName?: string | null
}>()
</script>

<template>
    <section
        class="minimal-home-stage flex min-w-0 items-center justify-center px-6 pb-[14vh] pt-8"
    >
        <!-- Optically centred: a little above the middle reads as centred. -->
        <div class="flex w-full max-w-[54rem] flex-col items-stretch gap-8">
            <HomeGreeting :player-name="playerName" variant="minimal" />
            <HomeLaunchCard :instances="instances" variant="stage" />
        </div>
    </section>
</template>

<style scoped>
.minimal-home-stage {
    min-height: calc(100vh - var(--top-bar-height) - 4rem);
}
</style>
