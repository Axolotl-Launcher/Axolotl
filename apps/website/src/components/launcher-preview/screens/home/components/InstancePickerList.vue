<script setup lang="ts">
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import type { GameInstance } from '../../../lib/types'
import InstanceIcon from './InstanceIcon.vue'

/** Port of `components/ui/instance/InstancePickerList.vue`. */
const props = defineProps<{
    instances: GameInstance[]
    searchPlaceholder: string
    noInstancesMessage: string
    noMatchesMessage: string
    selectLabel: (instance: GameInstance) => string
}>()

const emit = defineEmits<{
    select: [instance: GameInstance]
}>()

const { locale } = useVIntl()
const searchInput = ref<InstanceType<typeof StyledInput>>()
const searchQuery = ref('')

const visibleInstances = computed(() => {
    const query = searchQuery.value.trim().toLocaleLowerCase(locale.value)
    return props.instances
        .filter((instance) => {
            if (!query) return true
            return [instance.name, instance.loader, instance.game_version].some((value) =>
                value.toLocaleLowerCase(locale.value).includes(query),
            )
        })
        .slice()
        .sort((a, b) => a.name.localeCompare(b.name, locale.value, { sensitivity: 'base' }))
})

function reset() {
    searchQuery.value = ''
}

function focus() {
    searchInput.value?.focus()
}

defineExpose({ reset, focus })
</script>

<template>
    <StyledInput
        v-if="instances.length > 0"
        ref="searchInput"
        v-model="searchQuery"
        type="search"
        :icon="SearchIcon"
        :placeholder="searchPlaceholder"
        wrapper-class="w-full"
        clearable
    />
    <ul v-if="visibleInstances.length > 0" class="m-0 flex list-none flex-col gap-1 p-0">
        <li v-for="instance in visibleInstances" :key="instance.id" class="min-w-0">
            <button
                type="button"
                class="flex min-h-16 w-full cursor-pointer items-center gap-3 rounded-lg border-0 bg-transparent px-3 py-2 text-left text-[var(--color-text-default)] transition-colors hover:bg-surface-4 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                :aria-label="selectLabel(instance)"
                @click="emit('select', instance)"
            >
                <InstanceIcon
                    class="size-10 shrink-0"
                    :icon-path="instance.icon_path"
                    :instance-id="instance.id"
                    :loader="instance.loader"
                />
                <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span class="truncate font-semibold text-[var(--color-text-primary)]">{{
                        instance.name
                    }}</span>
                    <span class="truncate text-sm capitalize text-[var(--color-text-tertiary)]">
                        {{ instance.loader }} {{ instance.game_version }}
                    </span>
                </span>
                <slot name="action" :instance="instance" />
            </button>
        </li>
    </ul>
    <EmptyState
        v-else
        type="empty-inbox"
        :heading="instances.length === 0 ? noInstancesMessage : noMatchesMessage"
    />
</template>
