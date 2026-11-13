<script setup lang="ts">
import { computed, watch } from 'vue'

import { useNetworkStatus } from '@/composables/useNetworkStatus'

import NetworkOffline from './NetworkOffline.vue'

const props = defineProps<{ requiresNetwork: boolean }>()
const emit = defineEmits<{ blocked: [] }>()
const { offline } = useNetworkStatus()
const blocked = computed(() => props.requiresNetwork && offline.value)
watch(
    blocked,
    (value) => {
        if (value) emit('blocked')
    },
    { immediate: true },
)
</script>

<template>
    <NetworkOffline v-if="blocked" />
    <slot v-else />
</template>
