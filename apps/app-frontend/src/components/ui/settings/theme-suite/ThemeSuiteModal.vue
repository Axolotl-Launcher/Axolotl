<script setup lang="ts">
import { MultiStageModal } from '@modrinth/ui'
import { shallowRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { createThemeSuiteContext, provideThemeSuiteContext } from './context'
import { themeSuiteMessages } from './messages'
import { themeSuiteStages } from './stages'

const modal = shallowRef<ComponentExposed<typeof MultiStageModal> | null>(null)
const ctx = provideThemeSuiteContext(createThemeSuiteContext(modal))

defineExpose({ show: ctx.show })
</script>

<template>
    <MultiStageModal
        ref="modal"
        :stages="themeSuiteStages"
        :context="ctx"
        disable-progress
        :cancel-button="{
            label: ctx.formatMessage(themeSuiteMessages.close),
            onClick: () => modal?.hide(),
        }"
        @hide="ctx.close"
    >
    </MultiStageModal>
</template>
