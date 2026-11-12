<template>
    <NavIconSvg :motion="motion">
        <circle class="settings-hub" cx="12" cy="12" r="3" />
        <path
            class="settings-gear"
            d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"
        />
    </NavIconSvg>
</template>

<script setup lang="ts">
import { bob, frames, type NavIconMotion } from './motion'
import NavIconSvg from './NavIconSvg.vue'

const TURN_EASING = 'cubic-bezier(0.3, 0.7, 0.4, 1)'

/** A full turn that starts at speed and ends with a mechanical overshoot. */
const fullTurn = frames(
    'transform',
    [
        [0, 'rotate(0deg)'],
        [0.48, 'rotate(400deg)'],
        [0.66, 'rotate(348deg)'],
        [0.82, 'rotate(366deg)'],
        [1, 'rotate(360deg)'],
    ],
    TURN_EASING,
)

/**
 * While the pointer stays, the gear keeps turning at an even pace. The outline
 * is not exactly symmetric, so each cycle is a whole turn.
 */
const turn = frames('transform', [
    [0, 'rotate(0deg)', 'linear'],
    [1, 'rotate(360deg)'],
])

const pulse = frames(
    'transform',
    [
        [0, 'scale(1)'],
        [0.12, 'scale(0.35)'],
        [0.38, 'scale(1.3)'],
        [0.58, 'scale(0.88)'],
        [0.78, 'scale(1.06)'],
        [1, 'scale(1)'],
    ],
    'ease-out',
)

const motion: NavIconMotion = {
    '.settings-gear': {
        enter: { keyframes: fullTurn, duration: 950 },
        idle: { keyframes: turn, duration: 3000, delay: 950 },
        activate: { keyframes: fullTurn, duration: 800 },
    },
    '.settings-hub': {
        enter: { keyframes: pulse, duration: 950 },
        idle: { keyframes: bob((size) => `scale(${size})`, 0.8, 1), duration: 900, delay: 950 },
        activate: { keyframes: pulse, duration: 800 },
    },
}
</script>

<style scoped>
.settings-hub,
.settings-gear {
    transform-origin: 12px 12px;
}
</style>
