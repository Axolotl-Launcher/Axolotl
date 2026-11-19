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
import { frames, type NavIconMotion } from './motion'
import NavIconSvg from './NavIconSvg.vue'

/**
 * The needle spins off at once, two full turns, and stops dead on north: most
 * of the way goes by in the first quarter, the rest is one long, smooth stop.
 */
const spin = frames('transform', [
    [0, 'rotate(0deg)', 'cubic-bezier(0.3, 0.44, 0.7, 0.82)'],
    [0.25, 'rotate(492deg)', 'cubic-bezier(0.15, 0.6, 0.35, 1)'],
    [1, 'rotate(720deg)'],
])

/**
 * While the pointer stays, the needle keeps turning. It looks the same either
 * way round, so each cycle is a half turn that ends at rest.
 */
const turn = frames('transform', [
    [0, 'rotate(0deg)', 'linear'],
    [1, 'rotate(180deg)'],
])

/** A click strikes the needle: it is squeezed small, then flicked on half a turn. */
const flick = frames(
    'transform',
    [
        [0, 'rotate(0deg) scale(1)'],
        [0.14, 'rotate(0deg) scale(0.5)'],
        [0.42, 'rotate(150deg) scale(1.35)'],
        [0.66, 'rotate(180deg) scale(0.92)'],
        [0.84, 'rotate(180deg) scale(1.03)'],
        [1, 'rotate(180deg) scale(1)'],
    ],
    'ease-out',
)

const motion: NavIconMotion = {
    '.discover-needle': {
        enter: { keyframes: spin, duration: 1000 },
        idle: { keyframes: turn, duration: 1600, delay: 700 },
        activate: { keyframes: flick, duration: 700 },
    },
}
</script>

<style scoped>
.discover-needle {
    transform-origin: 12px 12px;
}
</style>
