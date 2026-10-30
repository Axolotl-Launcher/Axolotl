<script setup lang="ts">
import fastaiLogo from '@/assets/external/fastai.png'

withDefaults(defineProps<{ size?: number }>(), { size: undefined })
</script>

<template>
    <img
        :src="fastaiLogo"
        alt=""
        class="block flex-none object-contain"
        :style="size ? { height: `${size}px`, width: `${size}px` } : undefined"
        aria-hidden="true"
    />
</template>
