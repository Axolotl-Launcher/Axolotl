<script setup lang="ts">
import { ChevronDownIcon, ChevronUpIcon } from '@modrinth/assets'
import { computed, getCurrentInstance, ref, toRef, watch } from 'vue'

import Checkbox from '#ui/components/base/Checkbox.vue'
import { useVIntl } from '#ui/composables/i18n'
import { useStickyObserver } from '#ui/composables/sticky-observer'
import { useVirtualScroll } from '#ui/composables/virtual-scroll'
import { commonMessages } from '#ui/utils/common-messages'

import { useGroupSelection } from '../composables/group-selection'
import type {
    ContentCardTableItem,
    ContentCardTableSortColumn,
    ContentCardTableSortDirection,
} from '../types'
import ContentCardItem from './ContentCardItem.vue'

const { formatMessage } = useVIntl()

interface Props {
    items: ContentCardTableItem[]
    showSelection?: boolean
    sortable?: boolean
    sortBy?: ContentCardTableSortColumn
    sortDirection?: ContentCardTableSortDirection
    virtualized?: boolean
    hideDelete?: boolean
    hideHeader?: boolean
    flat?: boolean
    expandedGroups?: Set<string>
}

const props = withDefaults(defineProps<Props>(), {
    showSelection: false,
    sortable: false,
    sortBy: undefined,
    sortDirection: 'asc',
    virtualized: true,
    hideDelete: false,
    hideHeader: false,
    flat: false,
    expandedGroups: () => new Set(),
})

const stickyHeaderRef = ref<HTMLElement | null>(null)
const { isStuck } = useStickyObserver(stickyHeaderRef, 'ContentCardTable')

const selectedIds = defineModel<string[]>('selectedIds', { default: () => [] })

const emit = defineEmits<{
    'update:enabled': [id: string, value: boolean]
    delete: [id: string, event: MouseEvent]
    update: [id: string]
    switchVersion: [id: string]
    rollback: [id: string]
    sort: [column: ContentCardTableSortColumn, direction: ContentCardTableSortDirection]
    toggleExpand: [groupId: string]
    visibleItems: [items: ContentCardTableItem[]]
}>()

// Check if any actions are available
const instance = getCurrentInstance()
const hasDeleteListener = computed(() => typeof instance?.vnode.props?.onDelete === 'function')
const hasUpdateListener = computed(() => typeof instance?.vnode.props?.onUpdate === 'function')
const hasSwitchVersionListener = computed(
    () => typeof instance?.vnode.props?.onSwitchVersion === 'function',
)
const hasEnabledListener = computed(
    () => typeof instance?.vnode.props?.['onUpdate:enabled'] === 'function',
)

const hasAnyActions = computed(() => {
    // Check if there are listeners for actions
    const hasListeners =
        (hasDeleteListener.value && !props.hideDelete) ||
        hasUpdateListener.value ||
        hasSwitchVersionListener.value ||
        hasEnabledListener.value

    // Check if any items have overflow options or updates
    const hasItemActions = props.items.some(
        (item) =>
            (item.overflowOptions && item.overflowOptions.length > 0) ||
            (item.inlineActions && item.inlineActions.length > 0) ||
            item.isGroupHeader ||
            item.hasUpdate ||
            item.enabled !== undefined,
    )

    return hasListeners || hasItemActions
})

// Virtualization
const { listContainer, totalHeight, visibleRange, visibleTop, visibleItems } = useVirtualScroll(
    toRef(props, 'items'),
    {
        itemHeight: 74,
        bufferSize: 5,
        initialItemCount: 20,
        enabled: toRef(props, 'virtualized'),
    },
)

watch(
    visibleItems,
    (items) => {
        emit('visibleItems', items)
    },
    { immediate: true },
)

// Expose for perf monitoring
defineExpose({
    visibleRange,
    visibleItems,
})

// Selection logic
const {
    allSelected,
    someSelected,
    getGroupCheckboxState,
    isItemSelected,
    toggleSelectAll,
    toggleItemSelection,
} = useGroupSelection({
    items: toRef(props, 'items'),
    selectedIds,
})

