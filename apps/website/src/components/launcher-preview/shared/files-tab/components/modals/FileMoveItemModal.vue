<template>
    <NewModal
        ref="modal"
        :header="
            formatMessage(messages.header, {
                type: formatFileItemType(formatMessage, item?.type),
            })
        "
        max-width="500px"
    >
        <form class="space-y-6 md:min-w-[400px]" @submit.prevent="handleSubmit">
            <div class="flex flex-col gap-1">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.currentLocation)
                }}</span>
                <span class="text-[var(--color-text-tertiary)]">{{
                    `${currentPath}/${item?.name}`.replace('//', '/')
                }}</span>
            </div>
            <label class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.destinationPath)
                }}</span>
                <StyledInput
                    ref="destinationInput"
                    v-model="destination"
                    :placeholder="formatMessage(messages.destinationPlaceholder)"
                    wrapper-class="w-full"
                />
            </label>
        </form>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="hide"
                    ><XIcon class="h-5 w-5" />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="brand" @click="handleSubmit"
                    ><RightArrowIcon class="h-5 w-5" />
                    {{ formatMessage(commonMessages.moveButton) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import RightArrowIcon from '@modrinth/assets/icons/right-arrow.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type { FileItem } from '@modrinth/ui/src/layouts/shared/files-tab/types.ts'
import { commonMessages, formatFileItemType } from '@modrinth/ui/src/utils/common-messages.ts'
import { nextTick, ref } from 'vue'

import NewModal from '../../../../overlays/PreviewModal.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    header: {
        id: 'files.move-modal.header',
        defaultMessage: 'Move {type}',
    },
    currentLocation: {
        id: 'files.move-modal.current-location',
        defaultMessage: 'Current location',
    },
    destinationPath: {
        id: 'files.move-modal.destination-path',
        defaultMessage: 'Destination path',
    },
    destinationPlaceholder: {
        id: 'files.move-modal.destination-placeholder',
        defaultMessage: 'e.g. /my-folder',
    },
})

const destinationInput = ref<HTMLInputElement | null>(null)

defineProps<{
    item: Pick<FileItem, 'name' | 'type'> | null
    currentPath: string
}>()

const emit = defineEmits<{
    move: [destination: string]
}>()

const modal = ref<InstanceType<typeof NewModal>>()
const destination = ref('')

const handleSubmit = () => {
    const path = destination.value.replace('//', '/')
    const normalized = path.startsWith('/') ? path : `/${path}`
    emit('move', normalized)
    hide()
}

const show = () => {
    destination.value = ''
    modal.value?.show()
    nextTick(() => {
        setTimeout(() => {
            destinationInput.value?.focus()
        }, 100)
    })
}

const hide = () => {
    modal.value?.hide()
}

defineExpose({ show, hide })
</script>
