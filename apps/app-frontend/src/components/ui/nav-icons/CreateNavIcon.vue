<template>
    <NavIconSvg :motion="motion">
        <g class="create-plus">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
        </g>
    </NavIconSvg>
</template>

<script setup lang="ts">
import { bob, frames, type NavIconMotion } from './motion'
import NavIconSvg from './NavIconSvg.vue'

const TURN_EASING = 'cubic-bezier(0.3, 0.7, 0.4, 1)'

/** A springy half turn; the plus is symmetric, so it ends where it started. */
const halfTurn = frames(
    'transform',
    [
        [0, 'rotate(0deg)'],
        [0.45, 'rotate(200deg)'],
        [0.65, 'rotate(170deg)'],
        [0.82, 'rotate(184deg)'],
        [1, 'rotate(180deg)'],
    ],
    TURN_EASING,
)

/** A click squeezes the plus small, then bursts it open with a quarter twist. */
const burst = frames(
    'transform',
    [
        [0, 'rotate(0deg) scale(1)'],
        [0.14, 'rotate(0deg) scale(0.62)'],
        [0.4, 'rotate(45deg) scale(1.38)'],
        [0.62, 'rotate(84deg) scale(0.93)'],
        [0.8, 'rotate(92deg) scale(1.03)'],
        [1, 'rotate(90deg) scale(1)'],
    ],
    'ease-out',
)

const motion: NavIconMotion = {
    '.create-plus': {
        enter: { keyframes: halfTurn, duration: 700 },
        // While the pointer stays, the plus keeps pulsing as an invitation.
        idle: { keyframes: bob((size) => `scale(${size})`, 1.16, 1), duration: 1100, delay: 620 },
        activate: { keyframes: burst, duration: 600 },
    },
}
</script>

<style scoped>
.create-plus {
    transform-origin: 12px 12px;
}
</style>
