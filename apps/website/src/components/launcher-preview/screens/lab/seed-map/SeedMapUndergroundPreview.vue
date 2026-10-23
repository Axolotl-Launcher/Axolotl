<script setup lang="ts">
import ArrowLeftIcon from '@modrinth/assets/icons/arrow-left.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

import type { SeedMapEdition } from './backend.ts'
import type { SeedMapDimension } from './biomes.ts'
import SeedMapPreviewNotice from './SeedMapPreviewNotice.vue'

/**
 * The 3D underground view of `components/lab/seed-map/SeedMapUndergroundPreview.vue`.
 * The desktop app generates its block volume natively, so the preview keeps the
 * view's frame and explains where to get it.
 */
defineProps<{
    query: {
        seed: string
        edition: SeedMapEdition
        version: string
        dimension: SeedMapDimension
        x: number
        y: number
        z: number
    }
}>()
const emit = defineEmits<{ close: [] }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    title: {
        id: 'app.lab.seed-map.underground-3d.title',
        defaultMessage: '3D underground · X-Ray',
    },
    back: { id: 'app.lab.seed-map.underground-3d.back', defaultMessage: 'Back to map' },
})
</script>

<template>
    <div
        class="absolute inset-0 z-30 flex min-h-0 flex-col bg-surface-1"
        data-onboarding-id="seed-map-underground-3d-view"
        @pointerdown.stop
        @wheel.stop
    >
        <div
            class="flex flex-wrap items-center gap-2 border-b border-surface-4 bg-surface-2 p-2"
            @keydown.stop
        >
            <Button size="2xs" type="quiet" @click="emit('close')"
                ><ArrowLeftIcon />{{ formatMessage(messages.back) }}</Button
            >
            <strong class="mr-auto text-sm">{{ formatMessage(messages.title) }}</strong>
        </div>
        <div class="flex min-h-0 flex-1 items-center justify-center p-6">
            <SeedMapPreviewNotice reason="underground" class="w-full max-w-md" />
        </div>
    </div>
</template>
