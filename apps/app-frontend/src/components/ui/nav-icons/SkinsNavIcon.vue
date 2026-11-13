<template>
    <NavIconSvg :motion="motion" view-box="0 0 49.915 52.72">
        <g class="skins-hanger">
            <!-- The source artwork is drawn facing right and mirrored for display. -->
            <g transform="matrix(-1 0 0 1 49.915 0)">
                <path
                    stroke-width="4.331"
                    d="M15.71 31.484v19.07h18.63v-19.07l6.538 6.539 6.871-6.872-11.203-11.733H14.122L2.166 31.375l6.827 6.827z"
                />
                <path stroke-width="3.993" d="M24.872 19.548v-6.44" />
                <path
                    stroke-width="4.331"
                    d="M24.704 13.202a5.518 5.518 0 0 1-5.518-5.518 5.518 5.518 0 0 1 5.518-5.518 5.518 5.518 0 0 1 5.518 5.518"
                />
            </g>
        </g>
    </NavIconSvg>
</template>

<script setup lang="ts">
import { frames, type NavIconMotion, swing } from './motion'
import NavIconSvg from './NavIconSvg.vue'

/** The shirt is flung sideways at once, then swings to rest like a damped pendulum. */
const fling = frames('transform', [
    [0, 'rotate(0deg)', 'cubic-bezier(0.2, 0.8, 0.35, 1)'],
    [0.12, 'rotate(26deg)'],
    [0.3, 'rotate(-19deg)'],
    [0.47, 'rotate(12deg)'],
    [0.63, 'rotate(-6.5deg)'],
    [0.77, 'rotate(3deg)'],
    [0.89, 'rotate(-1deg)'],
    [1, 'rotate(0deg)'],
])

/** A click turns the hanger right round on the rail: back of the shirt, then front again. */
const turnAround = frames('transform', [
    [0, 'scaleX(1)', 'ease-in'],
    [0.25, 'scaleX(0)', 'ease-out'],
    [0.5, 'scaleX(-1)', 'ease-in'],
    [0.75, 'scaleX(0)', 'ease-out'],
    [1, 'scaleX(1)'],
])

const motion: NavIconMotion = {
    '.skins-hanger': {
        enter: { keyframes: fling, duration: 1000 },
        idle: { keyframes: swing((angle) => `rotate(${angle}deg)`, 7), duration: 1800, delay: 760 },
        activate: { keyframes: turnAround, duration: 700 },
    },
}
</script>

<style scoped>
/* Top of the hook, after mirroring: where the hanger rests on the rail. */
.skins-hanger {
    transform-origin: 25.211px 2.166px;
}
</style>
