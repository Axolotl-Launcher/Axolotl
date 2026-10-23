<script setup lang="ts">
import ArrowDownAZIcon from '@modrinth/assets/icons/arrow-down-a-z.svg?component'
import ArrowUpZAIcon from '@modrinth/assets/icons/arrow-up-z-a.svg?component'
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ClockArrowDownIcon from '@modrinth/assets/icons/clock-arrow-down.svg?component'
import ClockArrowUpIcon from '@modrinth/assets/icons/clock-arrow-up.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import PinIcon from '@modrinth/assets/icons/pin.svg?component'
import RotateCounterClockwiseIcon from '@modrinth/assets/icons/rotate-counter-clockwise.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import IconButton from '@modrinth/ui/src/components/base/buttons/IconButton.vue'
import PopoutMenu from '@modrinth/ui/src/components/base/PopoutMenu.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type { ContentSortMode } from '@modrinth/ui/src/layouts/shared/content-tab/composables/index.ts'

import { injectPreviewWindow } from '../../../lib/window'

const { formatMessage } = useVIntl()
const { layer: previewLayer } = injectPreviewWindow()
const messages = defineMessages({
    updateAll: { id: 'content.actions.update-all', defaultMessage: 'Update all' },
})

export interface ContentSortOption {
    id: ContentSortMode
    label: string
}

const props = withDefaults(
    defineProps<{
        sortMode: ContentSortMode
        sortLabel: string
        sortOptions: ContentSortOption[]
        viewOptionsLabel: string
        pinned: boolean
        pinTooltip: string
        resetTooltip: string
        hasBulkUpdateSupport?: boolean
        hasOutdatedProjects?: boolean
        bulkUpdateTooltip?: string
        isBulkOperating?: boolean
    }>(),
    {
        hasBulkUpdateSupport: false,
        hasOutdatedProjects: false,
        bulkUpdateTooltip: undefined,
        isBulkOperating: false,
    },
)

const emit = defineEmits<{
    selectSort: [mode: ContentSortMode]
    togglePin: []
    resetView: []
    updateAll: []
}>()
</script>

<template>
    <div class="flex items-center justify-end gap-2">
        <PopoutMenu
            :tooltip="props.sortLabel"
            placement="bottom-end"
            :container="previewLayer ?? undefined"
        >
            <Button type="quiet" circular icon-only :aria-label="props.sortLabel"
                ><ArrowUpZAIcon
                    v-if="
                        props.sortMode === 'project-name-desc' ||
                        props.sortMode === 'file-name-desc'
                    "
                />
                <ClockArrowDownIcon v-else-if="props.sortMode === 'date-added-newest'" />
                <ClockArrowUpIcon v-else-if="props.sortMode === 'date-added-oldest'" />
                <ArrowDownAZIcon v-else />
            </Button>
            <template #menu>
                <div
                    class="flex w-56 flex-col gap-1 p-1"
                    role="menu"
                    :aria-label="props.viewOptionsLabel"
                >
                    <Button
                        v-for="option in props.sortOptions"
                        :key="option.id"
                        :type="props.sortMode === option.id ? 'base' : 'quiet'"
                        class="flex w-full items-center !justify-start gap-2 text-left"
                        role="menuitemradio"
                        :aria-checked="props.sortMode === option.id"
                        @click="emit('selectSort', option.id)"
                    >
                        <CheckIcon
                            class="size-4 shrink-0"
                            :class="props.sortMode === option.id ? 'opacity-100' : 'opacity-0'"
                        />
                        <span>{{ option.label }}</span>
                    </Button>
                    <div class="my-1 h-px bg-surface-5" />
                    <Button
                        type="quiet"
                        class="flex w-full items-center !justify-start gap-2 text-left"
                        @click="emit('resetView')"
                        ><RotateCounterClockwiseIcon class="size-4" />
                        <span>{{ props.resetTooltip }}</span>
                    </Button>
                </div>
            </template>
        </PopoutMenu>

        <IconButton
            v-tooltip="props.pinTooltip"
            :label="props.pinTooltip"
            :type="props.pinned ? 'chip' : 'quiet'"
            :color="props.pinned ? 'brand' : undefined"
            :aria-pressed="props.pinned"
            @click="emit('togglePin')"
        >
            <PinIcon />
        </IconButton>

        <IconButton
            v-if="props.hasBulkUpdateSupport && props.hasOutdatedProjects"
            v-tooltip="props.bulkUpdateTooltip"
            :label="props.bulkUpdateTooltip ?? formatMessage(messages.updateAll)"
            color="green"
            type="quiet"
            interaction="filled"
            :disabled="props.isBulkOperating"
            @click="emit('updateAll')"
        >
            <DownloadIcon />
        </IconButton>
    </div>
</template>
