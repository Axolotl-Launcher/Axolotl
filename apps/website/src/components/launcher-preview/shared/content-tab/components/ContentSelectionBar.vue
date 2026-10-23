<script setup lang="ts">
import PowerIcon from '@modrinth/assets/icons/power.svg?component'
import PowerOffIcon from '@modrinth/assets/icons/power-off.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type { BulkOperationType } from '@modrinth/ui/src/layouts/shared/content-tab/composables/bulk-operations.ts'
import {
    canToggleContentItem,
    isDisabledContentItem,
    isEnabledContentItem,
} from '@modrinth/ui/src/layouts/shared/content-tab/composables/content-filtering.ts'
import type { ContentItem } from '@modrinth/ui/src/layouts/shared/content-tab/types.ts'
import {
    commonMessages,
    formatContentTypeSentence,
} from '@modrinth/ui/src/utils/common-messages.ts'
import { computed } from 'vue'

import { autoCleanToText, autoToHTML } from '../../../lib/motd-parser'
import FloatingActionBar from '../../../overlays/PreviewFloatingActionBar.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    selectedCount: {
        id: 'content.selection-bar.selected-count',
        defaultMessage: '{count, number} {contentType} selected',
    },
    selectedCountSimple: {
        id: 'content.selection-bar.selected-count-simple',
        defaultMessage: '{count, number} selected',
    },
    bulkEnabling: {
        id: 'content.selection-bar.bulk.enabling',
        defaultMessage: 'Enabling {progress}/{total} {contentType}...',
    },
    bulkEnablingWaiting: {
        id: 'content.selection-bar.bulk.enabling-waiting',
        defaultMessage: 'Enabling {contentType}...',
    },
    bulkDisabling: {
        id: 'content.selection-bar.bulk.disabling',
        defaultMessage: 'Disabling {progress}/{total} {contentType}...',
    },
    bulkDisablingWaiting: {
        id: 'content.selection-bar.bulk.disabling-waiting',
        defaultMessage: 'Disabling {contentType}...',
    },
    bulkUpdating: {
        id: 'content.selection-bar.bulk.updating',
        defaultMessage: 'Updating {progress}/{total} {contentType}...',
    },
    bulkUpdatingWaiting: {
        id: 'content.selection-bar.bulk.updating-waiting',
        defaultMessage: 'Updating {contentType}...',
    },
    bulkUpdatingCount: {
        id: 'content.selection-bar.bulk.updating-count',
        defaultMessage: 'Updating {count, number} {contentType}',
    },
    bulkDeleting: {
        id: 'content.selection-bar.bulk.deleting',
        defaultMessage: 'Deleting {progress}/{total} {contentType}...',
    },
    bulkDeletingWaiting: {
        id: 'content.selection-bar.bulk.deleting-waiting',
        defaultMessage: 'Deleting {contentType}...',
    },
    bulkEnablingCount: {
        id: 'content.selection-bar.bulk.enabling-count',
        defaultMessage: 'Enabling {count, number} {contentType}',
    },
    bulkDisablingCount: {
        id: 'content.selection-bar.bulk.disabling-count',
        defaultMessage: 'Disabling {count, number} {contentType}',
    },
    bulkDeletingCount: {
        id: 'content.selection-bar.bulk.deleting-count',
        defaultMessage: 'Deleting {count, number} {contentType}',
    },
    allAlreadyEnabled: {
        id: 'content.selection-bar.all-already-enabled',
        defaultMessage: 'All selected content is already enabled',
    },
    allAlreadyDisabled: {
        id: 'content.selection-bar.all-already-disabled',
        defaultMessage: 'All selected content is already disabled',
    },
})

interface Props {
    selectedItems: ContentItem[]
    contentTypeLabel?: string
    isBusy?: boolean
    busyTooltip?: string | null
    isBulkOperating?: boolean
    bulkOperation?: BulkOperationType | null
    bulkProgress?: number
    bulkTotal?: number
    bulkWaiting?: boolean
    bulkStatusMessage?: string | null
    bulkItemCount?: number
    ariaLabel?: string
    getItemId?: (item: ContentItem) => string
}

