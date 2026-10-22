<template>
    <NewModal
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
            <div class="flex gap-2 justify-end">
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
                    <component :is="confirmIcon" />
                    {{ confirmLabel }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import { XIcon } from '@modrinth/assets'
import { nextTick, ref } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { commonMessages } from '../../utils/common-messages'
import Button from '../base/buttons/Button.vue'
import NewModal from './NewModal.vue'

const props = withDefaults(
    defineProps<{
        header: string
        fade?: 'standard' | 'danger' | 'warning'
        maxWidth?: string
        confirmColor?: 'red' | 'green' | 'orange' | 'brand' | 'standard'
        confirmIcon?: object
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
    },
)

const emit = defineEmits<{
    (e: 'confirm'): void
}>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
    failed: {
        id: 'modal.confirm-action.failed',
        defaultMessage: 'The action could not be completed. Try again.',
    },
})
const modal = ref<InstanceType<typeof NewModal>>()
const busy = ref(false)
const closing = ref(false)
const error = ref<string | null>(null)

function show() {
    if (busy.value) return
    error.value = null
    modal.value?.show()
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
    } catch (failure) {
        console.error('Confirmation action failed', failure)
        error.value = formatMessage(messages.failed)
    } finally {
        closing.value = false
        busy.value = false
    }
}

defineExpose({ show })
</script>
