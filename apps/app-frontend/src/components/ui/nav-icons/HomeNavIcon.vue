<template>
    <NavIconSvg :motion="motion">
        <g class="home-house">
            <path class="home-roof" d="M3 9 12 2l9 7" />
            <path d="M21 9v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V9" />
            <polyline class="home-door" points="9 22 9 12 15 12 15 22" pathLength="1" />
        </g>
    </NavIconSvg>
</template>

<script setup lang="ts">
import { bob, frames, type NavIconMotion, redraw } from './motion'
import NavIconSvg from './NavIconSvg.vue'

/** The house leaps straight away, then lands with a squash. */
const jump = frames(
    'transform',
    [
        [0, 'translateY(0) scale(1, 1)'],
        [0.14, 'translateY(-4px) scale(0.92, 1.1)'],
        [0.3, 'translateY(-5px) scale(1, 1)', 'ease-in'],
        [0.48, 'translateY(0) scale(1.14, 0.84)'],
        [0.64, 'translateY(0) scale(0.95, 1.06)'],
        [0.8, 'translateY(0) scale(1.02, 0.98)'],
        [1, 'translateY(0) scale(1, 1)'],
    ],
    'ease-out',
)

/** The roof tips from its right eave, like lifting a hat. */
const tip = frames(
    'transform',
    [
        [0, 'translateY(0) rotate(0deg)'],
        [0.14, 'translateY(-2px) rotate(12deg)'],
        [0.3, 'translateY(-2.4px) rotate(14deg)', 'ease-in'],
        [0.48, 'translateY(0.5px) rotate(-3deg)'],
        [0.64, 'translateY(0) rotate(2.5deg)'],
        [0.8, 'translateY(0) rotate(-1deg)'],
        [1, 'translateY(0) rotate(0deg)'],
    ],
    'ease-out',
)

const motion: NavIconMotion = {
    '.home-house': {
        enter: { keyframes: jump, duration: 720 },
        idle: { keyframes: bob((y) => `translateY(${y}px)`, -1.4), duration: 1100, delay: 640 },
        activate: { keyframes: jump, duration: 720 },
    },
    '.home-roof': {
        enter: { keyframes: tip, duration: 720 },
        idle: { keyframes: bob((angle) => `rotate(${angle}deg)`, 6), duration: 1100, delay: 900 },
        activate: { keyframes: tip, duration: 720 },
    },
    '.home-door': { enter: redraw(720) },
}
</script>

<style scoped>
.home-house {
    transform-origin: 12px 22px;
}

.home-roof {
    transform-origin: 21px 9px;
}
</style>
