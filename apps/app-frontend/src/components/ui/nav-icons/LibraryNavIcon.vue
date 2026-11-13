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

/** ...and the leaning one snaps upright, then flops back into place. */
const flop = frames('transform', [
    [0, 'rotate(0deg)', 'cubic-bezier(0.2, 0.8, 0.35, 1)'],
    [0.22, 'rotate(16deg)'],
    [0.36, 'rotate(16deg)'],
    [0.52, 'rotate(-11deg)'],
    [0.66, 'rotate(5deg)'],
    [0.79, 'rotate(-2.2deg)'],
    [0.9, 'rotate(0.8deg)'],
    [1, 'rotate(0deg)'],
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

/** ...that jolts the books beside it... */
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

/** ...and knocks the leaning one further over for a moment. */
const knock = frames(
    'transform',
    [
        [0, 'rotate(0deg)'],
        [0.46, 'rotate(0deg)'],
        [0.58, 'rotate(-7deg)'],
        [0.76, 'rotate(3deg)'],
        [1, 'rotate(0deg)'],
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
        idle: { keyframes: bob((angle) => `rotate(${angle}deg)`, 6), duration: 1200, delay: 820 },
        activate: { keyframes: knock, duration: 650 },
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
