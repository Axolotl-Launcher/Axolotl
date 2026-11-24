<script setup lang="ts">
import { ArrowUpDownIcon } from '@modrinth/assets'
import { computed } from 'vue'

import Button from '#ui/components/base/buttons/Button.vue'
import Combobox, { type ComboboxOption } from '#ui/components/base/Combobox.vue'
import Pagination from '#ui/components/base/Pagination.vue'
import PopoutMenu from '#ui/components/base/PopoutMenu.vue'
import { useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

import type { BrowseDisplayMode, BrowseDisplayModeOption } from '../types'
import BrowsePageSizeSelector from './BrowsePageSizeSelector.vue'

const props = withDefaults(
    defineProps<{
        sort: string
        sortOptions: ComboboxOption<string>[]
        pageSize: number
        pageSizeOptions?: number[]
        displayMode?: BrowseDisplayMode
        displayModeOptions?: BrowseDisplayModeOption[]
        displayModeTooltip?: string
        page: number
        pageCount: number
        pageCountKnown?: boolean
        cursorPaged?: boolean
        loading?: boolean
        disabled?: boolean
        variant?: 'app' | 'web'
    }>(),
    { loading: false, disabled: false, variant: 'app', pageCountKnown: true, cursorPaged: false },
)
const emit = defineEmits<{
    'update:sort': [sort: string]
    'update:pageSize': [size: number]
    'display-mode': [mode: BrowseDisplayMode]
    'switch-page': [page: number]
}>()
const { formatMessage } = useVIntl()
const currentSort = computed({
    get: () => props.sort,
    set: (sort: string) => emit('update:sort', sort),
})
const currentPageSize = computed({
    get: () => props.pageSize,
    set: (size: number) => emit('update:pageSize', size),
})
const selectedDisplayMode = computed(() =>
    props.displayModeOptions?.find((option) => option.id === props.displayMode),
)
</script>

<template>
    <Combobox
        v-model="currentSort"
        v-tooltip="{ content: formatMessage(commonMessages.sortByLabel), triggers: ['hover'] }"
        :disabled="disabled"
        width="content"
        :options="sortOptions"
        trigger-class="!min-h-10"
    >
        <template #prefix
            ><ArrowUpDownIcon class="size-5 shrink-0 text-[var(--color-text-default)]"
        /></template>
        <template #selected="{ label }"
            ><span class="font-semibold text-[var(--color-text-tertiary)]">{{
                label
            }}</span></template
        >
    </Combobox>
    <BrowsePageSizeSelector
        v-model="currentPageSize"
        :disabled="disabled"
        :options="pageSizeOptions"
    />
    <slot name="filters-button" />
    <PopoutMenu v-if="displayMode && displayModeOptions?.length" placement="bottom-end">
        <template #trigger>
            <Button
                v-tooltip="displayModeTooltip"
                circular
                icon-only
                :aria-label="displayModeTooltip"
            >
                <component :is="selectedDisplayMode?.icon" />
            </Button>
        </template>
        <template #menu>
            <div class="flex w-44 flex-col gap-1 p-1">
                <Button
                    v-for="option in displayModeOptions"
                    :key="option.id"
                    :type="displayMode === option.id ? 'base' : 'quiet'"
                    class="flex w-full items-center gap-2 !justify-start text-left"
                    :aria-pressed="displayMode === option.id"
                    @click="emit('display-mode', option.id)"
                >
                    <component :is="option.icon" class="size-4" />
                    <span>{{ option.label }}</span>
                </Button>
            </div>
        </template>
    </PopoutMenu>
    <slot name="display-mode-after" />
    <Pagination
        :page="page"
        :count="pageCount"
        :count-known="pageCountKnown"
        :cursor-paged="cursorPaged"
        :loading="loading"
        :class="variant === 'web' ? 'mx-auto sm:ml-auto sm:mr-0' : 'ml-auto'"
        @switch-page="emit('switch-page', $event)"
    />
</template>
