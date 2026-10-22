<script setup lang="ts">
import '../../../../styles/overlays.css'

import { FilterIcon } from '@modrinth/assets'
import {
    TooltipArrow,
    TooltipContent,
    TooltipPortal,
    TooltipProvider,
    TooltipRoot,
    TooltipTrigger,
} from 'reka-ui'
import { onBeforeUnmount, ref } from 'vue'

import IconButton from '#ui/components/base/buttons/IconButton.vue'
import MultiSelect from '#ui/components/base/MultiSelect.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'

import { type MetadataFilterCategory, useHorizontalFilterScroll } from '../composables'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    searchPlaceholder: {
        id: 'content.metadata-filter.search',
        defaultMessage: 'Search...',
    },
    clear: {
        id: 'content.metadata-filter.clear',
        defaultMessage: 'Clear',
    },
    selectAll: {
        id: 'content.metadata-filter.select-all',
        defaultMessage: 'Select all',
    },
    filterToggle: {
        id: 'content.metadata-filter.toggle',
        defaultMessage: 'Filter',
    },
    filterToggleActive: {
        id: 'content.metadata-filter.toggle-active',
        defaultMessage: 'Filter ({count, number} active)',
    },
    longPressReset: {
        id: 'content.metadata-filter.long-press-reset',
        defaultMessage: 'Long-press to reset filters',
    },
})

const props = withDefaults(
    defineProps<{
        categories: MetadataFilterCategory[]
        modelValue: Record<string, string[]>
        filteringKeys?: string[]
        activeFilterCount?: number
    }>(),
    {
        filteringKeys: () => [],
        activeFilterCount: 0,
    },
)

const emit = defineEmits<{
    'update:category': [key: string, values: string[]]
}>()

const expanded = defineModel<boolean>('expanded', { default: false })

// ---- 长按重置筛选（仅在展开状态生效） ----

const LONG_PRESS_MS = 600
const RING_DELAY_MS = 120
const RING_FILL_MS = LONG_PRESS_MS - RING_DELAY_MS
let longPressTimer: ReturnType<typeof setTimeout> | null = null
let ringTimer: ReturnType<typeof setTimeout> | null = null
let longPressTriggered = false
const pressing = ref(false)

function resetAllFilters() {
    for (const category of props.categories) {
        emit(
            'update:category',
            category.key,
            category.options.map((option) => option.value),
        )
    }
}

function startLongPress() {
    if (!expanded.value) return
    longPressTriggered = false
    ringTimer = setTimeout(() => {
        ringTimer = null
        pressing.value = true
    }, RING_DELAY_MS)
    longPressTimer = setTimeout(() => {
        longPressTimer = null
        pressing.value = false
        longPressTriggered = true
        resetAllFilters()
    }, LONG_PRESS_MS)
}

function cancelLongPress() {
    if (ringTimer !== null) {
        clearTimeout(ringTimer)
        ringTimer = null
    }
    if (longPressTimer !== null) {
        clearTimeout(longPressTimer)
        longPressTimer = null
    }
    pressing.value = false
}

function handleToggleClick() {
    if (longPressTriggered) {
        longPressTriggered = false
        return
    }
    expanded.value = !expanded.value
}

onBeforeUnmount(() => {
    if (longPressTimer !== null) clearTimeout(longPressTimer)
    if (ringTimer !== null) clearTimeout(ringTimer)
})

const filterScrollRef = ref<HTMLElement | null>(null)
const scrollbarThumbRef = ref<HTMLElement | null>(null)
const { suppressHoverOpen, handleScroll } = useHorizontalFilterScroll(
    filterScrollRef,
    scrollbarThumbRef,
)

function selectedCount(category: MetadataFilterCategory): number {
    return (props.modelValue[category.key] ?? []).length
}

function isCategoryFiltering(category: MetadataFilterCategory): boolean {
    return props.filteringKeys.includes(category.key)
}

function filterButtonLabel(): string {
    if (expanded.value) return formatMessage(messages.longPressReset)
    if (props.activeFilterCount > 0) {
        return formatMessage(messages.filterToggleActive, { count: props.activeFilterCount })
    }
    return formatMessage(messages.filterToggle)
}
</script>

