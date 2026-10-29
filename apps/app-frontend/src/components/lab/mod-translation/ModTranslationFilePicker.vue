<script setup lang="ts">
import { FileArchiveIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'

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
    dialogTitle: {
        id: 'app.lab.mod-translation.dialog-title',
        defaultMessage: 'Choose a Minecraft mod JAR',
    },
    fileFilterMod: {
        id: 'app.lab.mod-translation.file-filter.mod',
        defaultMessage: 'Minecraft mod',
    },
})

async function pickFile() {
    const path = await open({
        multiple: false,
        title: formatMessage(messages.dialogTitle),
        filters: [{ name: formatMessage(messages.fileFilterMod), extensions: ['jar'] }],
    })
    if (typeof path === 'string') emit('update:path', path)
}
</script>

<template>
    <div class="file-picker">
        <Button type="outlined" color="brand" class="file-pick-button" @click="pickFile"
            ><FileArchiveIcon />
            <span>{{ formatMessage(messages.selectFile) }}</span>
        </Button>
        <span
            v-if="path"
            class="selected-path min-w-0 overflow-hidden flex-1 text-[var(--color-text-primary)] text-[0.78rem] truncate"
            :title="path"
            >{{ path }}</span
        >
        <span
            v-else
            class="selected-path empty min-w-0 overflow-hidden flex-1 text-[var(--color-text-tertiary)] text-[0.78rem] truncate"
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
