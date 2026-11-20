<script setup lang="ts">
import { PlusIcon, XIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import effectsData from '@/lab/command-generator/data/effects.json'
import enchantmentsData from '@/lab/command-generator/data/enchantments.json'
import {
    emptyGroupRow,
    type NbtGroupColumn,
    type NbtGroupRow,
    type NbtGroupSpec,
    readGroupHeader,
    readGroupRows,
    writeGroupHeader,
    writeGroupRows,
} from '@/lab/command-generator/nbt/groups.ts'
import type { NbtTagNode } from '@/lab/command-generator/nbt/model.ts'

import CommandGeneratorNbtGroupCell from './CommandGeneratorNbtGroupCell.vue'

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
})

const rows = computed(() => readGroupRows(props.node, props.spec))
const header = computed(() =>
    props.spec.header?.length ? readGroupHeader(props.node, props.spec) : undefined,
)

const enchantmentOptions = enchantmentsData.enchantments as Array<{
    id: string
    en?: string
    zh?: string
}>

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

function setHeader(column: NbtGroupColumn, value: string) {
    writeGroupHeader(props.node, props.spec, { ...header.value, [column.key]: value })
    emit('change')
}

function addRow() {
    const next = emptyGroupRow(props.spec)
    const slotKey = props.spec.slotKey
    if (slotKey) {
        // 槽位是有限集合：默认挑一个还空着的，省得用户每次都要改
        const used = new Set(rows.value.map((row) => row[slotKey]))
        const free = props.spec.columns
            .find((column) => column.key === slotKey)
            ?.enumValues?.find((value) => !used.has(value))
        if (free) next[slotKey] = free
    }
    commit([...rows.value, next])
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
</script>

<template>
    <div class="flex min-w-0 flex-col gap-2">
        <div
            v-if="header"
            class="flex min-w-0 flex-wrap items-end gap-2 rounded-lg bg-surface-4 p-2"
        >
            <CommandGeneratorNbtGroupCell
                v-for="column in spec.header"
                :key="column.key"
                :column="column"
                :model-value="header[column.key]"
                @update:model-value="setHeader(column, $event)"
            />
        </div>

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
            <CommandGeneratorNbtGroupCell
                v-for="column in spec.columns"
                :key="column.key"
                :column="column"
                :datalist="datalistFor(column)"
                :model-value="row[column.key]"
                @update:model-value="setCell(rowIndex, column, $event)"
            />

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
