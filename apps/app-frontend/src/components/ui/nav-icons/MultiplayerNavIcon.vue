<template>
    <NavIconSvg :motion="motion">
        <g class="mp-front">
            <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
            <circle cx="9" cy="7" r="4" />
        </g>
        <g class="mp-back">
            <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
            <path d="M16 3.13a4 4 0 0 1 0 7.75" />
        </g>
    </NavIconSvg>
</template>

<script setup lang="ts">
import { bob, frames, type NavIconMotion } from './motion'
import NavIconSvg from './NavIconSvg.vue'

/** A crouch, a jump and a squashed landing. */
const hop = frames(
    'transform',
    [
        [0, 'translateY(0) scale(1, 1)'],
        [0.14, 'translateY(-4.5px) scale(0.92, 1.1)'],
        [0.3, 'translateY(-5.5px) scale(1, 1)', 'ease-in'],
        [0.46, 'translateY(0) scale(1.14, 0.84)'],
        [0.62, 'translateY(0) scale(0.95, 1.06)'],
        [0.8, 'translateY(0) scale(1.02, 0.98)'],
        [1, 'translateY(0) scale(1, 1)'],
    ],
    'ease-out',
)

/** While the pointer stays, the two keep bobbing as if chatting, half a beat apart. */
const chat = bob((y) => `translateY(${y}px)`, -1.8)

/** A click makes the two crouch together and spring up tall, like a cheer. */
const cheer = frames(
    'transform',
    [
        [0, 'translateY(0) scale(1, 1)'],
        [0.12, 'translateY(0) scale(1.14, 0.8)'],
        [0.36, 'translateY(-1.6px) scale(0.88, 1.2)'],
        [0.58, 'translateY(0) scale(1.05, 0.94)'],
        [0.78, 'translateY(0) scale(0.98, 1.02)'],
        [1, 'translateY(0) scale(1, 1)'],
    ],
    'ease-out',
)

const motion: NavIconMotion = {
    '.mp-front': {
        enter: { keyframes: hop, duration: 700 },
        idle: { keyframes: chat, duration: 1100, delay: 620 },
        activate: { keyframes: cheer, duration: 600 },
    },
    '.mp-back': {
        enter: { keyframes: hop, duration: 700, delay: 110 },
        idle: { keyframes: chat, duration: 1100, delay: 1170 },
        activate: { keyframes: cheer, duration: 600, delay: 50 },
    },
}
</script>

<style scoped>
.mp-front {
    transform-origin: 9px 21px;
}

.mp-back {
    transform-origin: 20px 21px;
}
</style>
