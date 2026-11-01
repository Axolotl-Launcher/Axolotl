<script setup lang="ts">
import { SaveIcon, TrashIcon, XIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { onBeforeUnmount, ref } from 'vue'

type SkinEditorUnsavedResult = 'save' | 'discard' | 'stay'

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof NewModal>>()
let resolvePrompt: ((result: SkinEditorUnsavedResult) => void) | null = null
let activePrompt: Promise<SkinEditorUnsavedResult> | null = null

const messages = defineMessages({
    title: { id: 'app.lab.skin-editor.unsaved.title', defaultMessage: 'Unsaved skin' },
    body: {
        id: 'app.lab.skin-editor.unsaved.body',
        defaultMessage: 'Save your skin before leaving, or discard the current changes.',
    },
    stay: { id: 'app.lab.skin-editor.unsaved.stay', defaultMessage: 'Stay' },
    discard: { id: 'app.lab.skin-editor.unsaved.discard', defaultMessage: 'Discard' },
    save: { id: 'app.lab.skin-editor.unsaved.save', defaultMessage: 'Save PNG' },
})

function prompt(): Promise<SkinEditorUnsavedResult> {
    if (activePrompt) return activePrompt
    activePrompt = new Promise((resolve) => {
        resolvePrompt = resolve
        modal.value?.show()
    })
    return activePrompt
}

function resolve(result: SkinEditorUnsavedResult) {
    modal.value?.hide()
    resolvePrompt?.(result)
    resolvePrompt = null
    activePrompt = null
}

onBeforeUnmount(() => resolve('stay'))

defineExpose({ prompt })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        max-width="500px"
        :closable="false"
        :close-on-esc="false"
        :close-on-click-outside="false"
    >
        <p class="m-0 text-[var(--color-text-tertiary)]">{{ formatMessage(messages.body) }}</p>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="resolve('stay')">
                    <XIcon />
                    {{ formatMessage(messages.stay) }}
                </Button>
                <Button type="colored" color="red" @click="resolve('discard')">
                    <TrashIcon />
                    {{ formatMessage(messages.discard) }}
                </Button>
                <Button type="colored" color="brand" @click="resolve('save')">
                    <SaveIcon />
                    {{ formatMessage(messages.save) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
