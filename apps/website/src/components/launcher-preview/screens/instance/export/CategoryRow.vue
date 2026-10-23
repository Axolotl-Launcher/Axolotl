<script setup lang="ts">
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { computed } from 'vue'

import { exportMessages } from '../messages'
import type { ExportCategory } from './backend'
import { injectExportContext } from './context'
const props = defineProps<{ category: ExportCategory }>()
const { categoryFiles, categoryState, categoryTitle, setCategory, formatMessage } =
    injectExportContext()
const formatBytes = useFormatBytes()
const formatDate = useFormatDateTime({ dateStyle: 'medium', timeStyle: 'short' })
const files = computed(() => categoryFiles(props.category))
const size = computed(() =>
    files.value.reduce((size, file) => size + (file.integrity.size ?? 0), 0),
)
</script>
<template>
    <div class="flex min-w-0 flex-col gap-2">
        <div class="flex min-w-0 flex-col items-start justify-between gap-2 sm:flex-row sm:gap-3">
            <Checkbox
                :model-value="categoryState(category).checked"
                :indeterminate="categoryState(category).indeterminate"
                :label="categoryTitle(category)"
                label-class="min-w-0 break-all"
                class="min-w-0 flex-1"
                @update:model-value="setCategory(category, $event)"
            />
            <span class="shrink-0 text-sm text-[var(--color-text-tertiary)]">{{
                formatMessage(exportMessages.count, {
                    count: files.length,
                    size: formatBytes(size),
                })
            }}</span>
        </div>
        <div class="ml-8 flex min-w-0 flex-col gap-1 text-sm text-[var(--color-text-tertiary)]">
            <span v-if="category.title" class="break-all">{{ category.paths[0] }}</span>
            <span v-if="category.modifiedAt">{{ formatDate(new Date(category.modifiedAt)) }}</span>
        </div>
        <div
            v-if="category.children.length"
            class="ml-8 flex flex-col gap-3 border-0 border-l border-solid border-surface-4 pl-3"
        >
            <CategoryRow v-for="child in category.children" :key="child.id" :category="child" />
        </div>
    </div>
</template>
