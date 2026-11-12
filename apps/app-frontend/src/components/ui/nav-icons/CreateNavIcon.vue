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

/** Springy turns; the plus is symmetric, so they end where they started. */
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

const quarterTurn = frames(
    'transform',
    [
        [0, 'rotate(0deg)'],
        [0.5, 'rotate(105deg)'],
        [0.72, 'rotate(84deg)'],
        [0.88, 'rotate(93deg)'],
        [1, 'rotate(90deg)'],
    ],
    TURN_EASING,
)

const motion: NavIconMotion = {
    '.create-plus': {
        enter: { keyframes: halfTurn, duration: 700 },
        // While the pointer stays, the plus keeps pulsing as an invitation.
        idle: { keyframes: bob((size) => `scale(${size})`, 1.16, 1), duration: 1100, delay: 620 },
        activate: { keyframes: quarterTurn, duration: 600 },
    },
}
</script>

<style scoped>
.create-plus {
    transform-origin: 12px 12px;
}
</style>
