<script setup lang="ts">
import { Button, useVIntl } from '@modrinth/ui'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { type Chunk, generationBands, isGenerationChunk } from './core'
import type { Measurement } from './measurements'
import { strongholdMessages as messages } from './messages'
import { chunkGridStep, direction, fitView, type View, worldAt, zoomAt } from './plot'

const props = defineProps<{
    observations: Measurement[]
    heatmap: Float64Array
    best?: Chunk
    selected?: Chunk
    halfAngle?: number
    radius?: number
    targetOffset?: number
}>()
const emit = defineEmits<{ select: [chunk: Chunk] }>()
const { formatMessage } = useVIntl()
const canvas = ref<HTMLCanvasElement>()
const view = ref<View>({ x: 0, z: 0, span: 4096 })
const hovered = ref<{ x: number; z: number; weight?: number }>()
const colors = ['--color-blue', '--color-orange', '--color-green', '--color-purple', '--color-red']
const bands = generationBands()
let width = 1,
    height = 1
let resize: ResizeObserver | undefined
let theme: MutationObserver | undefined
let frame = 0
let drag: { id: number; x: number; y: number; view: View } | undefined
const weights = computed(() => {
    const values = new Map<string, number>()
    for (let i = 0; i < props.heatmap.length; i += 3)
        values.set(`${props.heatmap[i]}:${props.heatmap[i + 1]}`, props.heatmap[i + 2])
    return values
})
const hoverText = computed(() => {
    const c = hovered.value
    if (!c) return formatMessage(messages.plotHover)
    if (c.weight === undefined && isGenerationChunk(c.x, c.z))
        return formatMessage(messages.plotGenerationChunk, { x: c.x, z: c.z })
    if (c.weight === undefined) return formatMessage(messages.plotOutside, { x: c.x, z: c.z })
    const probability = c.weight.toFixed(3)
    return formatMessage(messages.plotChunk, { x: c.x, z: c.z, probability })
})

