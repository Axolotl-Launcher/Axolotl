<template>
    <NewModal ref="modal" :header="formatMessage(messages.header)" fade="warning" max-width="500px">
        <div class="flex flex-col gap-6">
            <SymlinkWarningAdmonition :symlink-target="symlinkTarget" />
            <Admonition type="warning" :header="formatMessage(messages.admonitionHeader)">
                {{
                    props.scopeDescription ??
                    formatMessage(messages.admonitionBody, { count: visibleCount })
                }}
            </Admonition>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="modal?.hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    v-tooltip="props.actionDisabled ? props.actionDisabledTooltip : undefined"
                    type="colored"
                    color="orange"
                    :disabled="props.actionDisabled"
                    @click="confirm"
                    ><DownloadIcon />
                    {{
                        props.actionLabel ??
                        formatMessage(messages.updateButton, { count: visibleCount })
                    }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import SymlinkWarningAdmonition from '@modrinth/ui/src/layouts/shared/content-tab/components/modals/SymlinkWarningAdmonition.vue'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { nextTick, ref } from 'vue'

import NewModal from '../../../../overlays/PreviewModal.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    header: {
        id: 'content.confirm-bulk-update.header',
        defaultMessage: 'Update projects',
    },
    admonitionHeader: {
        id: 'content.confirm-bulk-update.admonition-header',
        defaultMessage: 'Update warning',
    },
    admonitionBody: {
        id: 'content.confirm-bulk-update.admonition-body',
        defaultMessage:
            "Are you sure you want to update {count, plural, one {# project} other {# projects}} to their latest compatible version? It's recommended to update content one-by-one.",
    },
    updateButton: {
        id: 'content.confirm-bulk-update.update-button',
        defaultMessage: 'Update {count, plural, one {# project} other {# projects}}',
    },
})

const props = defineProps<{
    count: number
    server?: boolean
    backupTip?: string
    actionDisabled?: boolean
    actionDisabledTooltip?: string
    symlinkTarget?: string
    actionLabel?: string
    scopeDescription?: string
}>()

const emit = defineEmits<{
    (e: 'update'): void
}>()

const modal = ref<InstanceType<typeof NewModal>>()
const visibleCount = ref(props.count)

async function show() {
    await nextTick()
    visibleCount.value = props.count
    modal.value?.show()
}

function confirm() {
    if (props.actionDisabled) return
    modal.value?.hide()
    emit('update')
}

defineExpose({
    show,
})
</script>