const lastSelectedIndex = ref<number | null>(null)

function handleSort(column: ContentCardTableSortColumn) {
    if (!props.sortable) return

    const newDirection: ContentCardTableSortDirection =
        props.sortBy === column && props.sortDirection === 'asc' ? 'desc' : 'asc'

    emit('sort', column, newDirection)
}

function handleItemUpdate(id: string) {
    emit('update', id)
}
</script>

<template>
    <div
        data-content-card-table
        role="table"
        class="@container border border-solid border-surface-4 shadow-sm overflow-clip"
        :class="[
            flat ? '' : 'rounded-[var(--radius-xl)]',
            isStuck || hideHeader ? 'border-t-0' : '',
        ]"
    >
        <div
            v-if="!hideHeader"
            ref="stickyHeaderRef"
            role="rowgroup"
            class="sticky top-0 z-10 bg-surface-3"
            :class="[
                flat || isStuck ? 'rounded-none' : 'rounded-t-[20px]',
                isStuck
                    ? 'transition-[border-radius] duration-100 border-0 border-y border-solid border-surface-4 shadow-md before:pointer-events-none before:absolute before:inset-x-0 before:-top-4 before:h-5 before:bg-surface-3'
                    : '',
            ]"
        >
            <div class="flex h-12 items-center justify-between gap-4 px-3">
                <div
                    role="row"
                    class="flex min-w-0 items-center gap-4"
                    :class="hasAnyActions ? 'flex-1 min-w-0' : 'flex-1'"
                >
                    <Checkbox
                        v-if="showSelection"
                        :model-value="allSelected"
                        :indeterminate="someSelected"
                        :aria-label="formatMessage(commonMessages.selectAllLabel)"
                        class="shrink-0"
                        @update:model-value="toggleSelectAll"
                    />

                    <template v-if="$slots['header-project']">
                        <slot name="header-project" />
                    </template>
                    <button
                        v-else-if="sortable"
                        role="columnheader"
                        :aria-sort="
                            sortBy === 'project'
                                ? sortDirection === 'asc'
                                    ? 'ascending'
                                    : 'descending'
                                : 'none'
                        "
                        class="flex items-center gap-1.5 font-semibold text-[var(--color-text-tertiary)]"
                        @click="handleSort('project')"
                    >
                        {{ formatMessage(commonMessages.projectLabel) }}
                        <ChevronUpIcon
                            v-if="sortBy === 'project' && sortDirection === 'asc'"
                            class="size-4"
                        />
                        <ChevronDownIcon
                            v-else-if="sortBy === 'project' && sortDirection === 'desc'"
                            class="size-4"
                        />
                    </button>
                    <span
                        v-else
                        role="columnheader"
                        class="font-semibold text-[var(--color-text-tertiary)]"
                        >{{ formatMessage(commonMessages.projectLabel) }}</span
                    >
                </div>

                <div v-if="!$slots['header-project']" class="hidden @[800px]:flex flex-none">
                    <button
                        v-if="sortable"
                        role="columnheader"
                        :aria-sort="
                            sortBy === 'version'
                                ? sortDirection === 'asc'
                                    ? 'ascending'
                                    : 'descending'
                                : 'none'
                        "
                        class="flex items-center gap-1.5 font-semibold text-[var(--color-text-tertiary)]"
                        @click="handleSort('version')"
                    >
                        <ChevronUpIcon
                            v-if="sortBy === 'version' && sortDirection === 'asc'"
                            class="size-4"
                        />
                        <ChevronDownIcon
                            v-else-if="sortBy === 'version' && sortDirection === 'desc'"
                            class="size-4"
                        />
                    </button>
                    <span
                        v-else
                        role="columnheader"
                        class="font-semibold text-[var(--color-text-tertiary)]"
                    ></span>
                </div>

                <div
                    v-if="hasAnyActions || $slots['header-actions']"
                    role="columnheader"
                    :class="$slots['header-project'] ? 'shrink-0' : 'min-w-[160px] shrink-0'"
                >
                    <slot name="header-actions" />
                </div>
            </div>
            <slot name="header-status" />
        </div>

        <div
            v-if="items.length > 0 && virtualized"
            ref="listContainer"
            role="rowgroup"
            class="relative w-full"
            :class="flat ? '' : 'rounded-b-[20px]'"
            :style="{ minHeight: `${totalHeight}px`, overflowAnchor: 'none' }"
        >
            <div class="absolute w-full" :style="{ top: `${visibleTop}px` }">
                <ContentCardItem
                    v-for="(item, idx) in visibleItems"
                    :key="item.id"
                    data-content-card-item
                    :data-content-card-item-id="item.id"
                    :project="item.project"
                    :project-link="item.projectLink"
                    :version="item.version"
                    :version-link="item.versionLink"
                    :owner="item.owner"
                    :enabled="item.enabled"
                    :installing="item.installing"
                    :pending-manual-download="item.pendingManualDownload"
                    :duplicate-count="item.duplicateCount"
                    :local-path="item.localPath"
                    :installation-source="item.installationSource"
                    :group-status="item.groupStatus"
                    :update-label="item.updateLabel"
                    :has-update="item.hasUpdate"
                    :rollback-file-name="item.rollbackFileName"
                    :hide-switch-version="item.hideSwitchVersion"
                    :overflow-options="item.overflowOptions"
                    :disabled="item.disabled"
                    :disabled-tooltip="item.disabledTooltip"
                    :post-upgrade-warning-tooltip="item.postUpgradeWarningTooltip"
                    :compatibility-warning-tooltip="item.compatibilityWarningTooltip"
                    :toggle-disabled="item.toggleDisabled"
                    :toggle-disabled-tooltip="item.toggleDisabledTooltip"
                    :dependency-badge="item.dependencyBadge"
                    :show-checkbox="showSelection"
                    :hide-delete="hideDelete"
                    :hide-actions="!hasAnyActions"
                    :is-group-header="item.isGroupHeader"
                    :group-depth="item.groupDepth"
                    :group-item-count="item.groupItemCount"
                    :is-group-child="!!item.group && !item.isGroupHeader"
                    :group-kind="item.groupKind"
                    :group-meta="item.groupMeta"
                    :downloads="item.downloads"
                    :categories="item.categories"
                    :inline-actions="item.inlineActions"
                    :group-checkbox-indeterminate="
                        item.isGroupHeader ? getGroupCheckboxState(item).indeterminate : false
                    "
                    :group-expanded="
                        item.isGroupHeader && item.group
                            ? props.expandedGroups.has(item.group)
                            : false
                    "
                    :group-switch-version="item.groupSwitchVersion"
                    :selected="
                        item.isGroupHeader
                            ? getGroupCheckboxState(item).checked
                            : isItemSelected(item.id)
                    "
                    :class="[
                        isItemSelected(item.id)
                            ? 'bg-surface-content-selected'
                            : (visibleRange.start + idx) % 2 === 1
                              ? 'bg-surface-content-alt'
                              : 'bg-surface-content-muted',
                        'border-0 border-t border-solid border-surface-4',
                        visibleRange.start + idx === items.length - 1 && !flat
                            ? 'rounded-b-[20px]'
                            : '',
                    ]"
                    @select="
                        (val, event) =>
                            toggleItemSelection(
                                item.id,
                                val ?? false,
                                lastSelectedIndex,
                                visibleRange.start + idx,
                                event,
                                item,
                            )
                    "
                    @update:enabled="(val) => emit('update:enabled', item.id, val)"
                    @delete="(e: MouseEvent) => emit('delete', item.id, e)"
                    @update="handleItemUpdate(item.id)"
                    @switch-version="emit('switchVersion', item.id)"
                    @rollback="emit('rollback', item.id)"
                    @toggle-expand="item.group ? emit('toggleExpand', item.group) : undefined"
                >
                    <template #additionalButtonsLeft>
                        <slot
                            name="itemButtonsLeft"
                            :item="item"
                            :index="visibleRange.start + idx"
                        />
                    </template>
                    <template #additionalButtonsRight>
                        <slot
                            name="itemButtonsRight"
                            :item="item"
                            :index="visibleRange.start + idx"
                        />
                    </template>
                </ContentCardItem>
            </div>
        </div>

        <div
            v-else-if="items.length > 0"
            ref="listContainer"
            role="rowgroup"
            :class="flat ? '' : 'rounded-b-[20px]'"
        >
            <ContentCardItem
                v-for="(item, index) in items"
                :key="item.id"
                data-content-card-item
                :data-content-card-item-id="item.id"
                :project="item.project"
                :project-link="item.projectLink"
                :version="item.version"
                :version-link="item.versionLink"
                :owner="item.owner"
                :enabled="item.enabled"
                :installing="item.installing"
                :pending-manual-download="item.pendingManualDownload"
                :duplicate-count="item.duplicateCount"
                :local-path="item.localPath"
                :installation-source="item.installationSource"
                :group-status="item.groupStatus"
                :update-label="item.updateLabel"
                :has-update="item.hasUpdate"
                :rollback-file-name="item.rollbackFileName"
                :hide-switch-version="item.hideSwitchVersion"
                :overflow-options="item.overflowOptions"
                :disabled="item.disabled"
                :disabled-tooltip="item.disabledTooltip"
                :post-upgrade-warning-tooltip="item.postUpgradeWarningTooltip"
                :compatibility-warning-tooltip="item.compatibilityWarningTooltip"
                :toggle-disabled="item.toggleDisabled"
                :toggle-disabled-tooltip="item.toggleDisabledTooltip"
                :dependency-badge="item.dependencyBadge"
                :show-checkbox="showSelection"
                :hide-delete="hideDelete"
                :hide-actions="!hasAnyActions"
                :is-group-header="item.isGroupHeader"
                :group-depth="item.groupDepth"
                :group-item-count="item.groupItemCount"
                :is-group-child="!!item.group && !item.isGroupHeader"
                :group-kind="item.groupKind"
                :group-meta="item.groupMeta"
                :downloads="item.downloads"
                :categories="item.categories"
                :inline-actions="item.inlineActions"
                :group-checkbox-indeterminate="
                    item.isGroupHeader ? getGroupCheckboxState(item).indeterminate : false
                "
                :group-expanded="
                    item.isGroupHeader && item.group ? props.expandedGroups.has(item.group) : false
                "
                :group-switch-version="item.groupSwitchVersion"
                :selected="
                    item.isGroupHeader
                        ? getGroupCheckboxState(item).checked
                        : isItemSelected(item.id)
                "
                :class="[
                    isItemSelected(item.id)
                        ? 'bg-surface-content-selected'
                        : index % 2 === 1
                          ? 'bg-surface-content-alt'
                          : 'bg-surface-content-muted',
                    'border-0 border-t border-solid border-surface-4',
                    index === items.length - 1 && !flat ? 'rounded-b-[20px]' : '',
                ]"
                @select="
                    (val, event) =>
                        toggleItemSelection(
                            item.id,
                            val ?? false,
                            lastSelectedIndex,
                            index,
                            event,
                            item,
                        )
                "
                @update:enabled="(val) => emit('update:enabled', item.id, val)"
                @delete="(e: MouseEvent) => emit('delete', item.id, e)"
                @update="handleItemUpdate(item.id)"
                @switch-version="emit('switchVersion', item.id)"
                @rollback="emit('rollback', item.id)"
                @toggle-expand="item.group ? emit('toggleExpand', item.group) : undefined"
            >
                <template #additionalButtonsLeft>
                    <slot name="itemButtonsLeft" :item="item" :index="index" />
                </template>
                <template #additionalButtonsRight>
                    <slot name="itemButtonsRight" :item="item" :index="index" />
                </template>
            </ContentCardItem>
        </div>

        <div
            v-else
            class="flex items-center justify-center py-12"
            :class="flat ? '' : 'rounded-b-[20px]'"
        >
            <slot name="empty">
                <span class="text-[var(--color-text-tertiary)]">{{
                    formatMessage(commonMessages.noItemsLabel)
                }}</span>
            </slot>
        </div>
    </div>
</template>
