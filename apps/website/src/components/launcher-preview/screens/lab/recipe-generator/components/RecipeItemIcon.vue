<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { countFontSize, countInset, countShadow } from '../count-display.ts'
import type { SlotDisplay } from '../display.ts'
import type { TextureAtlas } from '../resources.ts'

const props = withDefaults(
    defineProps<{
        display: SlotDisplay | null
        atlas: TextureAtlas
        size?: number
        showCount?: boolean
    }>(),
    {
        size: 32,
        showCount: true,
    },
)

const canvasRef = ref<HTMLCanvasElement | null>(null)
const region = computed(() => {
    const texture = props.display?.texture
    return texture ? props.atlas.layout[texture] : undefined
})

const contentSize = computed(() => Math.max(1, props.size - 2))

const countStyle = computed(() => {
    if (!props.display?.count || props.display.count <= 1) return undefined
    const inset = countInset(props.size)
    return {
        fontSize: `${countFontSize(props.size)}px`,
        right: `${inset}px`,
        bottom: `${inset}px`,
        textShadow: countShadow(props.size),
    }
})

const imageCache = new Map<string, Promise<HTMLImageElement>>()

function loadImage(url: string): Promise<HTMLImageElement> {
    const cached = imageCache.get(url)
    if (cached) return cached
    const promise = new Promise<HTMLImageElement>((resolve, reject) => {
        const image = new Image()
        image.onload = () => resolve(image)
        image.onerror = () => reject(new Error(`Unable to load image: ${url}`))
        image.src = url
    })
    imageCache.set(url, promise)
    return promise
}

let drawToken = 0

async function drawIcon() {
    const canvas = canvasRef.value
    const display = props.display
    if (!canvas || !display?.texture) return
    const context = canvas.getContext('2d')
    if (!context) return
    const token = ++drawToken
    const size = contentSize.value
    if (canvas.width !== size) canvas.width = size
    if (canvas.height !== size) canvas.height = size
    context.clearRect(0, 0, size, size)
    context.imageSmoothingEnabled = false

    try {
        const currentRegion = region.value
        const image = await loadImage(currentRegion ? props.atlas.url : display.texture)
        if (token !== drawToken || canvas !== canvasRef.value) return
        const sourceX = currentRegion?.[0] ?? 0
        const sourceY = currentRegion?.[1] ?? 0
        const sourceWidth = currentRegion?.[2] ?? image.naturalWidth
        const sourceHeight = currentRegion?.[3] ?? image.naturalHeight
        if (!sourceWidth || !sourceHeight) return
        const scale = Math.min(size / sourceWidth, size / sourceHeight)
        const drawWidth = Math.max(1, Math.round(sourceWidth * scale))
        const drawHeight = Math.max(1, Math.round(sourceHeight * scale))
        const drawX = Math.round((size - drawWidth) / 2)
        const drawY = Math.round((size - drawHeight) / 2)
        context.drawImage(
            image,
            sourceX,
            sourceY,
            sourceWidth,
            sourceHeight,
            drawX,
            drawY,
            drawWidth,
            drawHeight,
        )
    } catch {
        // Missing textures render as an empty slot.
    }
}

watch(
    [canvasRef, () => props.display?.texture, () => props.atlas.url, () => props.size],
    drawIcon,
    { immediate: true },
)
</script>

<template>
    <div
        class="relative box-border inline-block flex-none overflow-hidden border border-surface-5"
        :style="{ width: `${size}px`, height: `${size}px` }"
        :title="display?.label"
    >
        <canvas
            v-if="display?.texture"
            ref="canvasRef"
            class="recipe-item-canvas"
            :width="contentSize"
            :height="contentSize"
        ></canvas>
        <span v-else class="recipe-item-empty" aria-hidden="true"></span>
        <span
            v-if="showCount && display?.count && display.count > 1"
            class="pointer-events-none absolute font-bold leading-none text-white"
            :style="countStyle"
            >{{ display.count }}</span
        >
    </div>
</template>

<style scoped>
.recipe-item-canvas {
    display: block;
    width: 100%;
    height: 100%;
}

.recipe-item-empty {
    display: block;
    width: 100%;
    height: 100%;
    background: repeating-conic-gradient(var(--surface-5) 0% 25%, var(--surface-3) 0% 50%);
    background-size: 8px 8px;
}
</style>
