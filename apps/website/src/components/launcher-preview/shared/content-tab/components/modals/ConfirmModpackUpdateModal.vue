<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import SymlinkWarningAdmonition from '@modrinth/ui/src/layouts/shared/content-tab/components/modals/SymlinkWarningAdmonition.vue'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref } from 'vue'

import PreviewModal from '../../../../overlays/PreviewModal.vue'

/** Port of `packages/ui/src/layouts/shared/content-tab/components/modals/ConfirmModpackUpdateModal.vue`. */
const props = defineProps<{
    downgrade?: boolean
    backupTip?: string
    actionDisabled?: boolean
    actionDisabledTooltip?: string
    symlinkTarget?: string
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    header: {
        id: 'content.confirm-modpack-update.header',
        defaultMessage: '{action, select, downgrade {Downgrade} other {Update}} modpack',
    },
    admonitionHeader: {
        id: 'content.confirm-modpack-update.admonition-header',
        defaultMessage: '{action, select, downgrade {Downgrade} other {Update}} warning',
    },
    admonitionBody: {
        id: 'content.confirm-modpack-update.admonition-body',
        defaultMessage:
            '{action, select, downgrade {Downgrading} other {Updating}} may cause compatibility issues. Mods or content you added on top of the modpack will be kept, but may not be compatible with the new version.',
    },
    confirmButton: {
        id: 'content.confirm-modpack-update.confirm-button',
        defaultMessage: '{action, select, downgrade {Downgrade} other {Update}} modpack',
    },
})

const emit = defineEmits<{
    (e: 'confirm' | 'cancel'): void
}>()

const modal = ref<InstanceType<typeof PreviewModal>>()

function show() {
    modal.value?.show()
}

function handleConfirm() {
    if (props.actionDisabled) return
    modal.value?.hide()
    emit('confirm')
}

function handleCancel() {
    modal.value?.hide()
    emit('cancel')
}

defineExpose({
    show,
})
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.header, { action: downgrade ? 'downgrade' : 'update' })"
        fade="warning"
        max-width="500px"
    >
        <div class="flex flex-col gap-6">
            <SymlinkWarningAdmonition :symlink-target="symlinkTarget" />
            <Admonition
                type="warning"
                :header="
                    formatMessage(messages.admonitionHeader, {
                        action: downgrade ? 'downgrade' : 'update',
                    })
                "
            >
                {{
                    formatMessage(messages.admonitionBody, {
                        action: downgrade ? 'downgrade' : 'update',
                    })
                }}
            </Admonition>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="handleCancel"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    v-tooltip="props.actionDisabled ? props.actionDisabledTooltip : undefined"
                    type="colored"
                    color="orange"
                    :disabled="props.actionDisabled"
                    @click="handleConfirm"
                    ><DownloadIcon />
                    {{
                        formatMessage(messages.confirmButton, {
                            action: downgrade ? 'downgrade' : 'update',
                        })
                    }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
