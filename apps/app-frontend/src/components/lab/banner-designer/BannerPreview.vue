<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import { BANNER_FACE_HEIGHT, BANNER_FACE_WIDTH } from '@/lab/banner-designer/masks.ts'
import { renderBannerPixels } from '@/lab/banner-designer/render.ts'
import type { BannerDesign } from '@/lab/banner-designer/types.ts'

const props = withDefaults(
    defineProps<{
        design: BannerDesign
        /** Integer upscale factor for the 20 × 40 face texture. */
        scale?: number
        label?: string
    }>(),
    {
        scale: 8,
        label: '',
    },
)

const canvas = ref<HTMLCanvasElement | null>(null)

const upscale = computed(() => Math.max(1, Math.round(props.scale)))

/**
 * The bitmap keeps the texture's own 20 × 40 size and the browser scales it up
 * with nearest-neighbour sampling. Resampling the bitmap itself would land on
 * fractional device pixels at non-integer device pixel ratios, which shifts the
 * pattern by a pixel.
 */
const boxStyle = computed(() => ({
    width: `${BANNER_FACE_WIDTH * upscale.value}px`,
    height: `${BANNER_FACE_HEIGHT * upscale.value}px`,
}))

function draw() {
    const element = canvas.value
    if (!element) return
    const context = element.getContext('2d')
    if (!context) return
    if (element.width !== BANNER_FACE_WIDTH) element.width = BANNER_FACE_WIDTH
    if (element.height !== BANNER_FACE_HEIGHT) element.height = BANNER_FACE_HEIGHT
    context.putImageData(
        new ImageData(renderBannerPixels(props.design), BANNER_FACE_WIDTH, BANNER_FACE_HEIGHT),
        0,
        0,
    )
}

onMounted(draw)
watch(() => props.design, draw, { deep: true })
</script>

<template>
    <canvas
        ref="canvas"
        class="block shrink-0 [image-rendering:pixelated]"
        role="img"
        :aria-label="label || undefined"
        :style="boxStyle"
    ></canvas>
</template>
