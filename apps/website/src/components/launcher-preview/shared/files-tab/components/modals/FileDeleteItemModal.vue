<template>
    <NewModal
        ref="modal"
        fade="danger"
        :header="formatMessage(isBulk ? messages.bulkHeader : messages.header)"
        max-width="500px"
    >
        <div class="flex flex-col gap-4">
            <Admonition
                v-if="symlinkTarget"
                type="warning"
                :header="formatMessage(messages.symlinkWarningHeader)"
            >
                {{ formatMessage(messages.symlinkWarningBody, { path: symlinkTarget }) }}
            </Admonition>
            <Admonition type="critical" class="md:min-w-[400px]">
                <template #header>{{
                    isBulk
                        ? formatMessage(messages.deletingMultiple, { count: bulkCount })
                        : formatMessage(messages.deletingName, { name: item?.name })
                }}</template>
                {{
                    isBulk
                        ? formatMessage(messages.bulkWarning)
                        : formatMessage(
                              item?.type === 'directory'
                                  ? messages.deleteFolderWarning
                                  : messages.deleteFileWarning,
                          )
                }}
            </Admonition>
        </div>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="hide"
                    ><XIcon class="h-5 w-5" />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="red" @click="handleSubmit"
                    ><TrashIcon class="h-5 w-5" />
                    {{ formatMessage(commonMessages.deleteLabel) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type { FileItem } from '@modrinth/ui/src/layouts/shared/files-tab/types.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref } from 'vue'

import NewModal from '../../../../overlays/PreviewModal.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    header: {
        id: 'files.delete-modal.header',
        defaultMessage: 'Delete file',
    },
    bulkHeader: {
        id: 'files.delete-modal.bulk-header',
        defaultMessage: 'Delete multiple items',
    },
    deletingName: {
        id: 'files.delete-modal.deleting-name',
        defaultMessage: 'Deleting "{name}"',
    },
    deletingMultiple: {
        id: 'files.delete-modal.deleting-multiple',
        defaultMessage: 'Deleting {count} items',
    },
    deleteFileWarning: {
        id: 'files.delete-modal.warning.file',
        defaultMessage: 'This file will be permanently deleted. This action cannot be undone.',
    },
    deleteFolderWarning: {
        id: 'files.delete-modal.warning.folder',
        defaultMessage:
            'This folder and all its contents will be permanently deleted. This action cannot be undone.',
    },
    bulkWarning: {
        id: 'files.delete-modal.bulk-warning',
        defaultMessage:
            'The selected items will be permanently deleted. This action cannot be undone.',
    },
    symlinkWarningHeader: {
        id: 'files.delete-modal.symlink-warning-header',
        defaultMessage: 'Shared instance',
    },
    symlinkWarningBody: {
        id: 'files.delete-modal.symlink-warning-body',
        defaultMessage:
            'You are modifying files in a shared instance linked to "{path}". Changes will affect the original instance.',
    },
})

defineProps<{
    item: Pick<FileItem, 'name' | 'type'> | null
    symlinkTarget?: string | null
}>()

const emit = defineEmits<{
    delete: []
}>()

const modal = ref<InstanceType<typeof NewModal>>()
const isBulk = ref(false)
const bulkCount = ref(0)

const handleSubmit = () => {
    emit('delete')
    hide()
}

const show = () => {
    isBulk.value = false
    bulkCount.value = 0
    modal.value?.show()
}

const showBulk = (count: number) => {
    isBulk.value = true
    bulkCount.value = count
    modal.value?.show()
}

const hide = () => {
    modal.value?.hide()
}

defineExpose({ show, showBulk, hide })
</script>
