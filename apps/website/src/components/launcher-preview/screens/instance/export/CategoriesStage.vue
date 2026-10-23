<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'

import { exportMessages } from '../messages'
import CategoryRow from './CategoryRow.vue'
import { injectExportContext } from './context'
const { snapshot, scanning, error, categories, selection, selectedSize, scan, formatMessage } =
    injectExportContext()
const formatBytes = useFormatBytes()
</script>
<template>
    <div class="flex min-w-0 flex-col gap-4">
        <p class="m-0">{{ formatMessage(exportMessages.categoryHint) }}</p>
        <div
            v-if="scanning"
            class="flex flex-col gap-3"
            :aria-label="formatMessage(exportMessages.scan)"
            role="status"
        >
            <div
                v-for="row in 6"
                :key="row"
                class="h-8 w-full animate-pulse rounded-lg bg-surface-3"
            />
        </div>
        <div v-else-if="error" class="flex flex-col items-start gap-2" role="alert">
            <strong>{{ formatMessage(exportMessages.scanFailed) }}</strong>
            <p class="m-0 break-all">{{ error }}</p>
            <Button @click="scan">{{ formatMessage(exportMessages.retry) }}</Button>
        </div>
        <template v-else-if="snapshot">
            <div
                class="flex flex-col gap-1 rounded-xl border border-solid border-surface-4 bg-surface-3 p-3"
            >
                <strong>{{ formatMessage(exportMessages.requirements) }}</strong>
                <span
                    >Minecraft {{ snapshot.requirements.gameVersion
                    }}<template v-if="snapshot.requirements.loader">
                        · {{ snapshot.requirements.loader }}
                        {{ snapshot.requirements.loaderVersion }}</template
                    ></span
                >
                <span class="text-sm text-[var(--color-text-tertiary)]">{{
                    formatMessage(exportMessages.requirementsDescription)
                }}</span>
            </div>
            <p v-if="!snapshot.files.length" class="m-0">
                {{ formatMessage(exportMessages.noFiles) }}
            </p>
            <CategoryRow v-for="category in categories" :key="category.id" :category="category" />
            <p class="m-0 font-semibold">
                {{
                    formatMessage(exportMessages.selection, {
                        count: selection.length,
                        size: formatBytes(selectedSize),
                    })
                }}
            </p>
        </template>
    </div>
</template>
