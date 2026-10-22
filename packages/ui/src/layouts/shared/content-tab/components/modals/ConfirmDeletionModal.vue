<template>
    <NewModal
        ref="modal"
        :header="
            formatMessage(messages.header, {
                itemType: formatContentTypeSentence(formatMessage, visibleItemType, visibleCount),
            })
        "
        fade="warning"
        max-width="500px"
        :disable-close="busy"
    >
        <div class="flex flex-col gap-6">
            <SymlinkWarningAdmonition :symlink-target="symlinkTarget" />
            <Admonition type="warning" :header="formatMessage(messages.admonitionHeader)">
                {{ formatMessage(messages.admonitionBody) }}
            </Admonition>
            <p v-if="error" class="m-0 text-red">{{ error }}</p>
        </div>

        <template #actions>
            <div class="flex gap-2 justify-end">
                <Button type="outlined" :disabled="busy" @click="modal?.hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    v-tooltip="props.actionDisabled ? props.actionDisabledTooltip : undefined"
                    type="colored"
                    color="orange"
                    :disabled="props.actionDisabled || busy"
                    :loading="busy"
                    @click="confirm"
                    ><TrashIcon />
                    {{
                        formatMessage(messages.deleteButton, {
                            count: visibleCount,
                            itemType: formatContentTypeSentence(
                                formatMessage,
                                visibleItemType,
                                visibleCount,
                            ),
                        })
                    }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import { TrashIcon, XIcon } from '@modrinth/assets'
import { nextTick, ref } from 'vue'

import Admonition from '#ui/components/base/Admonition.vue'
import Button from '#ui/components/base/buttons/Button.vue'
import NewModal from '#ui/components/modal/NewModal.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages, formatContentTypeSentence } from '#ui/utils/common-messages'

import SymlinkWarningAdmonition from './SymlinkWarningAdmonition.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    failed: {
        id: 'content.confirm-deletion.failed',
        defaultMessage: 'The selected content could not be deleted. Try again.',
    },
    header: {
        id: 'content.confirm-deletion.header',
        defaultMessage: 'Delete {itemType}',
    },
    admonitionHeader: {
        id: 'content.confirm-deletion.admonition-header',
        defaultMessage: 'Deletion warning',
    },
    admonitionBody: {
        id: 'content.confirm-deletion.admonition-body',
        defaultMessage:
            'Deleting a mod can permanently affect your world and may cause missing content or unexpected issues when it loads again.',
    },
    deleteButton: {
        id: 'content.confirm-deletion.delete-button',
        defaultMessage: 'Delete {count, number} {itemType}',
    },
})

const props = withDefaults(
    defineProps<{
        count: number
        itemType: string
        variant?: 'instance' | 'server'
        backupTip?: string
        actionDisabled?: boolean
        actionDisabledTooltip?: string
        symlinkTarget?: string
        action?: () => Promise<void>
    }>(),
    {
        variant: 'instance',
        backupTip: undefined,
        actionDisabled: false,
        actionDisabledTooltip: undefined,
        symlinkTarget: undefined,
    },
)

const emit = defineEmits<{
    (e: 'delete'): void
}>()

const modal = ref<InstanceType<typeof NewModal>>()
const visibleCount = ref(props.count)
const visibleItemType = ref(props.itemType)
const busy = ref(false)
const error = ref<string | null>(null)

async function show() {
    error.value = null
    await nextTick()
    visibleCount.value = props.count
    visibleItemType.value = props.itemType
    modal.value?.show()
}

async function confirm() {
    if (props.actionDisabled || busy.value) return
    busy.value = true
    error.value = null
    try {
        if (props.action) await props.action()
        else emit('delete')
        busy.value = false
        await nextTick()
        await modal.value?.hide()
    } catch (failure) {
        console.error('Content deletion failed', failure)
        error.value = formatMessage(messages.failed)
    } finally {
        busy.value = false
    }
}

defineExpose({
    show,
})
</script>
