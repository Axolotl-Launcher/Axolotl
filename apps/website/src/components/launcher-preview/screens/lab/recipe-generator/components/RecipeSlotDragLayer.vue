<!-- 由 S4 集成到 LabRecipeGenerator.vue -->
<script setup lang="ts">
import { onUnmounted, ref } from 'vue'

import type { SlotDisplay } from '../display.ts'
import type { TextureAtlas } from '../resources.ts'
import type { SlotValue } from '../types.ts'
import RecipeItemIcon from './RecipeItemIcon.vue'

type DragFinish = (moved: boolean) => void

type StartDrag = (
    event: PointerEvent,
    value: SlotValue,
    display: SlotDisplay,
    atlas: TextureAtlas,
    onFinish?: DragFinish,
) => void

type ActiveDrag = {
    value: SlotValue
    display: SlotDisplay
    atlas: TextureAtlas
    pointerId: number
    startX: number
    startY: number
    onFinish?: DragFinish
}

defineSlots<{
    default: (props: { startDrag: StartDrag }) => unknown
}>()

const drag = ref<ActiveDrag | null>(null)
const ghostRef = ref<HTMLElement | null>(null)
let hoveredSlot: HTMLElement | null = null
let pointerX = 0
let pointerY = 0
let moved = false
let frame: number | null = null
let lastHitTestAt = 0

function startDrag(
    event: PointerEvent,
    value: SlotValue,
    display: SlotDisplay,
    atlas: TextureAtlas,
    onFinish?: DragFinish,
) {
    if (event.button !== 0 || drag.value) return
    drag.value = {
        value,
        display,
        atlas,
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        onFinish,
    }
    pointerX = event.clientX
    pointerY = event.clientY
    moved = false
    const target = event.currentTarget as HTMLElement | null
    try {
        target?.setPointerCapture(event.pointerId)
    } catch {
        // Pointer capture is optional; window listeners still track mouse drags.
    }
    window.addEventListener('pointermove', handlePointerMove, { passive: false })
    window.addEventListener('pointerup', handlePointerEnd)
    window.addEventListener('pointercancel', handlePointerCancel)
    scheduleFrame()
}

function handlePointerMove(event: PointerEvent) {
    const current = drag.value
    if (!current || current.pointerId !== event.pointerId) return
    event.preventDefault()
    pointerX = event.clientX
    pointerY = event.clientY
    if (!moved) {
        moved = Math.abs(pointerX - current.startX) > 4 || Math.abs(pointerY - current.startY) > 4
    }
    scheduleFrame()
}

function scheduleFrame() {
    if (frame !== null) return
    frame = window.requestAnimationFrame(() => {
        frame = null
        updateGhostPosition()
        updateHoveredSlot()
    })
}

function updateGhostPosition() {
    const ghost = ghostRef.value
    if (!ghost) return
    ghost.style.transform = `translate3d(${pointerX}px, ${pointerY}px, 0) translate(-50%, -50%)`
}

function updateHoveredSlot() {
    const now = performance.now()
    if (now - lastHitTestAt < 32) return
    lastHitTestAt = now
    const target = document.elementFromPoint(pointerX, pointerY)
    const next = target?.closest<HTMLElement>('[data-recipe-slot]') ?? null
    if (next === hoveredSlot) return
    hoveredSlot?.classList.remove('is-drag-target')
    next?.classList.add('is-drag-target')
    hoveredSlot = next
}

function handlePointerEnd(event: PointerEvent) {
    const current = drag.value
    if (!current || current.pointerId !== event.pointerId) return
    cleanup()
    if (moved) {
        event.preventDefault()
        const target = document.elementFromPoint(pointerX, pointerY)
        const slot = target?.closest<HTMLElement>('[data-recipe-slot]')
        if (slot) {
            slot.dispatchEvent(
                new CustomEvent('axolotl-recipe-slot-drop', {
                    detail: { value: current.value },
                    bubbles: true,
                }),
            )
        }
    }
    current.onFinish?.(moved)
    drag.value = null
}

function handlePointerCancel(event: PointerEvent) {
    const current = drag.value
    if (!current || current.pointerId !== event.pointerId) return
    cleanup()
    current.onFinish?.(false)
    drag.value = null
}

function cleanup() {
    if (frame !== null) {
        window.cancelAnimationFrame(frame)
        frame = null
    }
    window.removeEventListener('pointermove', handlePointerMove)
    window.removeEventListener('pointerup', handlePointerEnd)
    window.removeEventListener('pointercancel', handlePointerCancel)
    hoveredSlot?.classList.remove('is-drag-target')
    hoveredSlot = null
    lastHitTestAt = 0
}

onUnmounted(cleanup)
</script>

<template>
    <slot :start-drag="startDrag" />
    <Teleport to="body">
        <div v-if="drag" ref="ghostRef" class="recipe-slot-drag-ghost">
            <RecipeItemIcon
                :display="drag.display"
                :atlas="drag.atlas"
                :size="48"
                :show-count="false"
            />
        </div>
    </Teleport>
</template>

<style scoped>
.recipe-slot-drag-ghost {
    position: fixed;
    left: 0;
    top: 0;
    z-index: 10000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2px;
    border: 1px solid var(--color-brand);
    border-radius: var(--radius-sm);
    background: var(--color-surface-2);
    box-shadow: 0 0.5rem 1rem rgb(0 0 0 / 30%);
    pointer-events: none;
    opacity: 0.85;
    will-change: transform;
}
</style>
