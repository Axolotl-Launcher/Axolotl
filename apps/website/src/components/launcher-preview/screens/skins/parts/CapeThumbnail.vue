<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'

import { loadImage } from '../state'

/**
 * The 10×16 cape sheet of a cape texture, cut out and scaled up, in place of
 * the shared `CapeButton`'s CSS crop (that crop assumes a 64×32 cape sheet with
 * the same off-screen origin this project's textures do not have).
 */
const props = withDefaults(
    defineProps<{
        texture: string
        name?: string
        id: string
        selected?: boolean
        isEquipped?: boolean
        faded?: boolean
    }>(),
    { name: undefined, selected: undefined, isEquipped: false, faded: false },
)

const emit = defineEmits<{ (event: 'select'): void }>()

const src = ref<string | undefined>(undefined)
let disposed = false

const highlighted = computed(() => props.selected ?? props.isEquipped)

async function bake(texture: string) {
    if (!texture) {
        src.value = undefined
        return
    }
    try {
        const image = await loadImage(texture)
        if (disposed) return
        const canvas = document.createElement('canvas')
        canvas.width = 64
        canvas.height = 64
        const sheet = canvas.getContext('2d')
        if (!sheet) return
        sheet.imageSmoothingEnabled = false
        sheet.drawImage(image, 0, 0)

        const cape = document.createElement('canvas')
        cape.width = 10
        cape.height = 16
        const context = cape.getContext('2d')
        if (!context) return
        context.imageSmoothingEnabled = false
        context.drawImage(canvas, 1, 1, 10, 16, 0, 0, 10, 16)
        const data = context.getImageData(0, 0, 10, 16).data
        const hasPixels = Array.from({ length: 10 * 16 }, (_, index) => data[index * 4 + 3]).some(
            (alpha) => alpha !== 0,
        )
        src.value = hasPixels ? cape.toDataURL('image/png') : undefined
    } catch {
        src.value = undefined
    }
}

watch(
    () => props.texture,
    (texture) => {
        void bake(texture)
    },
    { immediate: true },
)

onUnmounted(() => {
    disposed = true
})
</script>

<template>
    <button
        v-tooltip="name"
        class="group m-0 block cursor-pointer border-0 bg-transparent p-0"
        :aria-label="name"
        @click="emit('select')"
    >
        <span
            :class="
                highlighted
                    ? 'highlighted-outer-glow bg-brand'
                    : 'bg-surface-4 brightness-95 group-hover:brightness-100'
            "
            class="relative block rounded-lg border-0 p-[3px] transition-all group-active:scale-95"
        >
            <span
                class="relative block overflow-hidden rounded-[5px]"
                :class="{ 'highlighted-inner-shadow': highlighted }"
                style="aspect-ratio: 10 / 16; width: 60px; min-height: 96px"
            >
                <img
                    v-if="src"
                    :src="src"
                    alt=""
                    class="absolute inset-0 h-full w-full object-cover"
                    :class="{ 'brightness-[0.3] contrast-[0.8]': faded }"
                    style="image-rendering: pixelated"
                />
                <span
                    v-else
                    class="absolute inset-0 flex items-center justify-center text-xs text-[var(--color-text-tertiary)]"
                >
                    {{ name }}
                </span>
            </span>
        </span>
    </button>
</template>

<style scoped>
.highlighted-inner-shadow::before {
    content: '';
    position: absolute;
    inset: 0;
    box-shadow: inset 0 0 4px 4px rgba(0, 0, 0, 0.4);
    z-index: 2;
}
</style>
