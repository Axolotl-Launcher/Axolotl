<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'

import type { GameInstance } from '../../../lib/types'
import { injectPreviewWindow } from '../../../lib/window'
import { useInstanceDraggable } from '../drag-drop'

/**
 * Port of `components/ui/library/DraggableInstanceCard.vue`. The card stays in
 * its grid slot (hidden) while a copy follows the pointer in the window's
 * overlay layer, which is how dnd-kit's feedback looks.
 */
const props = defineProps<{
    instance: GameInstance
    instanceGroupId: string
    disabled: boolean
}>()

const emit = defineEmits<{
    (e: 'contextmenu' | 'card-click', event: MouseEvent): void
    (e: 'start-long-press' | 'cancel-long-press'): void
}>()

const { layer } = injectPreviewWindow()
const cardElement = ref<HTMLElement>()
const dragDisabled = computed(() => props.disabled)

const { isDragging, feedback, onPointerDown } = useInstanceDraggable({
    id: computed(() => `instance:${props.instanceGroupId}:${props.instance.id}`),
    element: cardElement,
    disabled: dragDisabled,
    data: computed(() => ({
        instanceId: props.instance.id,
        fromGroup: props.instanceGroupId,
    })),
    onStart: () => emit('cancel-long-press'),
})

const feedbackStyle = computed(() => {
    const value = feedback.value
    if (!value) return undefined
    return {
        left: `${value.left}px`,
        top: `${value.top}px`,
        width: `${value.width}px`,
        height: `${value.height}px`,
        transform: `translate3d(${value.x}px, ${value.y}px, 0)`,
        transition: value.dropping ? 'transform 250ms ease' : 'none',
    }
})

let wantsClick = false

function onWindowPointerDown() {
    wantsClick = true
}

function onWindowPointerUp() {
    if (isDragging.value) wantsClick = false
}

function onWindowBlur() {
    wantsClick = false
}

onMounted(() => {
    window.addEventListener('pointerdown', onWindowPointerDown)
    window.addEventListener('pointerup', onWindowPointerUp)
    window.addEventListener('blur', onWindowBlur)
})

onUnmounted(() => {
    window.removeEventListener('pointerdown', onWindowPointerDown)
    window.removeEventListener('pointerup', onWindowPointerUp)
    window.removeEventListener('blur', onWindowBlur)
})

function onCardClick(event: MouseEvent) {
    if (!wantsClick || isDragging.value) return
    wantsClick = false
    emit('card-click', event)
}

function onContextMenu(event: MouseEvent) {
    emit('contextmenu', event)
}

function onMouseDown() {
    if (!dragDisabled.value) emit('start-long-press')
}

function onMouseUp() {
    emit('cancel-long-press')
}
</script>

<template>
    <div
        ref="cardElement"
        class="group relative w-full min-w-0"
        :class="{ invisible: isDragging }"
        @pointerdown="onPointerDown"
        @click="onCardClick"
        @mousedown="onMouseDown"
        @mouseup="onMouseUp"
        @mouseleave="onMouseUp"
        @touchstart="!dragDisabled && onMouseDown()"
        @touchend="onMouseUp"
        @touchcancel="onMouseUp"
        @contextmenu.prevent.stop="onContextMenu"
    >
        <slot :is-dragging="isDragging" />
        <slot v-if="!isDragging" name="overlay" />
    </div>
    <Teleport v-if="isDragging && feedbackStyle" :to="layer" :disabled="!layer">
        <div
            class="pointer-events-none fixed z-[2147483000] min-w-0 [&_*]:!pointer-events-none"
            :style="feedbackStyle"
            aria-hidden="true"
        >
            <div class="group relative w-full min-w-0">
                <slot :is-dragging="true" />
            </div>
        </div>
    </Teleport>
</template>
