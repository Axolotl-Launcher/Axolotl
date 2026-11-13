<template>
    <div class="flex flex-col relative">
        <div
            class="wrapper-wrapper"
            :class="{
                'top-fade': !scrollableAtTop && !disableScrolling,
                'bottom-fade': !scrollableAtBottom && !disableScrolling,
            }"
        >
            <div
                ref="scrollablePane"
                :class="{
                    'max-h-[19rem]': !disableScrolling,
                }"
                class="scrollable-pane"
                @scroll="onScroll"
            >
                <slot />
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'

withDefaults(
    defineProps<{
        disableScrolling?: boolean
    }>(),
    {
        disableScrolling: false,
    },
)

const scrollableAtTop = ref(true)
const scrollableAtBottom = ref(false)
const scrollablePane = ref<HTMLElement | null>(null)
let resizeObserver: ResizeObserver | undefined
onMounted(() => {
    resizeObserver = new ResizeObserver(function () {
        const pane = scrollablePane.value
        if (pane) {
            updateFade(pane.scrollTop, pane.offsetHeight, pane.scrollHeight)
        }
    })
    const pane = scrollablePane.value
    if (pane) {
        resizeObserver.observe(pane)
    }
})
onUnmounted(() => {
    if (resizeObserver) {
        resizeObserver.disconnect()
    }
})
function updateFade(scrollTop: number, offsetHeight: number, scrollHeight: number) {
    scrollableAtBottom.value = Math.ceil(scrollTop + offsetHeight) >= scrollHeight
    scrollableAtTop.value = scrollTop <= 0
}
function onScroll(event: Event) {
    const target = event.target
    if (!(target instanceof HTMLElement)) return
    updateFade(target.scrollTop, target.offsetHeight, target.scrollHeight)
}
</script>

<style lang="scss" scoped>
@property --_top-fade-height {
    syntax: '<length-percentage>';
    inherits: false;
    initial-value: 0%;
}

@property --_bottom-fade-height {
    syntax: '<length-percentage>';
    inherits: false;
    initial-value: 0%;
}

.wrapper-wrapper {
    flex-grow: 1;
    display: flex;
    overflow: hidden;
    position: relative;
    transition:
        --_top-fade-height 0.05s linear,
        --_bottom-fade-height 0.05s linear;

    --_fade-height: 3rem;

    mask-image: linear-gradient(
        transparent,
        rgb(0 0 0 / 100%) var(--_top-fade-height, 0%),
        rgb(0 0 0 / 100%) calc(100% - var(--_bottom-fade-height, 0%)),
        transparent 100%
    );

    &.top-fade {
        --_top-fade-height: var(--_fade-height);
    }

    &.bottom-fade {
        --_bottom-fade-height: var(--_fade-height);
    }
}
.scrollable-pane {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    height: 100%;
    width: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    position: relative;
}
</style>
