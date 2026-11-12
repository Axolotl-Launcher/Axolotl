<template>
    <svg
        ref="root"
        xmlns="http://www.w3.org/2000/svg"
        width="24"
        height="24"
        :viewBox="viewBox"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
        class="overflow-visible"
    >
        <slot />
    </svg>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'

import { injectNavIconInteraction } from './interaction'
import {
    frames,
    NAV_ICON_ANIMATION_PREFIX,
    type NavIconClip,
    type NavIconMotion,
    type NavIconPartMotion,
} from './motion'

const props = withDefaults(defineProps<{ motion: NavIconMotion; viewBox?: string }>(), {
    viewBox: '0 0 24 24',
})

/** The whole glyph pops the moment the pointer arrives... */
const POP: NavIconClip = {
    duration: 480,
    keyframes: frames(
        'transform',
        [
            [0, 'scale(1)'],
            [0.16, 'scale(1.18)'],
            [0.42, 'scale(0.93)'],
            [0.68, 'scale(1.04)'],
            [1, 'scale(1)'],
        ],
        'ease-out',
    ),
}

/** ...and squashes, then springs back, when it is pressed. */
const PRESS: NavIconClip = {
    duration: 460,
    keyframes: frames(
        'transform',
        [
            [0, 'scale(1)'],
            [0.16, 'scale(0.78)'],
            [0.48, 'scale(1.14)'],
            [0.72, 'scale(0.97)'],
            [1, 'scale(1)'],
        ],
        'ease-out',
    ),
}

/**
 * Once the pointer leaves, everything still moving is back at rest within
 * `WIND_DOWN_MAX_MS`: a loop finishes its current cycle at least
 * `WIND_DOWN_RATE` times faster, and longer one-shot clips speed up as needed.
 */
const WIND_DOWN_RATE = 2.5
const WIND_DOWN_MAX_MS = 500

/**
 * How many clips of one kind may overlap on a part, so quick repeated clicks or
 * entries each add visible motion without piling up without bound.
 */
const MAX_OVERLAP = 2

type AccentKind = 'enter' | 'activate' | 'press'

const root = ref<SVGSVGElement | null>(null)
const interaction = injectNavIconInteraction()
const reduceMotionQuery =
    typeof window.matchMedia === 'function'
        ? window.matchMedia('(prefers-reduced-motion: reduce)')
        : null

const accents = new Map<Element, Map<AccentKind, Animation[]>>()
const loops = new Map<Element, Animation>()

function isLive(animation: Animation | undefined): animation is Animation {
    return !!animation && animation.playState !== 'finished' && animation.playState !== 'idle'
}

function start(element: Element, kind: string, clip: NavIconClip, iterations = 1): Animation {
    return element.animate(clip.keyframes, {
        id: `${NAV_ICON_ANIMATION_PREFIX}${kind}`,
        duration: clip.duration,
        delay: clip.delay ?? 0,
        iterations,
        composite: clip.composite ?? 'add',
    })
}

function accent(element: Element, kind: AccentKind, clip: NavIconClip) {
    const kinds = accents.get(element) ?? new Map<AccentKind, Animation[]>()
    accents.set(element, kinds)
    const live = (kinds.get(kind) ?? []).filter(isLive)
    // Added clips stack seamlessly; a replacing or single one would jump if layered.
    const room = (clip.composite ?? 'add') === 'add' && !clip.single ? MAX_OVERLAP : 1
    kinds.set(kind, live.length < room ? [...live, start(element, kind, clip)] : live)
}

function loop(element: Element, clip: NavIconClip) {
    const current = loops.get(element)
    if (isLive(current)) {
        // The pointer came back while the loop was winding down: keep it going.
        current.effect?.updateTiming({ iterations: Infinity })
        current.updatePlaybackRate(1)
        return
    }
    loops.set(element, start(element, 'idle', clip, Infinity))
}

/** Brings everything still moving back to its resting pose quickly, without a jump. */
function windDown() {
    for (const [element, animation] of loops) {
        const timing = animation.effect?.getComputedTiming()
        const iteration = timing?.currentIteration
        if (timing == null || iteration == null) {
            // Still waiting for its delay, so nothing of it is on screen yet.
            animation.cancel()
            loops.delete(element)
            continue
        }
        const duration = Number(timing.duration)
        const intoCycle = Number(timing.localTime) - Number(timing.delay) - iteration * duration
        const remaining = Math.max(0, duration - intoCycle)
        animation.effect?.updateTiming({ iterations: iteration + 1 })
        animation.updatePlaybackRate(Math.max(WIND_DOWN_RATE, remaining / WIND_DOWN_MAX_MS))
    }
    for (const kinds of accents.values()) {
        for (const animation of [...kinds.values()].flat().filter(isLive)) {
            const timing = animation.effect?.getComputedTiming()
            if (!timing) continue
            const remaining = Number(timing.endTime) - Number(timing.localTime)
            const rate = remaining / WIND_DOWN_MAX_MS
            if (rate > animation.playbackRate) animation.updatePlaybackRate(rate)
        }
    }
}

/** Drops every animation at once, for when motion is no longer wanted. */
function stopAll() {
    for (const animation of loops.values()) animation.cancel()
    loops.clear()
    for (const kinds of accents.values()) {
        for (const animation of [...kinds.values()].flat()) animation.cancel()
    }
    accents.clear()
}

function parts(): [Element, NavIconPartMotion][] {
    const svg = root.value
    if (!svg) return []
    return Object.entries(props.motion).flatMap(([selector, motion]) =>
        [...svg.querySelectorAll(selector)].map((element): [Element, NavIconPartMotion] => [
            element,
            motion,
        ]),
    )
}

function enter() {
    if (!root.value || reduceMotionQuery?.matches) return
    accent(root.value, 'enter', POP)
    for (const [element, motion] of parts()) {
        if (motion.enter) accent(element, 'enter', motion.enter)
        if (motion.idle) loop(element, motion.idle)
    }
}

function press() {
    if (root.value && !reduceMotionQuery?.matches) accent(root.value, 'press', PRESS)
}

function activate() {
    if (!root.value || reduceMotionQuery?.matches) return
    for (const [element, motion] of parts()) {
        if (motion.activate) accent(element, 'activate', motion.activate)
    }
}

function onReduceMotionChange(event: MediaQueryListEvent) {
    if (event.matches) stopAll()
}

if (interaction) {
    watch(interaction.hovered, (hovered) => (hovered ? enter() : windDown()))
    watch(interaction.presses, press)
    watch(interaction.activations, activate)
    reduceMotionQuery?.addEventListener?.('change', onReduceMotionChange)
}

onBeforeUnmount(() => {
    reduceMotionQuery?.removeEventListener?.('change', onReduceMotionChange)
    stopAll()
})
</script>
