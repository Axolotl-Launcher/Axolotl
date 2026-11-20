<script setup lang="ts">
import { PlusIcon, XIcon } from '@modrinth/assets'
import { Button, Combobox, defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import effectsData from '@/lab/command-generator/data/effects.json'
import enchantmentsData from '@/lab/command-generator/data/enchantments.json'
import {
    emptyGroupRow,
    type NbtGroupColumn,
    type NbtGroupRow,
    type NbtGroupSpec,
    readGroupRows,
    writeGroupRows,
} from '@/lab/command-generator/nbt/groups.ts'
import type { NbtTagNode } from '@/lab/command-generator/nbt/model.ts'

const props = defineProps<{
    node: NbtTagNode
    spec: NbtGroupSpec
}>()

const emit = defineEmits<{
    change: []
}>()

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
    addRow: {
        id: 'app.lab.command-generator.nbt.group.add-row',
        defaultMessage: 'Add entry',
    },
    removeRow: {
        id: 'app.lab.command-generator.nbt.group.remove-row',
        defaultMessage: 'Remove entry',
    },
    empty: {
        id: 'app.lab.command-generator.nbt.group.empty',
        defaultMessage: 'No entries yet.',
    },
    idPlaceholder: {
        id: 'app.lab.command-generator.nbt.group.id-placeholder',
        defaultMessage: 'minecraft:…',
    },
})

const rows = computed(() => readGroupRows(props.node, props.spec))

const enchantmentOptions = (
    enchantmentsData.enchantments as Array<{
        id: string
        en?: string
        zh?: string
    }>
).map((entry) => entry)

const effectOptions = effectsData as Array<{ id: string; since: string }>

function isChinese(): boolean {
    return locale.value.toLowerCase().startsWith('zh')
}

function commit(next: NbtGroupRow[]) {
    writeGroupRows(props.node, props.spec, next)
    emit('change')
}

function setCell(rowIndex: number, column: NbtGroupColumn, value: string) {
    const next = rows.value.map((row, index) =>
        index === rowIndex ? { ...row, [column.key]: value } : row,
    )
    commit(next)
}

function addRow() {
    commit([...rows.value, emptyGroupRow(props.spec)])
}

function removeRow(rowIndex: number) {
    commit(rows.value.filter((_, index) => index !== rowIndex))
}

/** 附魔名有中英对照；状态效果只有 ID，写进指令的也是 ID。 */
function enchantmentLabel(id: string): string {
    const entry = enchantmentOptions.find((option) => option.id === id)
    const name = isChinese() ? entry?.zh : entry?.en
    return name ? `${name} (${id})` : id
}

/** 附魔 / 效果列：既可以从列表里挑，也可以直接写数据包里的自定义 ID。 */
function datalistFor(column: NbtGroupColumn): string | undefined {
    if (column.kind === 'enchantment') return 'nbt-group-enchantments'
    if (column.kind === 'effect') return 'nbt-group-effects'
    return undefined
}

function booleanOptions(): Array<{ value: string; label: string }> {
    return [
        { value: '1', label: isChinese() ? '是' : 'Yes' },
        { value: '0', label: isChinese() ? '否' : 'No' },
    ]
}

function comboboxOptions(column: NbtGroupColumn): Array<{ value: string; label: string }> {
    if (column.kind === 'boolean') return booleanOptions()
    return (column.enumValues ?? []).map((value) => ({ value, label: value }))
}
</script>

<template>
    <div class="flex min-w-0 flex-col gap-2">
        <p
            v-if="!rows.length"
            class="m-0 rounded-lg bg-surface-4 px-3 py-2 text-xs text-[var(--color-text-tertiary)]"
        >
            {{ formatMessage(messages.empty) }}
        </p>

        <div
            v-for="(row, rowIndex) in rows"
            :key="rowIndex"
            class="flex min-w-0 flex-wrap items-center gap-2 rounded-lg bg-surface-4 p-2"
        >
            <div
                v-for="column in spec.columns"
                :key="column.key"
                class="flex min-w-0 flex-col gap-0.5"
                :class="column.kind === 'number' ? 'w-20' : 'min-w-32 flex-1'"
            >
                <span class="text-[0.625rem] text-[var(--color-text-tertiary)]">{{
                    column.zh
                }}</span>

                <Combobox
                    v-if="column.kind === 'boolean' || column.kind === 'enum'"
                    :model-value="row[column.key]"
                    :options="comboboxOptions(column)"
                    width="full"
                    @update:model-value="setCell(rowIndex, column, String($event ?? ''))"
                />
                <input
                    v-else-if="column.kind === 'number'"
                    class="w-full rounded-md border-0 bg-surface-3 px-2 py-1 font-mono text-xs outline-none focus:ring-2 focus:ring-brand-shadow"
                    :aria-label="column.zh"
                    :min="column.min"
                    :max="column.max"
                    type="number"
                    :value="row[column.key]"
                    @change="setCell(rowIndex, column, ($event.target as HTMLInputElement).value)"
                />
                <input
                    v-else
                    class="w-full rounded-md border-0 bg-surface-3 px-2 py-1 font-mono text-xs outline-none focus:ring-2 focus:ring-brand-shadow"
                    :aria-label="column.zh"
                    :list="datalistFor(column)"
                    :placeholder="
                        column.kind === 'text' ? undefined : formatMessage(messages.idPlaceholder)
                    "
                    :value="row[column.key]"
                    @change="setCell(rowIndex, column, ($event.target as HTMLInputElement).value)"
                />
            </div>

            <Button
                v-tooltip="formatMessage(messages.removeRow)"
                type="quiet"
                size="2xs"
                icon-only
                :label="formatMessage(messages.removeRow)"
                @click="removeRow(rowIndex)"
            >
                <XIcon class="h-3 w-3" />
            </Button>
        </div>

        <Button type="outlined" size="2xs" class="self-start" @click="addRow">
            <PlusIcon class="h-3 w-3" />
            {{ formatMessage(messages.addRow) }}
        </Button>

        <datalist id="nbt-group-enchantments">
            <option v-for="entry in enchantmentOptions" :key="entry.id" :value="entry.id">
                {{ enchantmentLabel(entry.id) }}
            </option>
        </datalist>
        <datalist id="nbt-group-effects">
            <option v-for="entry in effectOptions" :key="entry.id" :value="entry.id" />
        </datalist>
    </div>
</template>