function fit() {
    view.value = fitView(props.observations, props.heatmap, width / height)
}
function fitGeneration() {
    view.value = {
        x: 0,
        z: 0,
        span: bands[bands.length - 1].outer * 2.1 * Math.max(1, width / height),
    }
}
function zoom(factor: number) {
    view.value = zoomAt(view.value, factor, view.value)
}
function schedule() {
    cancelAnimationFrame(frame)
    frame = requestAnimationFrame(draw)
}
function draw() {
    const el = canvas.value
    const ctx = el?.getContext('2d')
    if (!el || !ctx) return
    const palette = getComputedStyle(el)
    const color = (token: string) => palette.getPropertyValue(token).trim()
    const dpr = Math.min(devicePixelRatio || 1, 2)
    el.width = Math.round(width * dpr)
    el.height = Math.round(height * dpr)
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, height)
    const scale = width / view.value.span
    const sx = (x: number) => width / 2 + (x - view.value.x) * scale
    const sy = (z: number) => height / 2 + (z - view.value.z) * scale
    const bounds = worldAt(view.value, width, height, 0, 0)
    const right = worldAt(view.value, width, height, width, height)
    ctx.fillStyle = color('--surface-3')
    const chunkSize = 16 * scale
    let generationCells = 0
    if (chunkSize >= 6) {
        // At close range, mark individual chunks using the unchanged base prior.
        for (let x = Math.floor(bounds.x / 16); x <= Math.floor(right.x / 16); x++) {
            for (let z = Math.floor(bounds.z / 16); z <= Math.floor(right.z / 16); z++) {
                if (!isGenerationChunk(x, z)) continue
                ctx.fillRect(sx(x * 16), sy(z * 16), chunkSize, chunkSize)
                generationCells++
            }
        }
    } else {
        // Chunk cells are subpixel at overview scale; show enclosing reference bands.
        for (const band of bands) {
            ctx.beginPath()
            ctx.arc(sx(0), sy(0), band.outer * scale, 0, Math.PI * 2)
            ctx.arc(sx(0), sy(0), band.inner * scale, 0, Math.PI * 2, true)
            ctx.fill('evenodd')
        }
    }
    el.dataset.generationCells = String(generationCells)
    const peak = props.heatmap[2] || 1
    ctx.fillStyle = color('--color-brand')
    // Draw weak cells first, so subpixel cells cannot hide a stronger neighbour.
    for (let i = props.heatmap.length - 3; i >= 0; i -= 3) {
        const x = sx(props.heatmap[i] * 16),
            z = sy(props.heatmap[i + 1] * 16)
        const size = Math.max(1, 16 * scale)
        if (x + size < 0 || x > width || z + size < 0 || z > height) continue
        const relative = props.heatmap[i + 2] / peak
        ctx.globalAlpha = 0.12 + 0.78 * relative
        ctx.fillRect(x, z, size, size)
    }
    ctx.globalAlpha = 1
    const step = chunkGridStep(view.value.span)
    el.dataset.gridStep = String(step)
    ctx.lineWidth = 1
    // A single 16-block lattice; zoomed-out ticks are a subset of the same lattice.
    const gridStep = chunkSize >= 6 ? 16 : step
    ctx.strokeStyle = color('--surface-4')
    ctx.beginPath()
    for (let x = Math.ceil(bounds.x / gridStep) * gridStep; x < right.x; x += gridStep) {
        ctx.moveTo(sx(x), 0)
        ctx.lineTo(sx(x), height)
    }
    for (let z = Math.ceil(bounds.z / gridStep) * gridStep; z < right.z; z += gridStep) {
        ctx.moveTo(0, sy(z))
        ctx.lineTo(width, sy(z))
    }
    ctx.stroke()
    ctx.font = '11px monospace'
    ctx.textBaseline = 'top'
    ctx.fillStyle = color('--color-text-tertiary')
    for (let x = Math.ceil(bounds.x / step) * step; x < right.x; x += step) {
        if (sx(x) > 24 && sx(x) < width - 65) ctx.fillText(String(x), sx(x) + 4, height - 18)
    }
    for (let z = Math.ceil(bounds.z / step) * step; z < right.z; z += step) {
        if (sy(z) > 22 && sy(z) < height - 35) ctx.fillText(String(z), 4, sy(z) + 4)
    }
    props.observations.forEach((eye, i) => {
        const d = direction(eye.angle)
        const x = sx(eye.x),
            z = sy(eye.z)
        const length =
            Math.hypot(view.value.span, height / scale) +
            Math.hypot(eye.x - view.value.x, eye.z - view.value.z)
        ctx.strokeStyle = color(colors[i % colors.length])
        ctx.fillStyle = ctx.strokeStyle
        if (Number.isFinite(props.halfAngle) && props.halfAngle! > 0 && props.halfAngle! <= 90) {
            // Nested wedges approximate the linear angular fade without covering the heatmap.
            for (let band = 10; band >= 1; band--) {
                const half = (props.halfAngle! * band) / 10
                const left = direction(eye.angle - half),
                    right = direction(eye.angle + half)
                ctx.globalAlpha = 0.006
                ctx.beginPath()
                ctx.moveTo(x, z)
                ctx.lineTo(sx(eye.x + left.x * length), sy(eye.z + left.z * length))
                ctx.lineTo(sx(eye.x + right.x * length), sy(eye.z + right.z * length))
                ctx.closePath()
                ctx.fill()
            }
        }
        ctx.lineWidth = 1.5
        ctx.globalAlpha = 0.8
        ctx.beginPath()
        ctx.moveTo(x, z)
        ctx.lineTo(sx(eye.x + d.x * length), sy(eye.z + d.z * length))
        ctx.stroke()
        ctx.globalAlpha = 1
        ctx.beginPath()
        ctx.arc(x, z, 4, 0, Math.PI * 2)
        ctx.fill()
        // A short arrow remains visible even when most of the ray leaves the viewport.
        ctx.beginPath()
        ctx.moveTo(x + d.x * 22, z + d.z * 22)
        ctx.lineTo(x + d.x * 13 - d.z * 4, z + d.z * 13 + d.x * 4)
        ctx.lineTo(x + d.x * 13 + d.z * 4, z + d.z * 13 - d.x * 4)
        ctx.closePath()
        ctx.fill()
        ctx.font = 'bold 12px monospace'
        ctx.fillText(String(i + 1), x + 7, z - 16)
    })
    if (props.best) {
        const x = sx(props.best.x * 16 + 8),
            z = sy(props.best.z * 16 + 8)
        ctx.strokeStyle = color('--color-text-primary')
        ctx.lineWidth = 2
        ctx.beginPath()
        ctx.arc(x, z, 8, 0, Math.PI * 2)
        ctx.stroke()
        ctx.beginPath()
        ctx.moveTo(x - 12, z)
        ctx.lineTo(x + 12, z)
        ctx.moveTo(x, z - 12)
        ctx.lineTo(x, z + 12)
        ctx.stroke()
    }
    if (props.selected) {
        ctx.strokeStyle = color('--color-text-primary')
        ctx.lineWidth = 3
        ctx.strokeRect(
            sx(props.selected.x * 16) - 2,
            sy(props.selected.z * 16) - 2,
            Math.max(4, chunkSize) + 4,
            Math.max(4, chunkSize) + 4,
        )
    }
    ctx.font = 'bold 12px monospace'
    ctx.fillStyle = color('--color-text-primary')
    ctx.fillText('X →', width - 36, height - 36)
    ctx.fillText('Z ↓', 8, 8)
}
function position(event: MouseEvent) {
    const rect = canvas.value!.getBoundingClientRect()
    return {
        x: event.clientX - rect.left - canvas.value!.clientLeft,
        y: event.clientY - rect.top - canvas.value!.clientTop,
    }
}
function pointerDown(event: PointerEvent) {
    if (event.button !== 0) return
    const p = position(event)
    drag = { id: event.pointerId, ...p, view: { ...view.value } }
    canvas.value?.setPointerCapture(event.pointerId)
}
function pointerMove(event: PointerEvent) {
    const p = position(event)
    if (drag && drag.id === event.pointerId) {
        view.value = {
            ...drag.view,
            x: drag.view.x - ((p.x - drag.x) * drag.view.span) / width,
            z: drag.view.z - ((p.y - drag.y) * drag.view.span) / width,
        }
    }
    const world = worldAt(view.value, width, height, p.x, p.y)
    const x = Math.floor(world.x / 16),
        z = Math.floor(world.z / 16)
    hovered.value = { x, z, weight: weights.value.get(`${x}:${z}`) }
}
function pointerEnd(event: PointerEvent) {
    if (drag?.id !== event.pointerId) return
    const p = position(event)
    if (event.type === 'pointerup' && Math.hypot(p.x - drag.x, p.y - drag.y) < 5) {
        const world = worldAt(view.value, width, height, p.x, p.y)
        const x = Math.floor(world.x / 16),
            z = Math.floor(world.z / 16)
        const offsetX = x * 16 + (props.targetOffset ?? 0),
            offsetZ = z * 16 + (props.targetOffset ?? 0)
        if (isGenerationChunk(x, z) && Math.hypot(offsetX, offsetZ) <= (props.radius ?? 26000))
            emit('select', { x, z, weight: weights.value.get(`${x}:${z}`) ?? 0 })
    }
    drag = undefined
    if (canvas.value?.hasPointerCapture(event.pointerId))
        canvas.value.releasePointerCapture(event.pointerId)
}
function wheel(event: WheelEvent) {
    const p = position(event)
    view.value = zoomAt(
        view.value,
        Math.exp(Math.max(-1, Math.min(1, event.deltaY / 400))),
        worldAt(view.value, width, height, p.x, p.y),
    )
}
function keyboard(event: KeyboardEvent) {
    const delta = view.value.span / 10
    if (event.key === '+' || event.key === '=') zoom(0.7)
    else if (event.key === '-') zoom(1 / 0.7)
    else if (event.key === 'Home') fit()
    else if (event.key === 'ArrowLeft') view.value.x -= delta
    else if (event.key === 'ArrowRight') view.value.x += delta
    else if (event.key === 'ArrowUp') view.value.z -= delta
    else if (event.key === 'ArrowDown') view.value.z += delta
    else return
    event.preventDefault()
}
watch(
    () => [props.heatmap, props.observations],
    () => {
        hovered.value = undefined
        fit()
    },
)
watch(
    view,
    () => {
        hovered.value = undefined
        schedule()
    },
    { deep: true },
)
watch(() => [props.best, props.halfAngle], schedule)
watch(
    () => props.selected,
    (chunk) => {
        if (chunk)
            view.value = {
                x: chunk.x * 16 + 8,
                z: chunk.z * 16 + 8,
                span: Math.min(view.value.span, 2048),
            }
        schedule()
    },
)
onMounted(() => {
    resize = new ResizeObserver(() => {
        const initial = width === 1
        width = Math.max(1, canvas.value!.clientWidth)
        height = Math.max(1, canvas.value!.clientHeight)
        if (initial) fit()
        schedule()
    })
    resize.observe(canvas.value!)
    theme = new MutationObserver(schedule)
    theme.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['class', 'style'],
    })
    theme.observe(document.body, { attributes: true, attributeFilter: ['class', 'style'] })
})
onBeforeUnmount(() => {
    resize?.disconnect()
    theme?.disconnect()
    cancelAnimationFrame(frame)
})
</script>

