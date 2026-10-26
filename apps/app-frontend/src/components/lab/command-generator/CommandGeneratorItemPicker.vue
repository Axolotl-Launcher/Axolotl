<script setup lang="ts">
import { BoxIcon } from '@modrinth/assets'
import { defineMessages, StyledInput, useVIntl } from '@modrinth/ui'
import Fuse from 'fuse.js'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import RecipeItemIcon from '@/components/lab/recipe-generator/RecipeItemIcon.vue'
import type { SlotDisplay } from '@/lab/recipe-generator/display'
import type { SlotValue } from '@/lab/recipe-generator/types'
import type { TextureAtlas } from '@/lab/shared/minecraft/resources'

export type CommandItemEntry = {
    id: string
    name: string
    display?: SlotDisplay
    value: SlotValue
}

const props = defineProps<{
    entries: CommandItemEntry[]
    atlas: TextureAtlas
    loading: boolean
    kind?: 'item' | 'block' | 'effect' | 'advancement'
}>()

const emit = defineEmits<{
    pick: [value: SlotValue]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    searchPlaceholder: {
        id: 'app.lab.command-generator.item-picker.search-placeholder',
        defaultMessage: 'Search by name or ID',
    },
    loading: {
        id: 'app.lab.command-generator.item-picker.loading',
        defaultMessage: 'Loading…',
    },
    empty: {
        id: 'app.lab.command-generator.item-picker.empty',
        defaultMessage: 'No matches found.',
    },
    pickItem: {
        id: 'app.lab.command-generator.item-picker.button',
        defaultMessage: 'Pick item',
    },
    pickBlock: {
        id: 'app.lab.command-generator.block-picker.button',
        defaultMessage: 'Pick block',
    },
    pickEffect: {
        id: 'app.lab.command-generator.effect-picker.button',
        defaultMessage: 'Pick effect',
    },
    pickAdvancement: {
        id: 'app.lab.command-generator.advancement-picker.button',
        defaultMessage: 'Pick advancement',
    },
})

const search = ref('')
const debouncedSearch = ref('')

let searchTimer: ReturnType<typeof setTimeout> | undefined

watch(search, (value) => {
    if (searchTimer) clearTimeout(searchTimer)
    searchTimer = setTimeout(() => {
        debouncedSearch.value = value
    }, 120)
})

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

const actionText = computed(() => {
    switch (props.kind) {
        case 'block':
            return formatMessage(messages.pickBlock)
        case 'effect':
            return formatMessage(messages.pickEffect)
        case 'advancement':
            return formatMessage(messages.pickAdvancement)
        default:
            return formatMessage(messages.pickItem)
    }
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
            class="grid min-h-0 flex-1 grid-cols-[repeat(auto-fill,minmax(6rem,1fr))] auto-rows-max gap-2 overflow-y-auto overscroll-contain p-1"
        >
            <button
                v-for="entry in visibleEntries"
                :key="entry.id"
                v-tooltip="`${actionText}: ${entry.name} (${entry.id})`"
                type="button"
                class="flex min-w-0 cursor-pointer flex-col items-center gap-1 rounded-lg border border-solid border-surface-4 bg-surface-3 p-2 text-[var(--color-text-primary)] hover:border-brand focus-visible:outline-brand"
                :aria-label="`${actionText}: ${entry.name}`"
                @click="pick(entry.value)"
            >
                <RecipeItemIcon
                    v-if="entry.display"
                    :display="entry.display"
                    :atlas="atlas"
                    :size="34"
                    :show-count="false"
                />
                <div
                    v-else
                    class="flex items-center justify-center rounded-lg bg-surface-4"
                    style="width: 34px; height: 34px"
                >
                    <BoxIcon class="h-5 w-5 text-[var(--color-text-tertiary)]" />
                </div>
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
