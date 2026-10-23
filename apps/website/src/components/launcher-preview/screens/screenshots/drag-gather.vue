<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import { injectPreviewWindow } from '../../lib/window'
import ScreenshotDragPreview from './drag-preview.vue'
import { gatherDuration, type ScreenshotDragGatherItem } from './use-screenshot-drag-gather'

const props = defineProps<{
    items: ScreenshotDragGatherItem[]
    target: {
        x: number
        y: number
    }
    thumbnailFor: (item: ScreenshotDragGatherItem) => string
}>()

const emit = defineEmits<{
    complete: []
}>()

const { layer, toLocal } = injectPreviewWindow()
const gathered = ref(false)
const finishing = ref(false)
let animationFrame: number | undefined
let completionTimer: number | undefined

const localTarget = computed(() => toLocal(props.target.x, props.target.y))

const itemContainerStyles = computed(() =>
    props.items.map((_, index) => ({
        left: `${localTarget.value.x}px`,
        top: `${localTarget.value.y}px`,
        zIndex: `${index}`,
    })),
)

const itemStyles = computed(() =>
    props.items.map((item, index) => {
        const origin = toLocal(item.rect.left, item.rect.top)
        const originX = origin.x - localTarget.value.x
        const originY = origin.y - localTarget.value.y

        return {
            width: `${item.rect.width}px`,
            height: `${item.rect.height}px`,
            transform: gathered.value
                ? `translate3d(${-item.rect.width / 2}px, ${-item.rect.height / 2}px, 0)`
                : `translate3d(${originX}px, ${originY}px, 0)`,
            transitionDuration: `${gatherDuration}ms`,
            transitionDelay: `${Math.min(index, 8) * 10}ms`,
        }
    }),
)

onMounted(() => {
    animationFrame = requestAnimationFrame(() => {
        animationFrame = requestAnimationFrame(() => {
            gathered.value = true
            completionTimer = window.setTimeout(
                () => {
                    finishing.value = true
                    emit('complete')
                },
                Math.max(gatherDuration - 250, 0),
            )
        })
    })
})

onBeforeUnmount(() => {
    if (animationFrame !== undefined) {
        cancelAnimationFrame(animationFrame)
    }
    if (completionTimer !== undefined) {
        clearTimeout(completionTimer)
    }
})
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <div
            class="pointer-events-none fixed inset-0 z-[9998] overflow-hidden transition-opacity duration-150 ease-out"
            :class="finishing ? 'opacity-0' : 'opacity-100'"
            aria-hidden="true"
        >
            <div
                v-for="(item, index) in items"
                :key="item.selectionKey"
                class="fixed"
                :style="itemContainerStyles[index]"
            >
                <div
                    class="origin-center transition-transform ease-[cubic-bezier(0.2,0.8,0.2,1)]"
                    :style="itemStyles[index]"
                >
                    <ScreenshotDragPreview
                        :screenshot="item.screenshot"
                        :thumbnail="thumbnailFor(item)"
                    />
                </div>
            </div>
        </div>
    </Teleport>
</template>
