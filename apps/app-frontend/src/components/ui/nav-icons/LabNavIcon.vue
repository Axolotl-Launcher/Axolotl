<template>
    <NavIconSvg :motion="motion">
        <g class="lab-flask">
            <path
                d="M14 2v6a2 2 0 0 0 .245.96l5.51 10.08A2 2 0 0 1 18 22H6a2 2 0 0 1-1.755-2.96l5.51-10.08A2 2 0 0 0 10 8V2"
            />
            <path d="M6.453 15h11.094" />
            <path d="M8.5 2h7" />
            <!-- Entry bubbles and the hover simmer use separate circles, so they never share a part. -->
            <circle
                v-for="bubble in BUBBLES"
                :key="bubble.name"
                :class="['lab-bubble', bubble.name]"
                :cx="bubble.x"
                :cy="bubble.y"
                :r="bubble.r"
                fill="currentColor"
                stroke="none"
            />
        </g>
    </NavIconSvg>
</template>

<script setup lang="ts">
import { frames, type NavIconMotion, swing } from './motion'
import NavIconSvg from './NavIconSvg.vue'

const BUBBLES = [
    { name: 'lab-burst-1', x: 9.6, y: 19, r: 1.2 },
    { name: 'lab-burst-2', x: 14.4, y: 19.2, r: 1 },
    { name: 'lab-burst-3', x: 12, y: 18.6, r: 1.1 },
    { name: 'lab-puff', x: 12, y: -0.6, r: 1.3 },
    { name: 'lab-simmer-1', x: 11, y: 19.6, r: 0.9 },
    { name: 'lab-simmer-2', x: 13.6, y: 18.4, r: 0.8 },
    { name: 'lab-simmer-3', x: 8.4, y: 18.8, r: 0.85 },
    { name: 'lab-blast', x: 12, y: -1, r: 1.8 },
]

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

/**
 * A bubble grows, rises through the liquid and pops at its surface. It is
 * hidden at both ends but grows from nothing, so it runs alone on its circle.
 */
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

/** A click sets off a reaction: the flask is slammed flat, then jumps back up... */
const slam = frames(
    'transform',
    [
        [0, 'scale(1, 1)'],
        [0.14, 'scale(1.14, 0.8)'],
        [0.34, 'scale(0.92, 1.12)'],
        [0.54, 'scale(1.04, 0.96)'],
        [0.74, 'scale(0.99, 1.01)'],
        [1, 'scale(1, 1)'],
    ],
    'ease-out',
)

/** ...and a big cloud blows out of the neck. */
const blast: Keyframe[] = [
    { offset: 0, opacity: 0, transform: 'translateY(0) scale(0)', easing: 'ease-out' },
    { offset: 0.2, opacity: 1, transform: 'translateY(-2px) scale(1)', easing: 'ease-out' },
    { offset: 1, opacity: 0, transform: 'translateY(-7px) scale(2.1)' },
]

const motion: NavIconMotion = {
    '.lab-flask': {
        enter: { keyframes: shake, duration: 900 },
        idle: { keyframes: swing((angle) => `rotate(${angle}deg)`, 4), duration: 1600, delay: 760 },
        activate: { keyframes: slam, duration: 620 },
    },
    '.lab-blast': { activate: { keyframes: blast, duration: 650, delay: 90, single: true } },
    '.lab-burst-1': { enter: { keyframes: rise, duration: 520, delay: 40, single: true } },
    '.lab-burst-2': { enter: { keyframes: rise, duration: 520, delay: 130, single: true } },
    '.lab-burst-3': { enter: { keyframes: rise, duration: 520, delay: 220, single: true } },
    '.lab-puff': { enter: { keyframes: puff, duration: 600, delay: 160, single: true } },
    // While the pointer stays, the liquid keeps simmering.
    '.lab-simmer-1': { idle: { keyframes: rise, duration: 1000, delay: 760 } },
    '.lab-simmer-2': { idle: { keyframes: rise, duration: 1000, delay: 1090 } },
    '.lab-simmer-3': { idle: { keyframes: rise, duration: 1000, delay: 1420 } },
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
