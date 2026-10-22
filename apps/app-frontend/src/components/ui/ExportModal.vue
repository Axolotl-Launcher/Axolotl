<script setup lang="ts">
import { commonMessages, MultiStageModal } from '@modrinth/ui'
import { shallowRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { createExportContext, provideExportContext } from './export/context'
import ExportProgress from './export/ExportProgress.vue'
import { exportStages } from './export/stages'

const props = defineProps<{ instance: { id: string; name: string } }>()
const modal = shallowRef<ComponentExposed<typeof MultiStageModal> | null>(null)
const ctx = provideExportContext(createExportContext(modal, () => props.instance))
defineExpose({ show: ctx.show })
</script>

<template>
    <MultiStageModal
        ref="modal"
        :stages="exportStages"
        :context="ctx"
        disable-progress
        :breadcrumbs="!ctx.exporting.value"
        :cancel-button="{
            label: ctx.formatMessage(commonMessages.cancelButton),
            disabled: ctx.exporting.value,
            onClick: () => modal?.hide(),
        }"
        @hide="ctx.close"
    >
        <template #header-extra><ExportProgress /></template>
    </MultiStageModal>
</template>
