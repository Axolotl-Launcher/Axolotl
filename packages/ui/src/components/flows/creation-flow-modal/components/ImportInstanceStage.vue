<template>
    <div class="flex flex-col gap-4">
        <div class="flex flex-col gap-3">
            <BigOptionButton
                :icon="FileIcon"
                no-icon-border
                :title="formatMessage(messages.selectFile)"
                :description="formatMessage(messages.selectFileDescription)"
                @click="handleOpenFilePicker"
            />
            <BigOptionButton
                :icon="FolderIcon"
                no-icon-border
                :title="formatMessage(messages.selectFolder)"
                :description="formatMessage(messages.selectFolderDescription)"
                @click="handleOpenFolderPicker"
            />
        </div>

        <span class="text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.importPrompt) }}
        </span>
    </div>
</template>

<script setup lang="ts">
import { FileIcon, FolderIcon } from '@modrinth/assets'
import { BigOptionButton, defineMessages, useVIntl } from '@modrinth/ui'

import { injectFilePicker } from '#ui/providers/file-picker'

import { injectCreationFlowContext } from '../creation-flow-context'

const ctx = injectCreationFlowContext()
const filePicker = injectFilePicker(null)
const { formatMessage } = useVIntl()

const messages = defineMessages({
    selectFile: {
        id: 'creation-flow.modal.import-instance.select-file',
        defaultMessage: 'Select file to import',
    },
    selectFileDescription: {
        id: 'creation-flow.modal.import-instance.select-file.description',
        defaultMessage: 'Import a modpack file or launcher archive',
    },
    selectFolder: {
        id: 'creation-flow.modal.import-instance.select-folder',
        defaultMessage: 'Select folder to import',
    },
    selectFolderDescription: {
        id: 'creation-flow.modal.import-instance.select-folder.description',
        defaultMessage: 'Import a launcher folder or .minecraft folder',
    },
    importPrompt: {
        id: 'creation-flow.modal.import-instance.import-prompt',
        defaultMessage:
            'Drag & drop launcher folders, modpack files, or .minecraft folders to import an instance in one click',
    },
})

// ── Native pickers (routed through the platform file-picker contract) ──
function applyPickedPath(filePath: string) {
    if (ctx.onImportFileReceived) {
        ctx.onImportFileReceived({
            file: null,
            filePath,
            source: 'file-picker',
        })
        return
    }

    // Fallback: set path directly on context
    ctx.modpackFile.value = null
    ctx.modpackFilePath.value = filePath
    if (ctx.finishDisabled.value) return
    if (ctx.flowType === 'instance') {
        ctx.finish()
    } else {
        ctx.modal.value?.setStage('final-config')
    }
}

async function handleOpenFilePicker() {
    try {
        const picked = await filePicker?.pickFile?.()
        if (!picked?.path) return
        applyPickedPath(picked.path)
    } catch {
        // do nothing
    }
}

async function handleOpenFolderPicker() {
    try {
        const picked = await filePicker?.pickFolder?.()
        if (!picked?.path) return
        applyPickedPath(picked.path)
    } catch {
        // do nothing
    }
}
</script>
