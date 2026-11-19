<template>
    <NavIconSvg :motion="motion">
        <path class="library-book library-book-1" d="M4 4v16" />
        <path class="library-book library-book-2" d="M8 8v12" />
        <path class="library-book library-book-3" d="M12 6v14" />
        <path class="library-lean" d="m16 6 4 14" />
    </NavIconSvg>
</template>

<script setup lang="ts">
import { bob, frames, type NavIconClip, type NavIconMotion } from './motion'
import NavIconSvg from './NavIconSvg.vue'

/** The upright books jump off the shelf in a quick wave... */
const hop = frames(
    'transform',
    [
        [0, 'translateY(0) scaleY(1)'],
        [0.28, 'translateY(-4.5px) scaleY(1)', 'ease-in'],
        [0.56, 'translateY(0) scaleY(0.88)'],
        [0.76, 'translateY(-1px) scaleY(1)'],
        [1, 'translateY(0) scaleY(1)'],
    ],
    'ease-out',
)

/** ...and the leaning one snaps upright, then drops back against the shelf with a thud. */
const flop = frames('transform', [
    [0, 'rotate(0deg) scaleY(1)', 'cubic-bezier(0.2, 0.8, 0.35, 1)'],
    [0.22, 'rotate(16deg) scaleY(1)'],
    [0.38, 'rotate(16deg) scaleY(1)', 'ease-in'],
    [0.6, 'rotate(0deg) scaleY(0.9)', 'ease-out'],
    [0.8, 'rotate(0deg) scaleY(1.03)'],
    [1, 'rotate(0deg) scaleY(1)'],
])

/** While the pointer stays, the books keep a slow wave going. */
const wave = bob((y) => `translateY(${y}px)`, -1.6)

/** A click pulls the middle book right out of the shelf and drops it back with a thud... */
const pull = frames(
    'transform',
    [
        [0, 'translateY(0) scaleY(1)'],
        [0.28, 'translateY(-6px) scaleY(1)', 'ease-in'],
        [0.5, 'translateY(0.6px) scaleY(0.8)'],
        [0.7, 'translateY(-0.6px) scaleY(1.05)'],
        [1, 'translateY(0) scaleY(1)'],
    ],
    'ease-out',
)

/** ...that jolts the rest of the shelf. */
const jolt = frames(
    'transform',
    [
        [0, 'translateY(0) scaleY(1)'],
        [0.46, 'translateY(0) scaleY(1)'],
        [0.56, 'translateY(0.4px) scaleY(0.88)'],
        [0.76, 'translateY(0) scaleY(1.03)'],
        [1, 'translateY(0) scaleY(1)'],
    ],
    'ease-out',
)

function book(order: number, activate: NavIconClip): NavIconMotion[string] {
    return {
        enter: { keyframes: hop, duration: 460, delay: order * 60 },
        idle: { keyframes: wave, duration: 1200, delay: 560 + order * 160 },
        activate,
    }
}

const motion: NavIconMotion = {
    '.library-book-1': book(0, { keyframes: jolt, duration: 650 }),
    '.library-book-2': book(1, { keyframes: jolt, duration: 650 }),
    '.library-book-3': book(2, { keyframes: pull, duration: 650 }),
    '.library-lean': {
        enter: { keyframes: flop, duration: 900 },
        idle: { keyframes: wave, duration: 1200, delay: 560 + 3 * 160 },
        activate: { keyframes: jolt, duration: 650 },
    },
}
</script>

<style scoped>
/* Books squash against the shelf line. */
.library-book {
    transform-origin: 0 20px;
}

/* The leaning book pivots on its bottom corner. */
.library-lean {
    transform-origin: 20px 20px;
}
</style>
