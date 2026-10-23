<template>
    <PreviewModal
        ref="modal"
        :fade="danger ? 'danger' : 'standard'"
        :on-hide="onHide"
        max-width="550px"
    >
        <template #title>
            <slot name="title">
                <span class="text-lg font-extrabold text-[var(--color-text-primary)]">{{
                    title
                }}</span>
            </slot>
        </template>
        <div class="flex flex-col gap-4">
            <p v-if="description" class="m-0 max-w-[35rem]">
                {{ description }}
            </p>
            <slot />
            <label v-if="hasToType" :for="inputId">
                <IntlFormatted
                    :message-id="messages.confirmationPrompt"
                    :values="{ confirmationText }"
                >
                    <template #confirmation-text="{ children }">
                        <span class="font-bold italic"><component :is="() => children" /></span>
                    </template>
                </IntlFormatted>
            </label>
            <StyledInput
                v-if="hasToType"
                :id="inputId"
                v-model="confirmationTyped"
                :placeholder="formatMessage(messages.confirmationPlaceholder)"
                wrapper-class="max-w-[20rem]"
            />
            <div class="flex justify-end gap-2">
                <Button class="!shadow-none" @click="hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    type="colored"
                    :color="danger ? 'red' : 'brand'"
                    :disabled="actionDisabled"
                    @click="proceed"
                >
                    <component :is="proceedIcon ?? TrashIcon" />
                    {{ proceedLabel || formatMessage(messages.proceed) }}
                </Button>
            </div>
        </div>
    </PreviewModal>
</template>

<script setup lang="ts">
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import IntlFormatted from '@modrinth/ui/src/components/base/IntlFormatted.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { type Component, computed, ref } from 'vue'

import PreviewModal from './PreviewModal.vue'

/** The shared `ConfirmModal` (plain-text description), rendered inside the launcher window. */
const props = withDefaults(
    defineProps<{
        title: string
        description?: string
        confirmationText?: string
        hasToType?: boolean
        proceedIcon?: Component
        proceedLabel?: string
        danger?: boolean
        onHide?: () => void
    }>(),
    {
        description: undefined,
        confirmationText: '',
        hasToType: false,
        proceedIcon: undefined,
        proceedLabel: undefined,
        danger: true,
        onHide: () => {},
    },
)

const emit = defineEmits<{ proceed: [] }>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
    confirmationPrompt: {
        id: 'modal.confirm.confirmation-prompt',
        defaultMessage:
            'To confirm you want to proceed, type <confirmation-text>{confirmationText}</confirmation-text> below:',
    },
    confirmationPlaceholder: {
        id: 'modal.confirm.confirmation-placeholder',
        defaultMessage: 'Type here...',
    },
    proceed: { id: 'modal.confirm.proceed', defaultMessage: 'Proceed' },
})

const modal = ref<InstanceType<typeof PreviewModal>>()
const confirmationTyped = ref('')
const inputId = `preview-confirm-${Math.random().toString(36).slice(2, 9)}`

const actionDisabled = computed(
    () =>
        props.hasToType &&
        confirmationTyped.value.toLowerCase() !== props.confirmationText.toLowerCase(),
)

function proceed() {
    modal.value?.hide()
    confirmationTyped.value = ''
    emit('proceed')
}

function show(event?: MouseEvent) {
    modal.value?.show(event)
}

function hide() {
    modal.value?.hide()
}

defineExpose({ show, hide })
</script>
