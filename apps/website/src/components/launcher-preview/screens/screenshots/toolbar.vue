<script setup lang="ts">
import ArrowUpDownIcon from '@modrinth/assets/icons/arrow-up-down.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import type { ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

import { messages } from './messages'

const search = defineModel<string>('search', { required: true })
const sort = defineModel<string>('sort', { required: true })
const group = defineModel<string>('group', { required: true })

defineProps<{
    sortOptions: ComboboxOption<string>[]
    groupOptions: ComboboxOption<string>[]
}>()

const emit = defineEmits<{
    (e: 'new-group'): void
}>()

const { formatMessage } = useVIntl()
</script>

<template>
    <div class="flex flex-col gap-2">
        <div class="flex flex-wrap gap-2">
            <StyledInput
                v-model="search"
                :icon="SearchIcon"
                type="text"
                :placeholder="formatMessage(messages.search)"
                clearable
                wrapper-class="min-w-[16rem] flex-1"
            />
            <Button @click="emit('new-group')">
                <PlusIcon />
                {{ formatMessage(messages.newGroup) }}
            </Button>
        </div>
        <div class="flex flex-wrap items-center gap-2">
            <Combobox
                v-model="sort"
                width="content"
                :options="sortOptions"
                :show-icon-in-selected="false"
                dropdown-min-width="160px"
            >
                <template #prefix>
                    <ArrowUpDownIcon
                        class="size-5 text-[var(--color-text-default)]"
                        :aria-label="formatMessage(messages.sortBy)"
                    />
                </template>
                <template #selected="{ label }">
                    <span>{{ label }}</span>
                </template>
            </Combobox>
            <Combobox
                v-model="group"
                width="content"
                :options="groupOptions"
                :show-icon-in-selected="false"
                dropdown-min-width="160px"
            >
                <template #prefix>
                    <GridIcon
                        class="size-5 text-[var(--color-text-default)]"
                        :aria-label="formatMessage(messages.groupBy)"
                    />
                </template>
                <template #selected="{ label }">
                    <span>{{ label }}</span>
                </template>
            </Combobox>
        </div>
    </div>
</template>
