<script setup lang="ts">
import { Button, Checkbox, FileTreeSelect, StyledInput, useFormatBytes } from '@modrinth/ui'

import { injectExportContext } from './context'
import { exportMessages } from './messages'
const {
    snapshot,
    treeItems,
    selection,
    selectedSize,
    currentPath,
    search,
    selectedOnly,
    restore,
    formatMessage,
} = injectExportContext()
const formatBytes = useFormatBytes()
</script>
<template>
    <div class="flex min-w-0 flex-col gap-3">
        <p class="m-0 text-sm">{{ formatMessage(exportMessages.fileHint) }}</p>
        <div class="flex flex-wrap items-center gap-3">
            <StyledInput
                v-model="search"
                :placeholder="formatMessage(exportMessages.search)"
                clearable
                wrapper-class="min-w-0 flex-1"
            />
            <Checkbox v-model="selectedOnly" :label="formatMessage(exportMessages.selected)" />
            <Button @click="restore">{{ formatMessage(exportMessages.restore) }}</Button>
        </div>
        <p class="m-0 font-semibold">
            {{
                formatMessage(exportMessages.selection, {
                    count: selection.length,
                    size: formatBytes(selectedSize),
                })
            }}
        </p>
        <FileTreeSelect
            v-model="selection"
            v-model:current-path="currentPath"
            :items="treeItems"
            :search-query="search"
            :selected-only="selectedOnly"
        />
        <p
            v-if="snapshot?.excludedPaths.length"
            class="m-0 text-sm text-[var(--color-text-tertiary)]"
        >
            {{ formatMessage(exportMessages.excluded, { count: snapshot.excludedPaths.length }) }}
        </p>
    </div>
</template>
