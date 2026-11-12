<template>
    <NavIconSvg :motion="motion">
        <path
            class="discover-needle"
            d="m16.24 7.76-1.804 5.411a2 2 0 0 1-1.265 1.265L7.76 16.24l1.804-5.411a2 2 0 0 1 1.265-1.265z"
        />
        <circle cx="12" cy="12" r="10" />
    </NavIconSvg>
</template>

<script setup lang="ts">
import { frames, type NavIconMotion, swing } from './motion'
import NavIconSvg from './NavIconSvg.vue'

const SPIN_EASING = 'cubic-bezier(0.3, 0.7, 0.4, 1)'

/** The needle spins off at once, overshoots north and swings until it settles. */
const spin = frames(
    'transform',
    [
        [0, 'rotate(0deg)'],
        [0.46, 'rotate(745deg)'],
        [0.62, 'rotate(702deg)'],
        [0.77, 'rotate(728deg)'],
        [0.9, 'rotate(716deg)'],
        [1, 'rotate(720deg)'],
    ],
    SPIN_EASING,
)

const respin = frames(
    'transform',
    [
        [0, 'rotate(0deg)'],
        [0.5, 'rotate(390deg)'],
        [0.7, 'rotate(350deg)'],
        [0.86, 'rotate(366deg)'],
        [1, 'rotate(360deg)'],
    ],
    SPIN_EASING,
)

const motion: NavIconMotion = {
    '.discover-needle': {
        enter: { keyframes: spin, duration: 1000 },
        // While the pointer stays, the needle keeps hunting for north.
        idle: {
            keyframes: swing((angle) => `rotate(${angle}deg)`, 16),
            duration: 1400,
            delay: 850,
        },
        activate: { keyframes: respin, duration: 800 },
    },
}
</script>

<style scoped>
.discover-needle {
    transform-origin: 12px 12px;
}
</style>
