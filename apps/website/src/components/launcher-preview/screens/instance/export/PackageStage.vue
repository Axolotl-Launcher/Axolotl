<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'

import { resourceOperationError } from '../content/resource-backups'
import { exportMessages } from '../messages'
import { resourceSourceName } from './backend'
import { injectExportContext } from './context'
const {
    packageInfo,
    format,
    exportError,
    exporting,
    selection,
    selectedSize,
    formatMessage,
    refreshing,
    refreshError,
    resolution,
    failedFiles,
    embedded,
    refresh,
    embed,
} = injectExportContext()
const formatBytes = useFormatBytes()
</script>
<template>
    <div class="flex min-w-0 flex-col gap-4">
        <p v-if="exportError" class="m-0 break-all" role="alert">{{ exportError }}</p>
        <div v-if="refreshError" class="flex flex-col items-start gap-2" role="alert">
            <p class="m-0 break-all">{{ refreshError }}</p>
            <Button :disabled="exporting" @click="refresh(true)">{{
                formatMessage(exportMessages.retryAnalysis)
            }}</Button>
        </div>
        <template v-else-if="resolution && !refreshing">
            <div v-if="failedFiles.length" class="flex flex-col gap-3">
                <p class="m-0">{{ formatMessage(exportMessages.failedAnalysis) }}</p>
                <div
                    v-for="file in failedFiles"
                    :key="file.snapshot.relativePath"
                    class="flex flex-col gap-2 rounded-xl border border-solid border-surface-4 p-3"
                >
                    <strong class="break-all">{{ file.snapshot.relativePath }}</strong>
                    <p v-if="file.localError" class="m-0 break-all">
                        {{ formatMessage(exportMessages.localFailure) }}
                    </p>
                    <template v-else>
                        <p
                            v-for="failure in file.resolutions.filter((r) => r.kind === 'failed')"
                            :key="failure.source"
                            class="m-0 break-all"
                        >
                            {{ resourceSourceName(failure.source) }}:
                            {{
                                failure.kind === 'failed'
                                    ? resourceOperationError(failure.message, formatMessage)
                                    : ''
                            }}
                        </p>
                        <Checkbox
                            :disabled="exporting"
                            :model-value="embedded.includes(file.snapshot.relativePath)"
                            :label="formatMessage(exportMessages.embedFailure)"
                            @update:model-value="embed(file.snapshot.relativePath, $event)"
                        />
                    </template>
                </div>
                <Button :disabled="exporting" class="self-start" @click="refresh(true)">{{
                    formatMessage(exportMessages.retryAnalysis)
                }}</Button>
            </div>
        </template>
        <p class="m-0">
            {{
                formatMessage(exportMessages.selection, {
                    count: selection.length,
                    size: formatBytes(selectedSize),
                })
            }}
        </p>
        <label class="flex flex-col gap-2">
            <span class="font-semibold">{{ formatMessage(exportMessages.name) }}</span>
            <StyledInput v-model="packageInfo.name" :disabled="exporting" wrapper-class="w-full" />
        </label>
        <label class="flex flex-col gap-2">
            <span class="font-semibold">{{ formatMessage(exportMessages.version) }}</span>
            <StyledInput
                v-model="packageInfo.version"
                :disabled="exporting"
                wrapper-class="w-full"
            />
        </label>
        <label class="flex flex-col gap-2">
            <span class="font-semibold">{{ formatMessage(exportMessages.author) }}</span>
            <StyledInput
                v-model="packageInfo.author"
                :disabled="exporting"
                wrapper-class="w-full"
            />
        </label>
        <label class="flex flex-col gap-2">
            <span class="font-semibold">{{ formatMessage(exportMessages.format) }}</span>
            <Combobox
                v-model="format"
                :disabled="exporting"
                :options="
                    (['modrinth', 'genericZip', 'curseForge'] as const).map((value) => ({
                        value,
                        label: formatMessage(exportMessages[value]),
                    }))
                "
            />
        </label>
        <p class="m-0 text-sm">{{ formatMessage(exportMessages.formatHint) }}</p>
        <label class="flex flex-col gap-2">
            <span class="font-semibold">{{ formatMessage(commonMessages.descriptionLabel) }}</span>
            <StyledInput
                v-model="packageInfo.description"
                :disabled="exporting"
                multiline
                wrapper-class="w-full"
            />
        </label>
    </div>
</template>
