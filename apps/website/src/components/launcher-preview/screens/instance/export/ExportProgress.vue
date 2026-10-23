<script setup lang="ts">
import ProgressBar from '@modrinth/ui/src/components/base/ProgressBar.vue'
import { computed } from 'vue'

import { exportMessages } from '../messages'
import { resourceSourceName } from './backend'
import { injectExportContext } from './context'

const {
    scanning,
    refreshing,
    refreshProgress,
    refreshError,
    resolution,
    failedFiles,
    formatMessage,
} = injectExportContext()
const active = computed(() => scanning.value || refreshing.value)
const label = computed(() => {
    if (scanning.value) return formatMessage(exportMessages.scan)
    if (active.value) {
        if (refreshProgress.value?.phase === 'complete')
            return formatMessage(exportMessages.finalizingAnalysis)
        if (refreshProgress.value?.phase === 'hashing')
            return formatMessage(exportMessages.verifyingLocal)
        if (
            refreshProgress.value &&
            ['modrinth', 'curseforge', 'mcarchive', 'planet_minecraft'].includes(
                refreshProgress.value.phase,
            )
        )
            return formatMessage(exportMessages.verifyingSource, {
                source: resourceSourceName(refreshProgress.value.phase),
            })
        return formatMessage(exportMessages.analyzing)
    }
    if (refreshError.value || failedFiles.value.length)
        return formatMessage(exportMessages.checkNeedsAttention)
    if (resolution.value) return formatMessage(exportMessages.analyzed)
    return ''
})
</script>

<template>
    <div
        v-if="label"
        class="flex min-w-0 flex-col gap-1.5 text-sm font-normal"
        role="status"
        data-export-progress
    >
        <div class="flex items-center justify-between gap-3">
            <span>{{ label }}</span>
            <span
                v-if="active && refreshProgress"
                class="shrink-0 text-xs tabular-nums text-[var(--color-text-tertiary)]"
            >
                {{
                    Math.round(
                        (((refreshProgress.overallCompleted ?? refreshProgress.completed) +
                            (refreshProgress.currentFileFraction ?? 0)) /
                            Math.max(
                                1,
                                refreshProgress.overallTotal ?? refreshProgress.total ?? 1,
                            )) *
                            100,
                    )
                }}%
            </span>
        </div>
        <ProgressBar
            v-if="active"
            full-width
            :gradient-border="false"
            :max="Math.max(1, refreshProgress?.overallTotal ?? refreshProgress?.total ?? 1)"
            :progress="
                !scanning && refreshProgress
                    ? (refreshProgress.overallCompleted ?? refreshProgress.completed) +
                      (refreshProgress.currentFileFraction ?? 0)
                    : 0
            "
            :waiting="scanning || !refreshProgress"
            class="cursor-default"
            :aria-label="label"
        />
    </div>
</template>
