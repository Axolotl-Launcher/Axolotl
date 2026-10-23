<script setup lang="ts">
import FileCodeIcon from '@modrinth/assets/icons/file-code.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'

import type { StudioDocument } from './useStudioDocuments'

defineProps<{
    documents: StudioDocument[]
    activePath: string
}>()

const emit = defineEmits<{
    activate: [path: string]
    close: [path: string]
}>()

const { formatMessage } = useVIntl()
let middleClickPath: string | null = null

function handleWheel(event: WheelEvent) {
    const container = event.currentTarget as HTMLElement
    if (container.scrollWidth <= container.clientWidth) return
    event.preventDefault()
    container.scrollLeft += event.deltaY || event.deltaX
}

function handleAuxClick(event: MouseEvent, path: string) {
    if (event.button !== 1) return
    event.preventDefault()
    event.stopPropagation()
    if (middleClickPath === path) {
        middleClickPath = null
        return
    }
    emit('close', path)
}

function handleMouseDown(event: MouseEvent, path: string) {
    if (event.button !== 1) return
    event.preventDefault()
    event.stopPropagation()
    middleClickPath = path
    emit('close', path)
}
</script>

<template>
    <div class="flex h-full min-w-0 flex-1 overflow-x-auto" @wheel="handleWheel">
        <div
            v-for="document in documents"
            :key="document.path"
            role="tab"
            tabindex="0"
            :aria-selected="document.path === activePath"
            class="flex h-full min-w-[8rem] max-w-[14rem] shrink-0 select-none items-center gap-2 border-0 border-r border-solid border-surface-4 px-3 text-left text-sm text-[var(--color-text-tertiary)] hover:bg-surface-2"
            :class="{
                'bg-surface-2 !text-[var(--color-text-primary)]': document.path === activePath,
            }"
            @click="emit('activate', document.path)"
            @mousedown="handleMouseDown($event, document.path)"
            @auxclick="handleAuxClick($event, document.path)"
            @keydown.enter="emit('activate', document.path)"
            @keydown.space.prevent="emit('activate', document.path)"
        >
            <XIcon v-if="document.kind === 'unsupported'" class="size-4 shrink-0 text-red" />
            <FileCodeIcon v-else class="size-4 shrink-0 text-[var(--color-text-tertiary)]" />
            <span class="min-w-0 flex-1 truncate">{{ document.name }}</span>
            <span
                v-if="document.content !== document.savedContent"
                class="size-2 shrink-0 rounded-full bg-brand"
            />
            <button
                type="button"
                :aria-label="formatMessage(commonMessages.closeButton)"
                class="flex size-5 shrink-0 cursor-pointer items-center justify-center rounded border-0 bg-transparent p-0 text-[var(--color-text-tertiary)] hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
                @pointerdown.stop
                @click.stop.prevent="emit('close', document.path)"
            >
                <XIcon class="size-3.5" />
            </button>
        </div>
    </div>
</template>