const props = withDefaults(defineProps<Props>(), {
    contentTypeLabel: undefined,
    isBusy: false,
    busyTooltip: undefined,
    isBulkOperating: false,
    bulkOperation: null,
    bulkProgress: 0,
    bulkTotal: 0,
    bulkWaiting: false,
    bulkStatusMessage: null,
    bulkItemCount: 0,
    ariaLabel: undefined,
    getItemId: undefined,
})

const emit = defineEmits<{
    clear: []
    enable: []
    disable: []
}>()

const shown = computed(() => props.selectedItems.length > 0 || props.isBulkOperating)
const iconStackOffset = 24
const visibleItems = computed(() => props.selectedItems.slice(0, 3))
const overflowCount = computed(() => Math.max(0, props.selectedItems.length - 3))
const iconStackWidth = computed(() => {
    if (props.selectedItems.length === 0) return 0
    return (
        32 + (visibleItems.value.length - 1 + (overflowCount.value > 0 ? 1 : 0)) * iconStackOffset
    )
})

function resolveItemId(item: ContentItem) {
    return props.getItemId?.(item) ?? item.file_path ?? item.file_name ?? item.id
}

function resolveItemTitle(item: ContentItem) {
    return item.project?.title ?? item.file_name
}

function itemTitleTooltip(item: ContentItem) {
    return { content: autoToHTML(resolveItemTitle(item)), html: true }
}

const allDisabled = computed(
    () =>
        !props.selectedItems.some(
            (item) => canToggleContentItem(item) && isEnabledContentItem(item),
        ),
)
const allEnabled = computed(
    () =>
        !props.selectedItems.some(
            (item) => canToggleContentItem(item) && isDisabledContentItem(item),
        ),
)

const selectedCountText = computed(() => {
    const count = props.isBulkOperating
        ? props.bulkItemCount || props.bulkTotal || props.selectedItems.length
        : props.selectedItems.length || props.bulkTotal
    if (props.isBulkOperating && props.bulkOperation) {
        const messageMap = {
            enable: messages.bulkEnablingCount,
            disable: messages.bulkDisablingCount,
            update: messages.bulkUpdatingCount,
            delete: messages.bulkDeletingCount,
        }
        return formatMessage(messageMap[props.bulkOperation], {
            count,
            contentType: formatContentTypeSentence(formatMessage, props.contentTypeLabel, count),
        })
    }

    if (props.contentTypeLabel) {
        return formatMessage(messages.selectedCount, {
            count,
            contentType: formatContentTypeSentence(formatMessage, props.contentTypeLabel, count),
        })
    }
    return formatMessage(messages.selectedCountSimple, { count })
})

const bulkProgressMessage = computed(() => {
    if (props.bulkStatusMessage) return props.bulkStatusMessage
    if (!props.bulkOperation) return ''
    const messageMap = {
        enable: props.bulkWaiting ? messages.bulkEnablingWaiting : messages.bulkEnabling,
        disable: props.bulkWaiting ? messages.bulkDisablingWaiting : messages.bulkDisabling,
        update: props.bulkWaiting ? messages.bulkUpdatingWaiting : messages.bulkUpdating,
        delete: props.bulkWaiting ? messages.bulkDeletingWaiting : messages.bulkDeleting,
    }
    return formatMessage(messageMap[props.bulkOperation], {
        progress: props.bulkProgress,
        total: props.bulkTotal,
        contentType: formatContentTypeSentence(
            formatMessage,
            props.contentTypeLabel,
            props.bulkTotal,
        ),
    })
})
</script>

