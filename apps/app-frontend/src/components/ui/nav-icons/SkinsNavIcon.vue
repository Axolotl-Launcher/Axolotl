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
import { bob, frames, type NavIconMotion } from './motion'
import NavIconSvg from './NavIconSvg.vue'

/**
 * The hanger is lifted off the rail and dropped back on: the shirt stretches as
 * it is yanked up, bunches at the top and stretches again as it lands.
 */
const lift = frames(
    'transform',
    [
        [0, 'translateY(0) scale(1, 1)'],
        [0.14, 'translateY(-8px) scale(0.95, 1.07)'],
        [0.3, 'translateY(-9.5px) scale(1.04, 0.94)', 'ease-in'],
        [0.48, 'translateY(0) scale(0.94, 1.09)'],
        [0.64, 'translateY(0) scale(1.03, 0.97)'],
        [0.8, 'translateY(0) scale(0.99, 1.01)'],
        [1, 'translateY(0) scale(1, 1)'],
    ],
    'ease-out',
)

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
        enter: { keyframes: lift, duration: 900 },
        // While the pointer stays, the hanger keeps bobbing gently on the rail.
        idle: { keyframes: bob((y) => `translateY(${y}px)`, -2.6), duration: 1600, delay: 760 },
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
