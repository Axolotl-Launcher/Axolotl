<script setup lang="ts">
import { computed } from 'vue'

import { HOME_MINIMAL_STANDARD, type HomeMinimalLayout } from '@/components/home/home-minimal'
import HomeGreeting from '@/components/home/HomeGreeting.vue'
import HomeLaunchCard from '@/components/home/HomeLaunchCard.vue'
import type { GameInstance } from '@/helpers/types'

const props = withDefaults(
    defineProps<{
        instances: GameInstance[]
        playerName?: string | null
        layout?: HomeMinimalLayout | null
    }>(),
    { playerName: null, layout: null },
)

const resolved = computed(() => props.layout ?? HOME_MINIMAL_STANDARD)
/** With neither the date nor the heading there is no greeting block, not an empty one. */
const showGreeting = computed(() => resolved.value.show.date || resolved.value.show.greeting)
</script>

<template>
    <section
        class="minimal-home-stage flex min-w-0 items-center justify-center px-6"
        :data-position="resolved.position"
        :data-density="resolved.density"
        :data-arrangement="resolved.arrangement"
    >
        <div class="minimal-home-column flex w-full flex-col items-stretch">
            <HomeGreeting
                v-if="showGreeting"
                :player-name="playerName"
                variant="minimal"
                :show-date="resolved.show.date"
                :show-heading="resolved.show.greeting"
            />
            <HomeLaunchCard :instances="instances" variant="stage" :stage="resolved" />
        </div>
    </section>
</template>

<style scoped>
.minimal-home-stage {
    min-height: calc(100vh - var(--top-bar-height) - 4rem);
    padding-top: 2rem;
    padding-bottom: 2rem;
}

/* Optically centred: a little above the middle reads as centred. */
.minimal-home-stage[data-position='upper'] {
    padding-bottom: 14vh;
}

.minimal-home-column {
    max-width: 54rem;
    gap: 2rem;
}

.minimal-home-stage[data-density='compact'] .minimal-home-column {
    max-width: 46rem;
    gap: 1.25rem;
}
</style>