<template>
    <FloatingActionBar :shown="shown" :aria-label="ariaLabel" hide-when-modal-open>
        <div class="flex items-center gap-0.5">
            <div
                v-if="selectedItems.length > 0"
                class="relative h-8 shrink-0"
                :style="{ width: `${iconStackWidth}px` }"
                aria-hidden="true"
            >
                <div
                    v-for="(item, index) in visibleItems"
                    :key="resolveItemId(item)"
                    v-tooltip="itemTitleTooltip(item)"
                    class="absolute top-0 flex h-8 w-8 items-center justify-center overflow-hidden rounded-lg border-[1.5px] border-solid border-surface-3 bg-surface-4"
                    :style="{
                        left: `${index * iconStackOffset}px`,
                        zIndex: visibleItems.length - index,
                    }"
                >
                    <Avatar
                        :src="item.project?.icon_url"
                        :alt="autoCleanToText(resolveItemTitle(item))"
                        :tint-by="resolveItemId(item)"
                        size="100%"
                        no-shadow
                        class="selected-content-avatar"
                    />
                </div>
                <div
                    v-if="overflowCount > 0"
                    class="absolute top-0 flex h-8 w-8 items-center justify-center rounded-lg border-[1.5px] border-solid border-surface-3 bg-surface-4 text-xs font-bold text-[var(--color-text-primary)]"
                    :style="{ left: `${visibleItems.length * iconStackOffset}px`, zIndex: 0 }"
                >
                    +{{ overflowCount }}
                </div>
            </div>

            <span
                class="px-3 py-2 text-base font-semibold tabular-nums text-[var(--color-text-primary)]"
            >
                {{ selectedCountText }}
            </span>
            <div class="mx-0.5 h-6 w-px bg-surface-5" />
            <Button
                v-tooltip="formatMessage(commonMessages.clearButton)"
                type="quiet"
                class="!text-[var(--color-text-default)]"
                :disabled="isBulkOperating"
                :class="{ 'pointer-events-none opacity-60': isBulkOperating }"
                @click="emit('clear')"
                ><XIcon class="cq-show-icon hidden" />
                <span class="bar-label">{{ formatMessage(commonMessages.clearButton) }}</span>
            </Button>
        </div>

        <div v-if="!isBulkOperating" class="ml-auto flex items-center gap-0.5">
            <slot name="actions" />

            <Button
                v-tooltip="
                    isBusy && busyTooltip
                        ? busyTooltip
                        : allEnabled
                          ? formatMessage(messages.allAlreadyEnabled)
                          : formatMessage(commonMessages.enableButton)
                "
                type="quiet"
                :disabled="isBusy || allEnabled"
                @click="emit('enable')"
                ><PowerIcon />
                <span class="bar-label">{{ formatMessage(commonMessages.enableButton) }}</span>
            </Button>
            <Button
                v-tooltip="
                    isBusy && busyTooltip
                        ? busyTooltip
                        : allDisabled
                          ? formatMessage(messages.allAlreadyDisabled)
                          : formatMessage(commonMessages.disableButton)
                "
                type="quiet"
                :disabled="isBusy || allDisabled"
                @click="emit('disable')"
                ><PowerOffIcon />
                <span class="bar-label">{{ formatMessage(commonMessages.disableButton) }}</span>
            </Button>

            <slot name="actions-end" />
        </div>

        <div v-else class="ml-auto flex items-center" aria-live="polite">
            <span
                class="px-4 py-2.5 text-base font-semibold tabular-nums text-[var(--color-text-tertiary)]"
            >
                {{ bulkProgressMessage }}
            </span>
        </div>

        <div v-if="isBulkOperating" class="absolute bottom-0 left-0 right-0 h-1">
            <div
                class="h-full rounded-l-full bg-brand transition-[width] duration-200 ease-in-out"
                :class="{ 'animate-indeterminate': bulkWaiting }"
                :style="
                    !bulkWaiting
                        ? { width: `${bulkTotal > 0 ? (bulkProgress / bulkTotal) * 100 : 0}%` }
                        : undefined
                "
                role="progressbar"
                :aria-valuenow="bulkWaiting ? undefined : bulkProgress"
                :aria-valuemin="0"
                :aria-valuemax="bulkTotal"
                style="box-shadow: 0px -2px 4px 0px var(--color-green)"
            />
        </div>
    </FloatingActionBar>
</template>

<style scoped>
@keyframes indeterminate {
    0% {
        width: 20%;
        margin-left: -20%;
    }
    100% {
        width: 60%;
        margin-left: 100%;
    }
}

.animate-indeterminate {
    animation: indeterminate 1.5s ease-in-out infinite;
}

:deep(.selected-content-avatar) {
    background-color: var(--surface-4);
}
</style>