<template>
    <div class="group relative flex min-w-0 flex-1 items-center gap-1.5">
        <TooltipProvider>
            <TooltipRoot :delay-duration="0">
                <TooltipTrigger as-child>
                    <IconButton
                        class="relative"
                        :label="filterButtonLabel()"
                        :type="expanded || props.activeFilterCount > 0 ? 'chip' : 'quiet'"
                        :color="expanded || props.activeFilterCount > 0 ? 'brand' : undefined"
                        interaction="filled"
                        :aria-expanded="expanded"
                        :aria-pressed="expanded"
                        @click="handleToggleClick"
                        @pointerdown="startLongPress"
                        @pointerup="cancelLongPress"
                        @pointerleave="cancelLongPress"
                        @pointercancel="cancelLongPress"
                    >
                        <FilterIcon />
                        <span
                            v-if="props.activeFilterCount > 0"
                            aria-hidden="true"
                            class="absolute -right-2 -top-2 min-w-4 rounded-full bg-brand-highlight px-1 text-[0.625rem] font-semibold leading-4 text-brand"
                        >
                            {{ props.activeFilterCount }}
                        </span>
                    </IconButton>
                </TooltipTrigger>
                <TooltipPortal>
                    <TooltipContent
                        side="bottom"
                        :side-offset="6"
                        class="tooltip-surface filter-metadata-tooltip"
                    >
                        <div class="flex flex-col items-center gap-1">
                            <span class="whitespace-nowrap text-xs font-semibold">
                                {{ filterButtonLabel() }}
                            </span>
                            <div
                                v-if="pressing"
                                class="long-press-bar h-1 w-full min-w-[5rem] overflow-hidden rounded-full bg-surface-5"
                            >
                                <div
                                    class="long-press-bar-fill h-full rounded-full bg-brand"
                                    :style="{ animationDuration: RING_FILL_MS + 'ms' }"
                                />
                            </div>
                        </div>
                        <TooltipArrow class="tooltip-arrow" :width="14" :height="7" />
                    </TooltipContent>
                </TooltipPortal>
            </TooltipRoot>
        </TooltipProvider>

        <div
            class="grid min-w-0 flex-1 transition-[grid-template-columns] duration-300 ease-in-out"
            :class="expanded ? 'grid-cols-[1fr]' : 'grid-cols-[0fr]'"
        >
            <div class="relative min-w-0 overflow-hidden">
                <div
                    ref="filterScrollRef"
                    class="content-filter-scroll flex w-full min-w-0 flex-nowrap items-center gap-1.5 px-1.5"
                    @scroll="handleScroll"
                >
                    <MultiSelect
                        v-for="category in props.categories"
                        :key="category.key"
                        :model-value="props.modelValue[category.key] ?? []"
                        :options="category.options"
                        :max-height="420"
                        :clearable="false"
                        :show-chevron="false"
                        :fit-content="true"
                        :searchable="category.searchable"
                        :search-placeholder="formatMessage(messages.searchPlaceholder)"
                        :trigger-class="'h-8 shrink-0 !rounded-full border-0 px-2.5 transition-all hover:brightness-110 active:brightness-110'"
                        :active="selectedCount(category) === category.options.length"
                        :dropdown-min-width="'15rem'"
                        :checkbox-position="'left'"
                        :hover-open="!suppressHoverOpen"
                        show-selection-actions
                        :selection-actions-clear-label="formatMessage(messages.clear)"
                        :selection-actions-select-all-label="formatMessage(messages.selectAll)"
                        @update:model-value="
                            (values) => emit('update:category', category.key, values)
                        "
                    >
                        <template #input-content>
                            <span class="flex items-center gap-1.5 text-sm font-semibold">
                                <span class="truncate">{{ category.label }}</span>
                                <span
                                    v-if="isCategoryFiltering(category)"
                                    class="rounded-full bg-brand-highlight px-1.5 text-xs font-normal tabular-nums text-brand"
                                >
                                    {{ selectedCount(category) }}/{{ category.options.length }}
                                </span>
                            </span>
                        </template>
                    </MultiSelect>
                </div>

                <div
                    ref="scrollbarThumbRef"
                    class="pointer-events-none absolute bottom-0 left-0 z-10 h-[3px] rounded-full bg-surface-5 opacity-0 transition-opacity duration-150 group-hover:opacity-100"
                />
            </div>
        </div>
    </div>
</template>

<style scoped>
/* 隐藏原生滚动条（不占布局空间），滚动条由自绘悬浮条替代 */
.content-filter-scroll {
    overflow-x: auto;
    scrollbar-width: none;
    -ms-overflow-style: none;
}

.content-filter-scroll::-webkit-scrollbar {
    display: none;
}

.long-press-bar-fill {
    animation: long-press-bar-fill 480ms linear forwards;
}

@keyframes long-press-bar-fill {
    from {
        width: 0%;
    }
    to {
        width: 100%;
    }
}
</style>

<style>
.filter-metadata-tooltip {
    transition: none !important;
}
</style>
