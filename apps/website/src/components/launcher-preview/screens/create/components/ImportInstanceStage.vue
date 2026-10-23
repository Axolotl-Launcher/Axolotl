<script setup lang="ts">
import FileIcon from '@modrinth/assets/icons/file.svg?component'
import FolderIcon from '@modrinth/assets/icons/folder.svg?component'
import BigOptionButton from '@modrinth/ui/src/components/base/BigOptionButton.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

import { importStageMessages as messages } from '../messages'
import { useCreateArea } from '../state'

/**
 * Port of `creation-flow-modal/components/ImportInstanceStage.vue`: the native file
 * and folder pickers, routed to `onImportFileReceived` (App.vue). The file picker is
 * the browser's; the folder picker is the simulated macOS open panel.
 */
const { formatMessage } = useVIntl()
const area = useCreateArea()

function handleOpenFilePicker() {
    void area.importFlow.pickFile()
}

function handleOpenFolderPicker() {
    void area.importFlow.pickFolder()
}
</script>

<template>
    <div data-onboarding-id="creation-import" class="flex flex-col gap-4">
        <div data-onboarding-id="creation-import-methods" class="flex flex-col gap-3">
            <BigOptionButton
                data-onboarding-id="creation-import-file"
                :icon="FileIcon"
                no-icon-border
                :title="formatMessage(messages.selectFile)"
                :description="formatMessage(messages.selectFileDescription)"
                @click="handleOpenFilePicker"
            />
            <BigOptionButton
                data-onboarding-id="creation-import-folder"
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
