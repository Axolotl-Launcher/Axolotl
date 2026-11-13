<template>
    <NavIconSvg :motion="motion">
        <path class="shots-back" d="M4 8a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2" />
        <g class="shots-front">
            <rect x="8" y="2" width="14" height="14" rx="2" />
            <path
                class="shots-hill"
                d="m22 11-1.296-1.296a2.4 2.4 0 0 0-3.408 0L11 16"
                pathLength="1"
            />
            <circle class="shots-sun" cx="13" cy="7" r="1" fill="currentColor" />
        </g>
    </NavIconSvg>
</template>

<script setup lang="ts">
import { bob, frames, type NavIconMotion, redraw } from './motion'
import NavIconSvg from './NavIconSvg.vue'

/** The top photo is picked up off the stack and set back down... */
const lift = frames(
    'transform',
    [
        [0, 'translateY(0) scale(1)'],
        [0.16, 'translateY(-3px) scale(1.06)'],
        [0.34, 'translateY(-3.2px) scale(1.06)', 'ease-in'],
        [0.52, 'translateY(0.4px) scale(0.97)'],
        [0.72, 'translateY(-0.3px) scale(1.01)'],
        [1, 'translateY(0) scale(1)'],
    ],
    'ease-out',
)

/** ...while the print behind it slides out from under it and back... */
const fan = frames(
    'transform',
    [
        [0, 'translate(0, 0)'],
        [0.18, 'translate(-2.8px, 2.8px)'],
        [0.4, 'translate(-2.8px, 2.8px)', 'ease-in-out'],
        [1, 'translate(0, 0)'],
    ],
    'ease-out',
)

/** ...and the sun flashes. */
const flash = frames(
    'transform',
    [
        [0, 'scale(1)'],
        [0.12, 'scale(0)'],
        [0.36, 'scale(2.6)'],
        [0.56, 'scale(0.8)'],
        [0.76, 'scale(1.15)'],
        [1, 'scale(1)'],
    ],
    'ease-out',
)

/** A click fires the shutter: the top photo punches straight out at you... */
const punch = frames(
    'transform',
    [
        [0, 'scale(1)'],
        [0.12, 'scale(0.82)'],
        [0.34, 'scale(1.22)'],
        [0.56, 'scale(0.94)'],
        [0.76, 'scale(1.04)'],
        [1, 'scale(1)'],
    ],
    'ease-out',
)

/** ...the print behind is knocked down and bounces back up... */
const knock = frames(
    'transform',
    [
        [0, 'translateY(0)'],
        [0.14, 'translateY(3.4px)'],
        [0.38, 'translateY(-0.9px)'],
        [0.6, 'translateY(0.3px)'],
        [1, 'translateY(0)'],
    ],
    'ease-out',
)

/** ...and the sun bursts. */
const burst = frames(
    'transform',
    [
        [0, 'scale(1)'],
        [0.3, 'scale(3.2)'],
        [0.6, 'scale(0.7)'],
        [1, 'scale(1)'],
    ],
    'ease-out',
)

const motion: NavIconMotion = {
    '.shots-front': {
        enter: { keyframes: lift, duration: 720 },
        // While the pointer stays, the two photos keep drifting apart and back together.
        idle: { keyframes: bob((y) => `translateY(${y}px)`, -1.2), duration: 1600, delay: 600 },
        activate: { keyframes: punch, duration: 560 },
    },
    '.shots-back': {
        enter: { keyframes: fan, duration: 720 },
        idle: { keyframes: bob((y) => `translateY(${y}px)`, 1), duration: 1600, delay: 600 },
        activate: { keyframes: knock, duration: 560 },
    },
    '.shots-sun': {
        enter: { keyframes: flash, duration: 720 },
        idle: { keyframes: bob((size) => `scale(${size})`, 1.6, 1), duration: 1600, delay: 900 },
        activate: { keyframes: burst, duration: 560 },
    },
    '.shots-hill': { enter: redraw(720) },
}
</script>

<style scoped>
.shots-back {
    transform-origin: 9px 15px;
}

.shots-front {
    transform-origin: 15px 9px;
}

.shots-sun {
    transform-origin: 13px 7px;
}
</style>
