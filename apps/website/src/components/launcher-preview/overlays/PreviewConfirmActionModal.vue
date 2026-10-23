<template>
    <PreviewModal
        ref="modal"
        :header="header"
        :fade="fade"
        :max-width="maxWidth"
        :disable-close="busy && !closing"
    >
        <div class="flex flex-col gap-6">
            <slot name="warning" />
            <slot />
            <p v-if="error" class="m-0 text-red">{{ error }}</p>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" :disabled="busy" @click="modal?.hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    v-tooltip="confirmDisabled ? confirmDisabledTooltip : undefined"
                    :type="confirmColor === 'standard' ? 'base' : 'colored'"
                    :color="confirmColor === 'standard' ? undefined : confirmColor"
                    :disabled="confirmDisabled || busy"
                    :loading="busy"
                    @click="onConfirm"
                >
                    <component :is="confirmIcon" v-if="confirmIcon" />
                    {{ confirmLabel }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>

<script setup lang="ts">
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { type Component, nextTick, ref } from 'vue'

import PreviewModal from './PreviewModal.vue'

/** The shared `ConfirmActionModal`, rendered inside the launcher window. */
const props = withDefaults(
    defineProps<{
        header: string
        fade?: 'standard' | 'danger' | 'warning'
        maxWidth?: string
        confirmColor?: 'red' | 'green' | 'orange' | 'brand' | 'standard'
        confirmIcon?: Component
        confirmLabel: string
        confirmDisabled?: boolean
        confirmDisabledTooltip?: string
        confirmAction?: () => void | Promise<void>
    }>(),
    {
        fade: 'standard',
        maxWidth: '500px',
        confirmColor: 'brand',
        confirmIcon: undefined,
        confirmDisabled: false,
        confirmDisabledTooltip: undefined,
        confirmAction: undefined,
    },
)

const emit = defineEmits<{ confirm: [] }>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
    failed: {
        id: 'modal.confirm-action.failed',
        defaultMessage: 'The action could not be completed. Try again.',
    },
})
const modal = ref<InstanceType<typeof PreviewModal>>()
const busy = ref(false)
const closing = ref(false)
const error = ref<string | null>(null)

function show(event?: MouseEvent) {
    if (busy.value) return
    error.value = null
    modal.value?.show(event)
}

async function onConfirm() {
    if (props.confirmDisabled || busy.value) return
    busy.value = true
    error.value = null
    try {
        if (props.confirmAction) await props.confirmAction()
        else emit('confirm')
        closing.value = true
        await nextTick()
        await modal.value?.hide()
    } catch {
        error.value = formatMessage(messages.failed)
    } finally {
        closing.value = false
        busy.value = false
    }
}

defineExpose({ show })
</script>
