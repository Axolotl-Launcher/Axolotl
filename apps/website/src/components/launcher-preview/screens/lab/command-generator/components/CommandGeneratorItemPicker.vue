<script setup lang="ts">
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import Fuse from '../../../../lib/fuzzy-search.ts'
import RecipeItemIcon from '../../recipe-generator/components/RecipeItemIcon.vue'
import type { SlotDisplay } from '../../recipe-generator/display.ts'
import type { TextureAtlas } from '../../recipe-generator/resources.ts'
import type { SlotValue } from '../../recipe-generator/types.ts'

export type CommandItemEntry = {
    id: string
    name: string
    display: SlotDisplay
    value: SlotValue
}

const props = defineProps<{
    entries: CommandItemEntry[]
    atlas: TextureAtlas
    loading: boolean
}>()

const emit = defineEmits<{
    pick: [value: SlotValue]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    searchPlaceholder: {
        id: 'app.lab.command-generator.item-picker.search-placeholder',
        defaultMessage: 'Search items by name or ID',
    },
    loading: {
        id: 'app.lab.command-generator.item-picker.loading',
        defaultMessage: 'Loading items',
    },
    empty: {
        id: 'app.lab.command-generator.item-picker.empty',
        defaultMessage: 'No items match your search.',
    },
    pickItem: {
        id: 'app.lab.command-generator.item-picker.pick',
        defaultMessage: 'Pick item',
    },
})

const search = ref('')
const debouncedSearch = ref('')

// 修复：使用 ReturnType<typeof setTimeout> 而非 window.setTimeout，避免 SSR/类型环境差异
let searchTimer: ReturnType<typeof setTimeout> | undefined

watch(search, (value) => {
    if (searchTimer) clearTimeout(searchTimer)
    searchTimer = setTimeout(() => {
        debouncedSearch.value = value
    }, 120)
})

// 修复：组件卸载时清理挂起的 timer
onBeforeUnmount(() => {
    if (searchTimer) {
        clearTimeout(searchTimer)
        searchTimer = undefined
    }
})

const fuse = computed(
    () =>
        new Fuse(props.entries, {
            keys: ['name', 'id'],
            threshold: 0.35,
            ignoreLocation: true,
        }),
)

const visibleEntries = computed(() => {
    const query = debouncedSearch.value.trim()
    if (!query) return props.entries
    return fuse.value.search(query).map((result) => result.item)
})

function pick(value: SlotValue) {
    emit('pick', value)
}
</script>

<template>
    <div class="item-picker flex min-h-0 min-w-0 flex-1 flex-col gap-2 p-3">
        <StyledInput
            v-model="search"
            :placeholder="formatMessage(messages.searchPlaceholder)"
            clearable
            class="w-full shrink-0"
        />
        <div
            v-if="loading"
            class="flex min-h-24 items-center justify-center text-sm text-[var(--color-text-tertiary)]"
        >
            {{ formatMessage(messages.loading) }}
        </div>
        <div
            v-else-if="!visibleEntries.length"
            class="flex min-h-24 items-center justify-center px-4 text-sm text-[var(--color-text-tertiary)]"
        >
            {{ formatMessage(messages.empty) }}
        </div>
        <div
            v-else
            class="grid min-h-0 flex-1 auto-rows-max grid-cols-[repeat(auto-fill,minmax(6rem,1fr))] gap-2 overflow-y-auto overscroll-contain p-1"
        >
            <button
                v-for="entry in visibleEntries"
                :key="entry.id"
                v-tooltip="`${formatMessage(messages.pickItem)}: ${entry.name} (${entry.id})`"
                type="button"
                class="flex min-w-0 cursor-pointer flex-col items-center gap-1 rounded-lg border border-solid border-surface-4 bg-surface-3 p-2 text-[var(--color-text-primary)] hover:border-brand focus-visible:outline-brand"
                :aria-label="`${formatMessage(messages.pickItem)}: ${entry.name}`"
                @click="pick(entry.value)"
            >
                <RecipeItemIcon
                    :display="entry.display"
                    :atlas="atlas"
                    :size="34"
                    :show-count="false"
                />
                <span class="w-full truncate text-center text-xs font-medium">{{
                    entry.name
                }}</span>
                <span
                    class="w-full truncate text-center font-mono text-[0.625rem] text-[var(--color-text-tertiary)]"
                    >{{ entry.id }}</span
                >
            </button>
        </div>
    </div>
</template>
