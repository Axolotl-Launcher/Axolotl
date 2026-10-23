<script setup lang="ts">
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref } from 'vue'

/**
 * Port of `components/lab/mod-translation/ModTranslationFilePicker.vue`. The
 * system open dialog is the browser's file chooser, which hands over the
 * file's name rather than its path.
 */
defineProps<{
    path: string
}>()

const emit = defineEmits<{
    'update:path': [path: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    selectFile: {
        id: 'app.lab.mod-translation.select-file',
        defaultMessage: 'Choose a mod JAR',
    },
})

const input = ref<HTMLInputElement | null>(null)

function pickFile() {
    input.value?.click()
}

function onPicked(event: Event) {
    const target = event.target as HTMLInputElement
    const file = target.files?.[0]
    if (file) emit('update:path', file.name)
    target.value = ''
}
</script>

<template>
    <div class="file-picker">
        <input
            ref="input"
            type="file"
            accept=".jar"
            class="hidden"
            :aria-label="formatMessage(messages.selectFile)"
            @change="onPicked"
        />
        <Button type="outlined" color="brand" class="file-pick-button" @click="pickFile"
            ><FileArchiveIcon />
            <span>{{ formatMessage(messages.selectFile) }}</span>
        </Button>
        <span
            v-if="path"
            class="selected-path min-w-0 flex-1 overflow-hidden truncate text-[0.78rem] text-[var(--color-text-primary)]"
            :title="path"
            >{{ path }}</span
        >
        <span
            v-else
            class="selected-path empty min-w-0 flex-1 overflow-hidden truncate text-[0.78rem] text-[var(--color-text-tertiary)]"
            >{{ formatMessage(messages.selectFile) }}…</span
        >
    </div>
</template>

<style scoped>
.file-picker {
    display: flex;
    align-items: center;
    gap: 0.75rem;
}

.file-pick-button {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    white-space: nowrap;
}
</style>
