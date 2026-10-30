<script setup lang="ts">
import { ref, watch } from 'vue'

import { loadMinecraftItemImage } from '@/lab/shared/minecraft/item-images.ts'

const props = withDefaults(
    defineProps<{
        item: string
        size?: number
    }>(),
    {
        size: 16,
    },
)

const source = ref<string | null>(null)

watch(
    () => props.item,
    (item) => {
        source.value = null
        loadMinecraftItemImage(item)
            .then((image) => {
                if (props.item === item) source.value = image.src
            })
            .catch(() => undefined)
    },
    { immediate: true },
)
</script>

<template>
    <img
        v-if="source"
        :src="source"
        alt=""
        class="shrink-0 [image-rendering:pixelated]"
        :style="{ width: `${size}px`, height: `${size}px` }"
    />
    <span
        v-else
        class="shrink-0 rounded-[2px] bg-surface-4"
        :style="{ width: `${size}px`, height: `${size}px` }"
        aria-hidden="true"
    ></span>
</template>
