<script setup lang="ts">
import { XIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

const { formatMessage } = useVIntl()

defineProps<{
    count: number
}>()

const emit = defineEmits<{
    stay: []
    discard: []
}>()

const messages = defineMessages({
    title: {
        id: 'app.unsaved-selection.title',
        defaultMessage: 'Uninstalled selection',
    },
    description: {
        id: 'app.unsaved-selection.description',
        defaultMessage:
            'You have selected {count} item(s) that have not been installed. Leaving will discard them.',
    },
    stay: {
        id: 'app.unsaved-selection.stay',
        defaultMessage: 'Stay on this page',
    },
    discard: {
        id: 'app.unsaved-selection.discard',
        defaultMessage: 'Discard and leave',
    },
})

const modal = ref<InstanceType<typeof NewModal> | null>(null)
let settled = false

function show() {
    settled = false
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

function settle(leave: boolean) {
    if (settled) return
    settled = true
    modal.value?.hide()
    if (leave) emit('discard')
    else emit('stay')
}

defineExpose({ show, hide })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        max-width="500px"
        :on-hide="() => settle(false)"
    >
        <p class="m-0 text-[var(--color-text-default)]">
            {{ formatMessage(messages.description, { count }) }}
        </p>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="settle(false)">
                    <XIcon />
                    {{ formatMessage(messages.stay) }}
                </Button>
                <Button type="colored" color="red" @click="settle(true)">
                    {{ formatMessage(messages.discard) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
