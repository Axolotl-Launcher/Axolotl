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

function book(order: number): { enter: NavIconClip; idle: NavIconClip; activate: NavIconClip } {
    const stagger = order * 60
    return {
        enter: { keyframes: hop, duration: 460, delay: stagger },
        idle: { keyframes: wave, duration: 1200, delay: 560 + order * 160 },
        activate: { keyframes: hop, duration: 460, delay: stagger },
    }
}

const motion: NavIconMotion = {
    '.library-book-1': book(0),
    '.library-book-2': book(1),
    '.library-book-3': book(2),
    '.library-lean': {
        enter: { keyframes: flop, duration: 900 },
        idle: { keyframes: bob((angle) => `rotate(${angle}deg)`, 6), duration: 1200, delay: 820 },
        activate: { keyframes: flop, duration: 900 },
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
