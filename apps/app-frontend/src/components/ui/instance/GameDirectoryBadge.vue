<script setup lang="ts">
import { FolderIcon } from '@modrinth/assets'
import { useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import type { GameDirectoryInfo } from '@/helpers/game-directory'
import { gameDirectoryMessages } from '@/helpers/game-directory-messages'

const props = defineProps<{
    directory: GameDirectoryInfo
}>()

const { formatMessage } = useVIntl()

const label = computed(() => {
    if (props.directory.kind === 'managed') {
        return formatMessage(gameDirectoryMessages.managedShort)
    }
    return props.directory.shortLabel ?? props.directory.path ?? ''
})

const title = computed(() => {
    if (props.directory.kind === 'managed') {
        return formatMessage(gameDirectoryMessages.managed)
    }
    return props.directory.path ?? label.value
})
</script>

<template>
    <span
        data-game-directory-badge
        class="inline-flex min-w-0 max-w-full items-center gap-1 rounded-md bg-surface-4 px-1.5 py-0.5 text-xs font-medium leading-4 text-[var(--color-text-tertiary)]"
        :title="title"
    >
        <FolderIcon class="size-3 shrink-0" aria-hidden="true" />
        <span class="truncate">{{ label }}</span>
    </span>
</template>
