<template>
    <NavIconSvg :motion="motion">
        <defs>
            <clipPath :id="clipId">
                <circle cx="12" cy="12" r="10" />
            </clipPath>
        </defs>
        <g :clip-path="`url(#${clipId})`">
            <g class="worlds-tilt">
                <!--
                    The land repeats every globe-width to the left, so every spin and
                    loop cycle, however they overlap, ends where it began.
                -->
                <g class="worlds-land">
                    <g
                        v-for="offset in LAND_COPIES"
                        :key="offset"
                        :transform="`translate(${offset} 0)`"
                    >
                        <path d="M21.54 15H17a2 2 0 0 0-2 2v4.54" />
                        <path
                            d="M7 3.34V5a3 3 0 0 0 3 3a2 2 0 0 1 2 2c0 1.1.9 2 2 2a2 2 0 0 0 2-2c0-1.1.9-2 2-2h3.17"
                        />
                        <path d="M11 21.95V18a2 2 0 0 0-2-2a2 2 0 0 1-2-2v-1a2 2 0 0 0-2-2H2.05" />
                    </g>
                </g>
            </g>
        </g>
        <circle cx="12" cy="12" r="10" />
    </NavIconSvg>
</template>

<script setup lang="ts">
import { useId } from 'vue'

import { frames, type NavIconMotion } from './motion'
import NavIconSvg from './NavIconSvg.vue'

const GLOBE_WIDTH = 20

/**
 * Two overlapping entry spins (two globe-widths each) plus one loop cycle can
 * add up to five globe-widths of travel.
 */
const LAND_COPIES = Array.from({ length: 6 }, (_, index) => -index * GLOBE_WIDTH)

const clipId = `nav-icon-worlds-${useId()}`

/** The land whips round two full turns and coasts to a stop. */
const spin = frames('transform', [
    [0, 'translateX(0px)', 'cubic-bezier(0.2, 0.75, 0.25, 1)'],
    [1, `translateX(${2 * GLOBE_WIDTH}px)`],
])

/** One even turn per cycle, so the loop wraps onto the next copy without a seam. */
const turn = frames('transform', [
    [0, 'translateX(0px)', 'linear'],
    [1, `translateX(${GLOBE_WIDTH}px)`],
])

/** A click knocks the globe on its stand: its axis wobbles hard and settles. */
const wobble = frames('transform', [
    [0, 'rotate(0deg)', 'cubic-bezier(0.2, 0.8, 0.35, 1)'],
    [0.12, 'rotate(-28deg)'],
    [0.3, 'rotate(20deg)'],
    [0.48, 'rotate(-12deg)'],
    [0.64, 'rotate(6deg)'],
    [0.8, 'rotate(-2.5deg)'],
    [1, 'rotate(0deg)'],
])

const motion: NavIconMotion = {
    '.worlds-land': {
        enter: { keyframes: spin, duration: 1000 },
        idle: { keyframes: turn, duration: 2400, delay: 900 },
    },
    '.worlds-tilt': { activate: { keyframes: wobble, duration: 800 } },
}
</script>

<style scoped>
/* The axis wobble turns the land about the globe's centre, inside the rim. */
.worlds-tilt {
    transform-origin: 12px 12px;
}
</style>
