<template>
    <NewModal
        ref="modal"
        :header="
            formatMessage(messages.header, {
                type: formatFileItemType(formatMessage, type),
            })
        "
        max-width="500px"
    >
        <form class="space-y-6 md:min-w-[400px]" @submit.prevent="handleSubmit">
            <label class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(fileValidationMessages.nameLabel)
                }}</span>
                <StyledInput
                    ref="createInput"
                    v-model="itemName"
                    :placeholder="
                        formatMessage(
                            type === 'file'
                                ? messages.placeholderFile
                                : messages.placeholderDirectory,
                        )
                    "
                    wrapper-class="w-full"
                />
                <div v-if="submitted && error" class="text-sm text-red">{{ error }}</div>
            </label>
        </form>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="hide"
                    ><XIcon class="h-5 w-5" />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    :disabled="!!error && submitted"
                    @click="handleSubmit"
                    ><PlusIcon class="h-5 w-5" />
                    {{
                        formatMessage(messages.createButton, {
                            type: formatFileItemType(formatMessage, type),
                        })
                    }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { fileValidationMessages } from '@modrinth/ui/src/layouts/shared/files-tab/components/modals/file-validation-messages.ts'
import { commonMessages, formatFileItemType } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, ref } from 'vue'

import NewModal from '../../../../overlays/PreviewModal.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    header: {
        id: 'files.create-modal.header',
        defaultMessage: 'Create a {type}',
    },
    placeholderFile: {
        id: 'files.create-modal.placeholder-file',
        defaultMessage: 'e.g. config.yml',
    },
    placeholderDirectory: {
        id: 'files.create-modal.placeholder-directory',
        defaultMessage: 'e.g. my-folder',
    },
    createButton: {
        id: 'files.create-modal.create-button',
        defaultMessage: 'Create {type}',
    },
})

const props = defineProps<{
    type: 'file' | 'directory'
}>()

const emit = defineEmits<{
    create: [name: string]
}>()

const modal = ref<InstanceType<typeof NewModal>>()
const createInput = ref<HTMLInputElement | null>(null)
const itemName = ref('')
const submitted = ref(false)

const error = computed(() => {
    if (!itemName.value) {
        return formatMessage(fileValidationMessages.nameRequired)
    }
    if (props.type === 'file') {
        const validPattern = /^[a-zA-Z0-9-_.\s]+$/
        if (!validPattern.test(itemName.value)) {
            return formatMessage(fileValidationMessages.nameInvalidFile)
        }
    } else {
        const validPattern = /^[a-zA-Z0-9-_.\s]+$/
        if (!validPattern.test(itemName.value)) {
            return formatMessage(fileValidationMessages.nameInvalidDirectory)
        }
    }
    return ''
})

const handleSubmit = () => {
    submitted.value = true
    if (!error.value) {
        emit('create', itemName.value)
        hide()
    }
}

const show = () => {
    itemName.value = ''
    submitted.value = false
    modal.value?.show()
    nextTick(() => {
        setTimeout(() => {
            createInput.value?.focus()
        }, 100)
    })
}

const hide = () => {
    modal.value?.hide()
}

defineExpose({ show, hide })
</script>
