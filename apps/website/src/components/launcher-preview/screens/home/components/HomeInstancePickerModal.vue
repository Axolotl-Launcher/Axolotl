<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { nextTick, ref } from 'vue'

import type { GameInstance } from '../../../lib/types'
import PreviewModal from '../../../overlays/PreviewModal.vue'
import { homeMessages } from '../messages'
import InstancePickerList from './InstancePickerList.vue'

defineProps<{
    instances: GameInstance[]
    selectedInstanceId?: string | null
}>()

const emit = defineEmits<{
    select: [instance: GameInstance]
}>()

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof PreviewModal>>()
const instancePicker = ref<InstanceType<typeof InstancePickerList>>()
const messages = homeMessages

function show() {
    instancePicker.value?.reset()
    modal.value?.show()
    void nextTick(() => instancePicker.value?.focus())
}

function selectInstance(instance: GameInstance) {
    emit('select', instance)
    modal.value?.hide()
}

defineExpose({ show })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.minimalPickerTitle)"
        max-width="560px"
        width="min(560px, calc(100% - 2rem))"
        scrollable
        max-content-height="min(36rem, 560px)"
    >
        <div class="flex min-w-0 flex-col gap-4">
            <InstancePickerList
                ref="instancePicker"
                :instances="instances"
                :search-placeholder="formatMessage(messages.minimalPickerSearch)"
                :no-instances-message="formatMessage(messages.minimalPickerNoInstances)"
                :no-matches-message="formatMessage(messages.minimalPickerNoResults)"
                :select-label="
                    (instance) =>
                        formatMessage(messages.minimalPickerSelect, { name: instance.name })
                "
                @select="selectInstance"
            >
                <template #action="{ instance }">
                    <CheckIcon
                        v-if="instance.id === selectedInstanceId"
                        class="size-5 shrink-0 text-brand"
                        aria-hidden="true"
                    />
                </template>
            </InstancePickerList>
        </div>
    </PreviewModal>
</template>
