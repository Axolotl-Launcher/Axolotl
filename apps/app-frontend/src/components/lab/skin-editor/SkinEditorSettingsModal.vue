<script setup lang="ts">
import { Button, defineMessages, NewModal, StyledInput, Toggle, useVIntl } from '@modrinth/ui'
import { reactive, ref, watch } from 'vue'

import { clampAutoSaveInterval, type SkinEditorPreferences } from '@/lab/skin-editor/session'

const props = defineProps<{
    preferences: SkinEditorPreferences
}>()

const emit = defineEmits<{
    save: [preferences: SkinEditorPreferences]
}>()

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof NewModal>>()
const draft = reactive<SkinEditorPreferences>({ ...props.preferences })

const messages = defineMessages({
    title: { id: 'app.lab.skin-editor.settings.title', defaultMessage: 'Editor settings' },
    autoSave: {
        id: 'app.lab.skin-editor.settings.auto-save',
        defaultMessage: 'Automatically save a recovery draft',
    },
    autoSaveDescription: {
        id: 'app.lab.skin-editor.settings.auto-save-description',
        defaultMessage:
            'Recovery drafts stay on this device and are separate from exported PNG files.',
    },
    interval: {
        id: 'app.lab.skin-editor.settings.interval',
        defaultMessage: 'Save interval (seconds)',
    },
    intervalDescription: {
        id: 'app.lab.skin-editor.settings.interval-description',
        defaultMessage: 'Use a value between 5 and 600 seconds.',
    },
    warnBeforeLeave: {
        id: 'app.lab.skin-editor.settings.warn-before-leave',
        defaultMessage: 'Warn before leaving with unsaved changes',
    },
    warnBeforeLeaveDescription: {
        id: 'app.lab.skin-editor.settings.warn-before-leave-description',
        defaultMessage:
            'When auto-save is off, this warning is always enabled to protect your work.',
    },
    save: { id: 'app.lab.skin-editor.settings.save', defaultMessage: 'Save settings' },
    cancel: { id: 'app.lab.skin-editor.settings.cancel', defaultMessage: 'Cancel' },
})

function resetDraft() {
    Object.assign(draft, props.preferences)
}

function show() {
    resetDraft()
    modal.value?.show()
}

function toggleAutoSave(enabled: boolean) {
    draft.autoSaveEnabled = enabled
    if (!enabled) draft.warnBeforeLeave = true
}

function save() {
    emit('save', {
        autoSaveEnabled: draft.autoSaveEnabled,
        autoSaveIntervalSeconds: clampAutoSaveInterval(draft.autoSaveIntervalSeconds),
        warnBeforeLeave: draft.autoSaveEnabled ? draft.warnBeforeLeave : true,
    })
    modal.value?.hide()
}

watch(
    () => props.preferences,
    () => {
        if (!modal.value) resetDraft()
    },
    { deep: true },
)

defineExpose({ show })
</script>

<template>
    <NewModal ref="modal" :header="formatMessage(messages.title)" max-width="520px">
        <div class="flex flex-col gap-5">
            <div class="flex items-start justify-between gap-4">
                <div class="min-w-0">
                    <p class="m-0 font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.autoSave) }}
                    </p>
                    <p class="m-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.autoSaveDescription) }}
                    </p>
                </div>
                <Toggle
                    :model-value="draft.autoSaveEnabled"
                    :aria-label="formatMessage(messages.autoSave)"
                    @update:model-value="toggleAutoSave"
                />
            </div>

            <div class="flex flex-col gap-2">
                <label
                    class="font-semibold text-[var(--color-text-primary)]"
                    for="skin-auto-save-interval"
                >
                    {{ formatMessage(messages.interval) }}
                </label>
                <StyledInput
                    id="skin-auto-save-interval"
                    v-model="draft.autoSaveIntervalSeconds"
                    type="number"
                    :min="5"
                    :max="600"
                    :step="5"
                    :disabled="!draft.autoSaveEnabled"
                />
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.intervalDescription) }}
                </p>
            </div>

            <div class="flex items-start justify-between gap-4">
                <div class="min-w-0">
                    <p class="m-0 font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.warnBeforeLeave) }}
                    </p>
                    <p class="m-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.warnBeforeLeaveDescription) }}
                    </p>
                </div>
                <Toggle
                    :model-value="draft.warnBeforeLeave || !draft.autoSaveEnabled"
                    :disabled="!draft.autoSaveEnabled"
                    :aria-label="formatMessage(messages.warnBeforeLeave)"
                    @update:model-value="draft.warnBeforeLeave = $event"
                />
            </div>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="modal?.hide()">
                    {{ formatMessage(messages.cancel) }}
                </Button>
                <Button type="colored" color="brand" @click="save">
                    {{ formatMessage(messages.save) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
