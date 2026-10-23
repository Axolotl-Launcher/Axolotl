<template>
    <NewModal ref="modal" fade="warning" :header="formatMessage(messages.header)" max-width="500px">
        <p class="m-0 text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.body) }}
        </p>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="handleCancel"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="red" @click="handleDiscard"
                    ><TrashIcon />
                    {{ formatMessage(messages.discard) }}
                </Button>
                <Button type="colored" color="green" @click="handleSave"
                    ><SaveIcon />
                    {{ formatMessage(commonMessages.saveButton) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref } from 'vue'

import NewModal from '../../../../overlays/PreviewModal.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    header: {
        id: 'files.unsaved-changes-modal.header',
        defaultMessage: 'Unsaved changes',
    },
    body: {
        id: 'files.unsaved-changes-modal.body',
        defaultMessage:
            'You have unsaved changes that will be lost if you leave. Would you like to save before leaving?',
    },
    discard: {
        id: 'files.unsaved-changes-modal.discard',
        defaultMessage: 'Discard',
    },
})

export type UnsavedChangesResult = 'cancel' | 'discard' | 'save'

const modal = ref<InstanceType<typeof NewModal>>()
let resolvePromise: ((value: UnsavedChangesResult) => void) | null = null

function prompt(): Promise<UnsavedChangesResult> {
    return new Promise((resolve) => {
        resolvePromise = resolve
        modal.value?.show()
    })
}

function resolve(result: UnsavedChangesResult) {
    modal.value?.hide()
    resolvePromise?.(result)
    resolvePromise = null
}

function handleCancel() {
    resolve('cancel')
}

function handleDiscard() {
    resolve('discard')
}

function handleSave() {
    resolve('save')
}

defineExpose({ prompt })
</script>
