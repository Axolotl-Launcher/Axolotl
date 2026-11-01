<script setup lang="ts">
import { computed } from 'vue'

import LabPluginBoundary from '@/components/lab/plugins/LabPluginBoundary.vue'
import { labPluginRegistrations } from '@/lab/plugins/registry'
import { collectSlotContributions } from '@/lab/plugins/slots'
import { isLabPluginEnabled } from '@/lab/plugins/state'
import type { LabPluginSlotName } from '@/lab/plugins/types'

const props = defineProps<{
    /** Slot to fill. Every contribution registered for it renders in order. */
    name: LabPluginSlotName
    /** Props passed to each contribution, matching the slot's context type. */
    context?: Record<string, unknown>
}>()

const contributions = computed(() =>
    collectSlotContributions(labPluginRegistrations, props.name, isLabPluginEnabled),
)
</script>

<template>
    <LabPluginBoundary
        v-for="(entry, index) in contributions"
        :key="`${entry.plugin.manifest.id}-${index}`"
        :plugin-id="entry.plugin.manifest.id"
        :component="entry.contribution.component"
        :component-props="context"
        variant="inline"
    />
</template>
