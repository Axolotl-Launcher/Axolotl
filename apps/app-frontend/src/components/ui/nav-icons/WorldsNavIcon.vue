<template>
    <NavIconSvg :motion="motion">
        <defs>
            <clipPath :id="clipId">
                <circle cx="12" cy="12" r="10" />
            </clipPath>
        </defs>
        <g :clip-path="`url(#${clipId})`">
            <!--
                The land repeats every globe-width to the left, enough for an entry
                spin, a click spin and a loop cycle at once to end where they began.
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
        <circle cx="12" cy="12" r="10" />
    </NavIconSvg>
</template>

<script setup lang="ts">
import { useId } from 'vue'

import { frames, type NavIconMotion } from './motion'
import NavIconSvg from './NavIconSvg.vue'

const GLOBE_WIDTH = 20
const LAND_COPIES = [0, -20, -40, -60, -80, -100]

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

const motion: NavIconMotion = {
    '.worlds-land': {
        enter: { keyframes: spin, duration: 1000 },
        idle: { keyframes: turn, duration: 2400, delay: 900 },
        activate: { keyframes: spin, duration: 900 },
    },
}
</script>
