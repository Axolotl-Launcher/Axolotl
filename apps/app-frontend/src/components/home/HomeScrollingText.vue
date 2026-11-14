<script setup lang="ts">
import { useElementSize, usePreferredReducedMotion, useTimeoutFn } from '@vueuse/core'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

const props = withDefaults(
    defineProps<{ text: string; active?: boolean; pauseAt?: 'start' | 'end' }>(),
    { active: true, pauseAt: 'start' },
)
const viewport = ref<HTMLElement>()
const track = ref<HTMLElement>()
const content = ref<HTMLElement>()
const { width: viewportWidth } = useElementSize(viewport)
const { width: contentWidth } = useElementSize(
    content,
    { width: 0, height: 0 },
    { box: 'border-box' },
)
const reducedMotion = usePreferredReducedMotion()
const overflows = computed(() => contentWidth.value - viewportWidth.value > 1)
const shouldScroll = computed(
    () => props.active && overflows.value && reducedMotion.value !== 'reduce',
)
let scrolling: Animation | undefined

function stopScrolling() {
    scrolling?.cancel()
    scrolling = undefined
}

function startScrolling() {
    if (!shouldScroll.value || !track.value || !content.value) return
    const nextCopy = track.value.children[1] as HTMLElement
    const distance = contentWidth.value - viewportWidth.value
    const cycle = nextCopy.offsetLeft - content.value.offsetLeft
    const travel = Math.max(1800, (distance / 30) * 1000)
    const pause = 3000
    const continuation = Math.max(1800, ((cycle - distance) / 30) * 1000)
    const duration =
        props.pauseAt === 'end'
            ? travel + pause + continuation
            : Math.max(1800, (cycle / 30) * 1000) + pause
    const easing = 'cubic-bezier(0.45, 0, 0.55, 1)'
    const frames: Keyframe[] =
        props.pauseAt === 'start'
            ? [
                  { transform: 'translateX(0)', offset: 0 },
                  { transform: 'translateX(0)', offset: pause / duration, easing },
                  { transform: `translateX(-${cycle}px)`, offset: 1 },
              ]
            : [
                  { transform: 'translateX(0)', offset: 0, easing },
                  { transform: `translateX(-${distance}px)`, offset: travel / duration },
                  {
                      transform: `translateX(-${distance}px)`,
                      offset: (travel + pause) / duration,
                      easing,
                  },
                  { transform: `translateX(-${cycle}px)`, offset: 1 },
              ]
    scrolling = track.value.animate(frames, { duration, iterations: Infinity })
}

// Wait for expanding containers to settle before measuring a new scroll cycle.
const { start: scheduleScrolling, stop: cancelScheduledScrolling } = useTimeoutFn(
    startScrolling,
    300,
    { immediate: false },
)
watch(
    [viewportWidth, contentWidth, () => props.text, () => props.pauseAt, shouldScroll],
    () => {
        cancelScheduledScrolling()
        stopScrolling()
        if (shouldScroll.value) scheduleScrolling()
    },
    { flush: 'post' },
)
onBeforeUnmount(() => {
    cancelScheduledScrolling()
    stopScrolling()
})
</script>

<template>
    <span
        ref="viewport"
        class="home-scrolling-text relative block min-w-0 flex-1 overflow-hidden"
        :class="{ 'is-scrolling': shouldScroll }"
        :title="text"
    >
        <span ref="track" class="relative flex w-max gap-8 whitespace-nowrap">
            <span ref="content" class="shrink-0" :class="{ 'px-2': shouldScroll }">{{ text }}</span>
            <span
                v-if="overflows"
                aria-hidden="true"
                class="shrink-0"
                :class="{ 'px-2': shouldScroll }"
                >{{ text }}</span
            >
        </span>
    </span>
</template>

<style scoped>
.home-scrolling-text.is-scrolling {
    mask-image: linear-gradient(
        to right,
        transparent,
        black 0.5rem,
        black calc(100% - 0.5rem),
        transparent
    );
}
</style>
