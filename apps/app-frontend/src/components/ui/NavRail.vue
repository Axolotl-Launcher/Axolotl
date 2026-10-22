<template>
    <div
        ref="rail"
        data-tooltip-group="navigation"
        class="nav-rail relative flex flex-col gap-[0.5rem]"
    >
        <slot />
        <div
            class="nav-rail-slider pointer-events-none absolute rounded-full"
            :class="[
                subpageSelected ? 'bg-surface-4' : 'bg-button-bgSelected',
                transitionsEnabled ? 'nav-rail-slider-transition' : '',
            ]"
            :style="sliderStyle"
            aria-hidden="true"
        />
    </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()

const rail = ref<HTMLElement | null>(null)

const hasActive = ref(false)
const subpageSelected = ref(false)
const sliderReady = ref(false)
const transitionsEnabled = ref(false)

const top = ref(0)
const bottom = ref(0)
const left = ref(0)
const width = ref(0)

const topDelay = ref('0ms')
const bottomDelay = ref('0ms')

const STAGGER_DELAY = '120ms'

const sliderStyle = computed(() => ({
    top: `${top.value}px`,
    bottom: `${bottom.value}px`,
    left: `${left.value}px`,
    width: `${width.value}px`,
    opacity: sliderReady.value && hasActive.value ? 1 : 0,
}))

function positionSlider() {
    const container = rail.value
    if (!container) return

    const el = container.querySelector<HTMLElement>('.router-link-active, .subpage-active')
    if (!el?.offsetParent || container.offsetHeight === 0) {
        hasActive.value = false
        return
    }

    subpageSelected.value = el.classList.contains('subpage-active')

    const newTop = el.offsetTop
    const newBottom = container.offsetHeight - el.offsetTop - el.offsetHeight
    const movingDown = newTop > top.value

    topDelay.value = movingDown ? STAGGER_DELAY : '0ms'
    bottomDelay.value = movingDown ? '0ms' : STAGGER_DELAY

    top.value = newTop
    bottom.value = newBottom
    left.value = el.offsetLeft
    width.value = el.offsetWidth
    hasActive.value = true

    if (!sliderReady.value) {
        sliderReady.value = true
        requestAnimationFrame(() => {
            transitionsEnabled.value = true
        })
    }
}

async function updateSlider() {
    await nextTick()
    positionSlider()
}

onMounted(updateSlider)

watch(() => [route.path, route.query], updateSlider)
</script>

<style scoped>
.nav-rail :deep(a),
.nav-rail :deep(button) {
    position: relative;
    z-index: 1;
}

.nav-rail :deep(a.router-link-active),
.nav-rail :deep(a.subpage-active),
.nav-rail :deep(button.router-link-active),
.nav-rail :deep(button.subpage-active) {
    background-color: transparent;
}

.nav-rail-slider {
    z-index: 0;
}

.nav-rail-slider-transition {
    transition:
        top 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(topDelay),
        bottom 150ms cubic-bezier(0.4, 0, 0.2, 1) v-bind(bottomDelay),
        left 150ms cubic-bezier(0.4, 0, 0.2, 1),
        width 150ms cubic-bezier(0.4, 0, 0.2, 1),
        opacity 250ms cubic-bezier(0.5, 0, 0.2, 1) 50ms;
}
</style>
