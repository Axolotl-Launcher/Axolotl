<template>
    <ConfirmActionModal
        ref="modal"
        :header="formatMessage(messages.header)"
        fade="warning"
        confirm-color="orange"
        :confirm-icon="UnlinkIcon"
        :confirm-label="formatMessage(props.server ? messages.header : messages.unlinkButton)"
        :confirm-disabled="props.actionDisabled"
        :confirm-disabled-tooltip="props.actionDisabledTooltip"
        :confirm-action="props.action"
        @confirm="emit('unlink')"
    >
        <template #warning>
            <Admonition type="warning" :header="formatMessage(messages.admonitionHeader)">
                {{ formatMessage(messages.admonitionBody) }}
            </Admonition>
        </template>
    </ConfirmActionModal>
</template>

<script setup lang="ts">
import { UnlinkIcon } from '@modrinth/assets'
import { useTemplateRef } from 'vue'

import Admonition from '#ui/components/base/Admonition.vue'
import ConfirmActionModal from '#ui/components/modal/ConfirmActionModal.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'

const props = defineProps<{
    server?: boolean
    actionDisabled?: boolean
    actionDisabledTooltip?: string
    action?: () => void | Promise<void>
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    header: {
        id: 'content.confirm-unlink.header',
        defaultMessage: 'Unlink modpack',
    },
    admonitionHeader: {
        id: 'content.confirm-unlink.admonition-header',
        defaultMessage: 'Unlinking modpack',
    },
    admonitionBody: {
        id: 'content.confirm-unlink.admonition-body',
        defaultMessage:
            'Mods and content will be merged with what you added on top of the modpack, and it will stop receiving updates.',
    },
    unlinkButton: {
        id: 'content.confirm-unlink.unlink-button',
        defaultMessage: 'Unlink',
    },
})

const emit = defineEmits<{
    (e: 'unlink'): void
}>()

const modal = useTemplateRef<InstanceType<typeof ConfirmActionModal>>('modal')

defineExpose({
    show: () => modal.value?.show(),
})
</script>