<template>
    <section
        class="flex min-w-0 flex-col gap-3 rounded-xl border border-solid border-surface-4 bg-surface-2 p-4"
        data-testid="stronghold-plot"
    >
        <div class="flex flex-wrap items-center justify-between gap-3">
            <h2 class="m-0 text-lg font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.plotTitle) }}
            </h2>
            <div class="flex flex-wrap gap-2">
                <Button
                    size="sm"
                    :aria-label="formatMessage(messages.plotZoomIn)"
                    @click="zoom(0.7)"
                    >+</Button
                >
                <Button
                    size="sm"
                    :aria-label="formatMessage(messages.plotZoomOut)"
                    @click="zoom(1 / 0.7)"
                    >−</Button
                >
                <Button size="sm" @click="fit">{{ formatMessage(messages.plotFit) }}</Button>
                <Button size="sm" @click="fitGeneration">{{
                    formatMessage(messages.plotGenerationFit)
                }}</Button>
            </div>
        </div>
        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.plotHelp) }}
        </p>
        <canvas
            ref="canvas"
            class="h-[26rem] w-full touch-none cursor-grab rounded-lg border border-solid border-surface-4 bg-surface-1 outline-offset-2 active:cursor-grabbing"
            tabindex="0"
            role="img"
            :aria-label="
                formatMessage(messages.plotAccessible, {
                    count: observations.length,
                    candidates: heatmap.length / 3,
                })
            "
            :data-span="view.span"
            :data-center-x="view.x"
            :data-center-z="view.z"
            :data-candidates="heatmap.length / 3"
            @pointerdown="pointerDown"
            @pointermove="pointerMove"
            @pointerup="pointerEnd"
            @pointercancel="pointerEnd"
            @lostpointercapture="drag = undefined"
            @pointerleave="hovered = undefined"
            @wheel.prevent="wheel"
            @keydown="keyboard"
        />
        <div
            class="flex flex-wrap items-center gap-x-4 gap-y-2 text-xs text-[var(--color-text-tertiary)]"
        >
            <span class="flex items-center gap-2"
                ><span class="h-3 w-3 border border-solid border-surface-4 bg-surface-3" />{{
                    formatMessage(messages.plotGeneration)
                }}</span
            >
            <span class="flex items-center gap-2"
                ><span class="h-3 w-16 rounded bg-gradient-to-r from-transparent to-brand" />{{
                    formatMessage(messages.plotHeat)
                }}</span
            >
            <span>⊕ {{ formatMessage(messages.plotBest) }}</span>
            <span
                v-for="(_, index) in observations"
                :key="index"
                class="font-mono"
                :style="{ color: `var(${colors[index % colors.length]})` }"
                >{{ index + 1 }} ↗</span
            >
        </div>
        <p class="m-0 min-h-5 break-words font-mono text-xs tabular-nums" data-testid="plot-hover">
            {{ hoverText }}
        </p>
    </section>
</template>
