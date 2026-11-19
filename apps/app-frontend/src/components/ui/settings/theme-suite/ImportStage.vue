<script setup lang="ts">
import { Button, useFormatBytes } from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { computed } from 'vue'

import { injectThemeSuiteContext } from './context'
import { themeSuiteMessages } from './messages'

const { formatMessage, importPath, installing, installError, limits } = injectThemeSuiteContext()

const formatBytes = useFormatBytes()

const archiveName = computed(() => {
    const path = importPath.value
    if (!path) return ''
    const segments = path.split(/[\\/]/)
    return segments[segments.length - 1] || path
})

const archiveLimit = computed(() =>
    limits.value
        ? formatMessage(themeSuiteMessages.archiveLimit, {
              size: formatBytes(limits.value.maxArchiveBytes),
          })
        : '',
)

async function chooseArchive() {
    const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: formatMessage(themeSuiteMessages.archiveFilter), extensions: ['zip'] }],
    })
    if (!selected || Array.isArray(selected)) return
    importPath.value = selected
    installError.value = ''
}
</script>

<template>
    <div class="flex min-w-0 flex-col gap-4">
        <div class="flex flex-col gap-1">
            <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(themeSuiteMessages.importTitle) }}
            </h3>
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(themeSuiteMessages.importDescription) }}
            </p>
        </div>

        <div class="flex flex-col gap-3 rounded-xl border border-solid border-surface-4 p-4">
            <div class="flex flex-wrap items-center gap-2">
                <Button :disabled="installing" @click="chooseArchive">
                    {{
                        formatMessage(
                            importPath
                                ? themeSuiteMessages.changeArchive
                                : themeSuiteMessages.chooseArchive,
                        )
                    }}
                </Button>
                <span
                    v-if="importPath"
                    class="min-w-0 break-all text-sm text-[var(--color-text-primary)]"
                >
                    {{ archiveName }}
                </span>
                <span v-else class="text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(themeSuiteMessages.noArchiveChosen) }}
                </span>
            </div>
            <p class="m-0 text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(themeSuiteMessages.archiveHint) }}
                <template v-if="archiveLimit"> {{ archiveLimit }}</template>
            </p>
        </div>

        <div v-if="installError" class="flex flex-col gap-1" role="alert">
            <strong class="text-sm text-[var(--color-red)]">
                {{ formatMessage(themeSuiteMessages.installFailed) }}
            </strong>
            <p class="m-0 break-all text-sm text-[var(--color-red)]">{{ installError }}</p>
        </div>
    </div>
</template>
