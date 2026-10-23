<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import SymlinkWarningAdmonition from '@modrinth/ui/src/layouts/shared/content-tab/components/modals/SymlinkWarningAdmonition.vue'
import { useTemplateRef } from 'vue'

import PreviewConfirmActionModal from '../../../../overlays/PreviewConfirmActionModal.vue'

/** Port of `packages/ui/src/layouts/shared/content-tab/components/modals/ConfirmReinstallModal.vue`. */
const { formatMessage } = useVIntl()

const messages = defineMessages({
    header: {
        id: 'instance.confirm-reinstall.header',
        defaultMessage: 'Reinstall modpack',
    },
    admonitionHeader: {
        id: 'instance.confirm-reinstall.admonition-header',
        defaultMessage: 'Reinstallation warning',
    },
    admonitionBody: {
        id: 'instance.confirm-reinstall.admonition-body',
        defaultMessage:
            'Reinstalling will reset all installed or modified content to what is provided by the modpack, removing any mods or content you have added on top of the original installation.',
    },
    reinstallButton: {
        id: 'instance.confirm-reinstall.reinstall-button',
        defaultMessage: 'Reinstall modpack',
    },
})

defineProps<{
    server?: boolean
    backupTip?: string
    symlinkTarget?: string
}>()

const emit = defineEmits<{
    (e: 'reinstall'): void
}>()

const modal = useTemplateRef<InstanceType<typeof PreviewConfirmActionModal>>('modal')

defineExpose({
    show: () => modal.value?.show(),
})
</script>

<template>
    <PreviewConfirmActionModal
        ref="modal"
        :header="formatMessage(messages.header)"
        fade="danger"
        confirm-color="red"
        :confirm-icon="DownloadIcon"
        :confirm-label="formatMessage(messages.reinstallButton)"
        @confirm="emit('reinstall')"
    >
        <template #warning>
            <SymlinkWarningAdmonition :symlink-target="symlinkTarget" />
            <Admonition type="critical" :header="formatMessage(messages.admonitionHeader)">
                {{ formatMessage(messages.admonitionBody) }}
            </Admonition>
        </template>
    </PreviewConfirmActionModal>
</template>
