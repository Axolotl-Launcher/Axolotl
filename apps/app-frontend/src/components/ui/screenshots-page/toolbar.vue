<script setup lang="ts">
import { ArrowUpDownIcon, GridIcon, PlusIcon, SearchIcon } from '@modrinth/assets'
import {
    Button,
    Combobox,
    type ComboboxOption,
    defineMessages,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'

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
const messages = defineMessages({
    search: { id: 'app.screenshots.search', defaultMessage: 'Search' },
    newGroup: { id: 'app.screenshots.group.new', defaultMessage: 'New group' },
    sortBy: { id: 'app.screenshots.sort-by', defaultMessage: 'Sort by' },
    groupBy: { id: 'app.screenshots.group-by', defaultMessage: 'Group by' },
})
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
