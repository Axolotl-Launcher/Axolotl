<script setup lang="ts">
import searchingVisual from './axo-searching.svg?url'
import teachingVisual from './axo-teaching.svg?url'

/** The tour's axolotl guide (apps/app-frontend/src/components/ui/onboarding/OnboardingMascotStage.vue). */
defineProps<{
    alt: string
}>()

const mascotVisuals = [teachingVisual, searchingVisual]
const mascotVisual = mascotVisuals[Math.floor(Math.random() * mascotVisuals.length)]
</script>

<template>
    <img class="onboarding-mascot" :src="mascotVisual" :alt="alt" />
</template>

<style scoped>
/* `clamp(6rem, 11vw, 9rem)` in the 1280px window. */
.onboarding-mascot {
    width: 140.8px;
    height: auto;
    object-fit: contain;
}
</style>
