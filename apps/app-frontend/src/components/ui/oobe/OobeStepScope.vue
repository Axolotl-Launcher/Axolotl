<script setup lang="ts">
import { provide } from 'vue'

import { oobeStepContextKey } from './context'

const props = defineProps<{
    stepId: string
    controls: {
        setBusy: (stepId: string, busy: boolean) => void
        setReady: (stepId: string, ready: boolean) => void
        markChanged: (stepId: string) => void
        next: (stepId: string) => void
    }
}>()

// Bound to one step, so a step still leaving the screen cannot hold the next one.
provide(oobeStepContextKey, {
    setBusy: (busy) => props.controls.setBusy(props.stepId, busy),
    setReady: (ready) => props.controls.setReady(props.stepId, ready),
    markChanged: () => props.controls.markChanged(props.stepId),
    next: () => props.controls.next(props.stepId),
})
</script>

<template>
    <slot />
</template>
