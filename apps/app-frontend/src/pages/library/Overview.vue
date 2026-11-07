<script setup lang="ts">
import GridDisplay from '@/components/GridDisplay.vue'
import type { GameInstance } from '@/helpers/types'

defineProps<{
    instances: GameInstance[]
}>()
</script>
<template>
    <GridDisplay
        v-if="instances && instances.length > 0"
        label="Instances"
        :instances="instances"
    />
</template>
