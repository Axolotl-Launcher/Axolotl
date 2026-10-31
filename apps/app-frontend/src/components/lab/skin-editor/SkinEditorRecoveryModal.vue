<script setup lang="ts">
import { HistoryIcon, PlusIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { computed, ref } from 'vue'

import type { SkinEditorRecovery } from '@/lab/skin-editor/session'

const { formatMessage, locale } = useVIntl()
const modal = ref<InstanceType<typeof NewModal>>()
const recovery = ref<SkinEditorRecovery>()
const emit = defineEmits<{
    continue: []
    'start-new': []
}>()

const messages = defineMessages({
    title: { id: 'app.lab.skin-editor.recovery.title', defaultMessage: 'Continue last skin?' },
    body: {
        id: 'app.lab.skin-editor.recovery.body',
        defaultMessage: 'A recovery draft was saved on {date}.',
    },
    startNew: { id: 'app.lab.skin-editor.recovery.start-new', defaultMessage: 'Start new' },
    continue: { id: 'app.lab.skin-editor.recovery.continue', defaultMessage: 'Continue' },
})

const formattedDate = computed(() => {
    if (!recovery.value) return ''
    return new Intl.DateTimeFormat(locale.value, {
        dateStyle: 'medium',
        timeStyle: 'short',
    }).format(new Date(recovery.value.updatedAt))
})

function show(nextRecovery: SkinEditorRecovery) {
    recovery.value = nextRecovery
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

function choose(result: 'continue' | 'start-new') {
    if (result === 'continue') emit('continue')
    else emit('start-new')
    hide()
}

defineExpose({ show, hide })
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
        <p class="m-0 text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.body, { date: formattedDate }) }}
        </p>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="choose('start-new')">
                    <PlusIcon />
                    {{ formatMessage(messages.startNew) }}
                </Button>
                <Button type="colored" color="brand" @click="choose('continue')">
                    <HistoryIcon />
                    {{ formatMessage(messages.continue) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
