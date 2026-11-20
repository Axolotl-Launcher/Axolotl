<script setup lang="ts">
import { PlusIcon, XIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, ref, useTemplateRef } from 'vue'

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
import { isVersionInRange } from '@/lab/command-generator/nbt/schema.ts'

import CommandGeneratorNbtGroupCell from './CommandGeneratorNbtGroupCell.vue'
import CommandGeneratorOptionPickerModal, {
    type PickerGroup,
} from './CommandGeneratorOptionPickerModal.vue'

const props = defineProps<{
    node: NbtTagNode
    spec: NbtGroupSpec
    /** 用于过滤掉该版本不存在的附魔 / 状态效果。 */
    version?: string
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
    pickerSearch: {
        id: 'app.lab.command-generator.nbt.group.picker.search',
        defaultMessage: 'Search by name or ID',
    },
    pickerEmpty: {
        id: 'app.lab.command-generator.nbt.group.picker.empty',
        defaultMessage: 'No entry matches.',
    },
    pickerAdded: {
        id: 'app.lab.command-generator.nbt.group.picker.added',
        defaultMessage: 'Added',
    },
    enchantmentTitle: {
        id: 'app.lab.command-generator.nbt.group.enchantment-picker.title',
        defaultMessage: 'Pick an enchantment',
    },
    effectTitle: {
        id: 'app.lab.command-generator.nbt.group.effect-picker.title',
        defaultMessage: 'Pick a status effect',
    },
})

/** 数据文件里的条目带上版本区间；缺字段时视为全版本可用。 */
interface VersionedEntry {
    id: string
    en?: string
    zh?: string
    since?: string
    until?: string
}

const rows = computed(() => readGroupRows(props.node, props.spec))
const header = computed(() =>
    props.spec.header?.length ? readGroupHeader(props.node, props.spec) : undefined,
)

const isChinese = computed(() => locale.value.toLowerCase().startsWith('zh'))
const resolvedVersion = computed(() => props.version ?? '1.21')

// 版本切换要跟着重算，所以是 computed 而不是 setup 期过滤一次
const enchantments = computed(() =>
    (enchantmentsData.enchantments as VersionedEntry[]).filter((entry) =>
        isVersionInRange(entry, resolvedVersion.value),
    ),
)
const effects = computed(() =>
    (effectsData as VersionedEntry[]).filter((entry) =>
        isVersionInRange(entry, resolvedVersion.value),
    ),
)

const picker = useTemplateRef<InstanceType<typeof CommandGeneratorOptionPickerModal>>('picker')
/** 正在等弹窗回填的单元格。保留到最后一次选择之后，避免弹窗关闭时列表先空掉。 */
const pending = ref<{ rowIndex: number; columnKey: string } | undefined>(undefined)

const pendingColumn = computed(() =>
    pending.value
        ? props.spec.columns.find((column) => column.key === pending.value?.columnKey)
        : undefined,
)

const pickerTitle = computed(() =>
    pendingColumn.value?.kind === 'effect'
        ? formatMessage(messages.effectTitle)
        : formatMessage(messages.enchantmentTitle),
)

const pickerGroups = computed<PickerGroup[]>(() => {
    const column = pendingColumn.value
    if (!column) return []

    if (column.kind === 'effect') {
        return [
            {
                id: 'effects',
                options: effects.value.map((entry) => ({
                    id: `minecraft:${entry.id}`,
                    label: `minecraft:${entry.id}`,
                    keywords: entry.id,
                })),
            },
        ]
    }

    if (column.kind === 'enchantment') {
        return [
            {
                id: 'enchantments',
                options: enchantments.value.map((entry) => ({
                    id: `minecraft:${entry.id}`,
                    label: `minecraft:${entry.id}`,
                    detail: isChinese.value ? entry.zh : entry.en,
                    keywords: `${entry.id} ${entry.en ?? ''} ${entry.zh ?? ''}`,
                })),
            },
        ]
    }

    return []
})

/** 已经写进这一组的 ID，列表上标出来，避免重复键。当前正在编辑的这一行不算。 */
const pickerTakenIds = computed(() => {
    const target = pending.value
    if (!target) return []
    return rows.value
        .filter((_, index) => index !== target.rowIndex)
        .map((row) => row[target.columnKey])
        .filter(Boolean)
})

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

function requestPick(rowIndex: number, column: NbtGroupColumn) {
    if (column.kind !== 'enchantment' && column.kind !== 'effect') return
    pending.value = { rowIndex, columnKey: column.key }
    picker.value?.show()
}

function onPicked(id: string) {
    const target = pending.value
    if (!target) return
    const column = props.spec.columns.find((entry) => entry.key === target.columnKey)
    if (column) setCell(target.rowIndex, column, id)
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
                :model-value="row[column.key]"
                @update:model-value="setCell(rowIndex, column, $event)"
                @request-pick="requestPick(rowIndex, column)"
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

        <CommandGeneratorOptionPickerModal
            ref="picker"
            :title="pickerTitle"
            :search-placeholder="formatMessage(messages.pickerSearch)"
            :empty-text="formatMessage(messages.pickerEmpty)"
            :groups="pickerGroups"
            :taken-ids="pickerTakenIds"
            :taken-label="formatMessage(messages.pickerAdded)"
            @pick="onPicked"
        />
    </div>
</template>
