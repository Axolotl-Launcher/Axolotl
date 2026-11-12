<template>
    <NavIconSvg :motion="motion">
        <g class="lab-flask">
            <path
                d="M14 2v6a2 2 0 0 0 .245.96l5.51 10.08A2 2 0 0 1 18 22H6a2 2 0 0 1-1.755-2.96l5.51-10.08A2 2 0 0 0 10 8V2"
            />
            <path d="M6.453 15h11.094" />
            <path d="M8.5 2h7" />
            <circle
                class="lab-bubble lab-bubble-1"
                cx="9.6"
                cy="19"
                r="1.2"
                fill="currentColor"
                stroke="none"
            />
            <circle
                class="lab-bubble lab-bubble-2"
                cx="14.4"
                cy="19.2"
                r="1"
                fill="currentColor"
                stroke="none"
            />
            <circle
                class="lab-bubble lab-bubble-3"
                cx="12"
                cy="18.6"
                r="1.1"
                fill="currentColor"
                stroke="none"
            />
            <circle
                class="lab-bubble lab-puff"
                cx="12"
                cy="-0.6"
                r="1.3"
                fill="currentColor"
                stroke="none"
            />
        </g>
    </NavIconSvg>
</template>

<script setup lang="ts">
import { frames, type NavIconClip, type NavIconMotion, swing } from './motion'
import NavIconSvg from './NavIconSvg.vue'

/** A hard shake that dies away. */
const shake = frames('transform', [
    [0, 'rotate(0deg)', 'cubic-bezier(0.2, 0.8, 0.35, 1)'],
    [0.1, 'rotate(-16deg)'],
    [0.25, 'rotate(13deg)'],
    [0.4, 'rotate(-9deg)'],
    [0.55, 'rotate(5.5deg)'],
    [0.7, 'rotate(-2.5deg)'],
    [0.85, 'rotate(1deg)'],
    [1, 'rotate(0deg)'],
])

/** A bubble rises through the liquid and pops at its surface; it is hidden at both ends. */
const rise: Keyframe[] = [
    { offset: 0, opacity: 0, transform: 'translateY(0) scale(0)', easing: 'ease-out' },
    { offset: 0.2, opacity: 1, transform: 'translateY(0) scale(1)', easing: 'ease-out' },
    { offset: 0.74, opacity: 1, transform: 'translateY(-3px) scale(1)', easing: 'ease-out' },
    { offset: 1, opacity: 0, transform: 'translateY(-3.6px) scale(1.8)' },
]

/** One puff escapes out of the neck. */
const puff: Keyframe[] = [
    { offset: 0, opacity: 0, transform: 'translateY(0) scale(0)', easing: 'ease-out' },
    { offset: 0.25, opacity: 1, transform: 'translateY(-1px) scale(1)', easing: 'ease-out' },
    { offset: 1, opacity: 0, transform: 'translateY(-4.5px) scale(1.6)' },
]

/** Entry bubbles, then a steady simmer while the pointer stays. */
function bubble(order: number): { enter: NavIconClip; idle: NavIconClip } {
    return {
        enter: { keyframes: rise, duration: 520, delay: 40 + order * 90 },
        idle: { keyframes: rise, duration: 1000, delay: 840 + order * 330 },
    }
}

const motion: NavIconMotion = {
    '.lab-flask': {
        enter: { keyframes: shake, duration: 900 },
        idle: { keyframes: swing((angle) => `rotate(${angle}deg)`, 4), duration: 1600, delay: 760 },
        activate: { keyframes: shake, duration: 900 },
    },
    '.lab-bubble-1': bubble(0),
    '.lab-bubble-2': bubble(1),
    '.lab-bubble-3': bubble(2),
    '.lab-puff': { enter: { keyframes: puff, duration: 600, delay: 160 } },
}
</script>

<style scoped>
/* The flask rocks on its base. */
.lab-flask {
    transform-origin: 12px 22px;
}

/* Bubbles are hidden at rest; their clips add opacity on top. */
.lab-bubble {
    opacity: 0;
    transform-box: fill-box;
    transform-origin: center;
}
</style>
