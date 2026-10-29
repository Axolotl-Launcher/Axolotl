<script setup lang="ts">
import { computed } from 'vue'

import GridDisplay from '@/components/GridDisplay.vue'
import { useLinkedServerProjects } from '@/composables/useLinkedServerProjects'
import type { GameInstance } from '@/helpers/types'

const props = defineProps<{ instances: GameInstance[] }>()
const instances = computed(() => props.instances)
const isServer = useLinkedServerProjects(instances)
const filteredInstances = computed(() =>
    instances.value.filter((instance) => instance.link && !isServer(instance)),
)
</script>
<template>
    <GridDisplay
        v-if="filteredInstances && filteredInstances.length > 0"
        label="Instances"
        :instances="filteredInstances"
    />
</template>
