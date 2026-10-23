<script setup lang="ts">
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import type { ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref, watch } from 'vue'

import { commandGeneratorMessages as commonMessages } from '../i18n/messages.ts'
import { mergeItemNbt, type NbtTagNode, parseNbt, quoteSnbt } from '../nbt-tree.ts'

interface EnchantEntry {
    key: number
    id: string
    level: number
}

interface EnchantDefinition {
    id: string
    labelKey: string
    maxLevel: number
}

const props = defineProps<{
    modelValue?: string
    /** 目标游戏版本，如 '1.21'、'1.16'、'1.20' */
    version?: string
    /** 1.20.x 下由用户切换：是否使用 1.20.5+ 数据组件 */
    forceDataComponents?: boolean
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    title: {
        id: 'app.lab.command-generator.item-nbt.title',
        defaultMessage: 'Item customisation',
    },
    nameLabel: {
        id: 'app.lab.command-generator.item-nbt.name-label',
        defaultMessage: 'Custom name',
    },
    nameHint: {
        id: 'app.lab.command-generator.item-nbt.name-hint',
        defaultMessage: 'Shown as the item title.',
    },
    namePlaceholder: {
        id: 'app.lab.command-generator.item-nbt.name-placeholder',
        defaultMessage: 'Legendary sword',
    },
    loreLabel: {
        id: 'app.lab.command-generator.item-nbt.lore-label',
        defaultMessage: 'Lore lines',
    },
    loreHint: {
        id: 'app.lab.command-generator.item-nbt.lore-hint',
        defaultMessage: 'Each line appears under the item name.',
    },
    lorePlaceholder: {
        id: 'app.lab.command-generator.item-nbt.lore-placeholder',
        defaultMessage: 'A line of lore…',
    },
    addLore: {
        id: 'app.lab.command-generator.item-nbt.add-lore',
        defaultMessage: 'Add line',
    },
    enchLabel: {
        id: 'app.lab.command-generator.item-nbt.ench-label',
        defaultMessage: 'Enchantments',
    },
    enchHint: {
        id: 'app.lab.command-generator.item-nbt.ench-hint',
        defaultMessage: 'Pick an enchantment and set its level.',
    },
    enchAdd: {
        id: 'app.lab.command-generator.item-nbt.ench-add',
        defaultMessage: 'Add enchantment',
    },
    enchLevel: {
        id: 'app.lab.command-generator.item-nbt.ench-level',
        defaultMessage: 'Level',
    },
    enchEmpty: {
        id: 'app.lab.command-generator.item-nbt.ench-empty',
        defaultMessage: 'No enchantments yet.',
    },
    unbreakableLabel: {
        id: 'app.lab.command-generator.item-nbt.unbreakable-label',
        defaultMessage: 'Unbreakable',
    },
    unbreakableHint: {
        id: 'app.lab.command-generator.item-nbt.unbreakable-hint',
        defaultMessage: 'Prevents the item from taking durability damage.',
    },
    previewLabel: {
        id: 'app.lab.command-generator.item-nbt.preview-label',
        defaultMessage: 'NBT preview',
    },
    previewEmpty: {
        id: 'app.lab.command-generator.item-nbt.preview-empty',
        defaultMessage: 'Empty — no NBT will be applied.',
    },
})

const ENCHANTMENTS: EnchantDefinition[] = [
    {
        id: 'minecraft:sharpness',
        labelKey: 'app.lab.command-generator.enchantment.sharpness',
        maxLevel: 5,
    },
    { id: 'minecraft:smite', labelKey: 'app.lab.command-generator.enchantment.smite', maxLevel: 5 },
    {
        id: 'minecraft:bane_of_arthropods',
        labelKey: 'app.lab.command-generator.enchantment.bane-of-arthropods',
        maxLevel: 5,
    },
    {
        id: 'minecraft:knockback',
        labelKey: 'app.lab.command-generator.enchantment.knockback',
        maxLevel: 2,
    },
    {
        id: 'minecraft:fire_aspect',
        labelKey: 'app.lab.command-generator.enchantment.fire-aspect',
        maxLevel: 2,
    },
    {
        id: 'minecraft:looting',
        labelKey: 'app.lab.command-generator.enchantment.looting',
        maxLevel: 3,
    },
    {
        id: 'minecraft:sweeping_edge',
        labelKey: 'app.lab.command-generator.enchantment.sweeping-edge',
        maxLevel: 3,
    },
    {
        id: 'minecraft:efficiency',
        labelKey: 'app.lab.command-generator.enchantment.efficiency',
        maxLevel: 5,
    },
    {
        id: 'minecraft:silk_touch',
        labelKey: 'app.lab.command-generator.enchantment.silk-touch',
        maxLevel: 1,
    },
    {
        id: 'minecraft:unbreaking',
        labelKey: 'app.lab.command-generator.enchantment.unbreaking',
        maxLevel: 3,
    },
    {
        id: 'minecraft:fortune',
        labelKey: 'app.lab.command-generator.enchantment.fortune',
        maxLevel: 3,
    },
    {
        id: 'minecraft:mending',
        labelKey: 'app.lab.command-generator.enchantment.mending',
        maxLevel: 1,
    },
    { id: 'minecraft:power', labelKey: 'app.lab.command-generator.enchantment.power', maxLevel: 5 },
    { id: 'minecraft:punch', labelKey: 'app.lab.command-generator.enchantment.punch', maxLevel: 2 },
    { id: 'minecraft:flame', labelKey: 'app.lab.command-generator.enchantment.flame', maxLevel: 1 },
    {
        id: 'minecraft:infinity',
        labelKey: 'app.lab.command-generator.enchantment.infinity',
        maxLevel: 1,
    },
    {
        id: 'minecraft:multishot',
        labelKey: 'app.lab.command-generator.enchantment.multishot',
        maxLevel: 1,
    },
    {
        id: 'minecraft:quick_charge',
        labelKey: 'app.lab.command-generator.enchantment.quick-charge',
        maxLevel: 3,
    },
    {
        id: 'minecraft:piercing',
        labelKey: 'app.lab.command-generator.enchantment.piercing',
        maxLevel: 4,
    },
    {
        id: 'minecraft:protection',
        labelKey: 'app.lab.command-generator.enchantment.protection',
        maxLevel: 4,
    },
    {
        id: 'minecraft:fire_protection',
        labelKey: 'app.lab.command-generator.enchantment.fire-protection',
        maxLevel: 4,
    },
    {
        id: 'minecraft:feather_falling',
        labelKey: 'app.lab.command-generator.enchantment.feather-falling',
        maxLevel: 4,
    },
    {
        id: 'minecraft:blast_protection',
        labelKey: 'app.lab.command-generator.enchantment.blast-protection',
        maxLevel: 4,
    },
    {
        id: 'minecraft:projectile_protection',
        labelKey: 'app.lab.command-generator.enchantment.projectile-protection',
        maxLevel: 4,
    },
    {
        id: 'minecraft:respiration',
        labelKey: 'app.lab.command-generator.enchantment.respiration',
        maxLevel: 3,
    },
    {
        id: 'minecraft:aqua_affinity',
        labelKey: 'app.lab.command-generator.enchantment.aqua-affinity',
        maxLevel: 1,
    },
    {
        id: 'minecraft:thorns',
        labelKey: 'app.lab.command-generator.enchantment.thorns',
        maxLevel: 3,
    },
    {
        id: 'minecraft:depth_strider',
        labelKey: 'app.lab.command-generator.enchantment.depth-strider',
        maxLevel: 3,
    },
    {
        id: 'minecraft:frost_walker',
        labelKey: 'app.lab.command-generator.enchantment.frost-walker',
        maxLevel: 2,
    },
    {
        id: 'minecraft:binding_curse',
        labelKey: 'app.lab.command-generator.enchantment.binding-curse',
        maxLevel: 1,
    },
    {
        id: 'minecraft:vanishing_curse',
        labelKey: 'app.lab.command-generator.enchantment.vanishing-curse',
        maxLevel: 1,
    },
    {
        id: 'minecraft:soul_speed',
        labelKey: 'app.lab.command-generator.enchantment.soul-speed',
        maxLevel: 3,
    },
    {
        id: 'minecraft:swift_sneak',
        labelKey: 'app.lab.command-generator.enchantment.swift-sneak',
        maxLevel: 3,
    },
    {
        id: 'minecraft:loyalty',
        labelKey: 'app.lab.command-generator.enchantment.loyalty',
        maxLevel: 3,
    },
    {
        id: 'minecraft:impaling',
        labelKey: 'app.lab.command-generator.enchantment.impaling',
        maxLevel: 5,
    },
    {
        id: 'minecraft:riptide',
        labelKey: 'app.lab.command-generator.enchantment.riptide',
        maxLevel: 3,
    },
    {
        id: 'minecraft:channeling',
        labelKey: 'app.lab.command-generator.enchantment.channeling',
        maxLevel: 1,
    },
    {
        id: 'minecraft:luck_of_the_sea',
        labelKey: 'app.lab.command-generator.enchantment.luck-of-the-sea',
        maxLevel: 3,
    },
    { id: 'minecraft:lure', labelKey: 'app.lab.command-generator.enchantment.lure', maxLevel: 3 },
]

const enchantById = computed(() => new Map(ENCHANTMENTS.map((e) => [e.id, e])))

function enchantLabel(id: string): string {
    const def = enchantById.value.get(id)
    if (!def) return id
    return formatMessage({ id: def.labelKey, defaultMessage: def.labelKey })
}

const enchantOptions = computed<ComboboxOption<string>[]>(() =>
    ENCHANTMENTS.map((e) => ({
        value: e.id,
        label: enchantLabel(e.id),
    })),
)

const name = ref('')
const lore = ref<string[]>([])
const enchantments = ref<EnchantEntry[]>([])
const unbreakable = ref(false)
let nextKey = 1

// ============ 版本判断 ============

function parseVersion(v: string): number[] {
    return v.split('.').map((n) => {
        const num = Number.parseInt(n, 10)
        return Number.isFinite(num) ? num : 0
    })
}

/**
 * 决定最终使用数据组件（1.20.5+）还是旧版 NBT。
 *
 * 三档：
 * - 1.20.5 及以上（含 1.21+）：始终数据组件
 * - 1.20.x 但低于 1.20.5：
 *     - forceDataComponents 显式传了 → 跟随用户切换
 *     - 否则默认数据组件（1.20.x 用户大概率在用新语法）
 * - 1.16 ~ 1.19：始终旧版 NBT
 *
 * 注意 "1.20" 这种无小版本的字符串会走第二档，
 * 因为 manifest 里 1.20 这一项就是 "1.20"。
 */
const useDataComponents = computed(() => {
    const v = parseVersion(props.version ?? '1.21')

    const isAtLeast1205 =
        v[0] > 1 || (v[0] === 1 && v[1] > 20) || (v[0] === 1 && v[1] === 20 && (v[2] ?? 0) >= 5)
    if (isAtLeast1205) return true

    const is120Family = v[0] === 1 && v[1] === 20
    if (is120Family) {
        // 用户切换优先
        if (props.forceDataComponents !== undefined) {
            return props.forceDataComponents
        }
        // 默认走数据组件
        return true
    }

    return false
})

const useInlineText = computed(
    () => (props.version ?? '1.21').localeCompare('1.21.5', undefined, { numeric: true }) >= 0,
)

// ============ 序列化 ============

/**
 * SNBT 字符串内容转义。
 * - 单引号包裹：转义 \ 和 '
 * - 双引号包裹：转义 \ 和 "
 */
function escapeSnbt(value: string, quote: "'" | '"' = "'"): string {
    let result = value.replace(/\\/g, '\\\\')
    if (quote === "'") {
        result = result.replace(/'/g, "\\'")
    } else {
        result = result.replace(/"/g, '\\"')
    }
    return result
}

/**
 * 旧版 NBT 格式（1.20.4 及之前）
 * 使用 {display:{Name:...}} + Enchantments + Unbreakable
 */
function serializeLegacy(): string {
    const parts: string[] = []

    const displayParts: string[] = []
    if (name.value.trim()) {
        displayParts.push(`Name:'${escapeSnbt(JSON.stringify({ text: name.value }), "'")}'`)
    }
    const cleanLore = lore.value.map((l) => l.trim()).filter(Boolean)
    if (cleanLore.length) {
        const loreItems = cleanLore
            .map((l) => `'${escapeSnbt(JSON.stringify({ text: l }), "'")}'`)
            .join(',')
        displayParts.push(`Lore:[${loreItems}]`)
    }
    if (displayParts.length) {
        parts.push(`display:{${displayParts.join(',')}}`)
    }

    if (enchantments.value.length) {
        // 旧版附魔 lvl 是 short 类型，必须加 s 后缀
        const items = enchantments.value.map((e) => `{id:"${e.id}",lvl:${e.level}s}`).join(',')
        parts.push(`Enchantments:[${items}]`)
    }

    if (unbreakable.value) {
        parts.push('Unbreakable:1b')
    }

    return parts.length ? `{${parts.join(',')}}` : ''
}

/**
 * 新版数据组件格式（1.20.5+）
 * 使用 [custom_name=..., lore=..., enchantments={...}, unbreakable={}]
 */
function serializeDataComponents(): string {
    const parts: string[] = []

    if (name.value.trim()) {
        parts.push(
            useInlineText.value
                ? `custom_name={text:${quoteSnbt(name.value)}}`
                : `custom_name='${escapeSnbt(JSON.stringify({ text: name.value }), "'")}'`,
        )
    }
    const cleanLore = lore.value.map((l) => l.trim()).filter(Boolean)
    if (cleanLore.length) {
        const loreItems = cleanLore
            .map((l) =>
                useInlineText.value
                    ? `{text:${quoteSnbt(l)}}`
                    : `'${escapeSnbt(JSON.stringify({ text: l }), "'")}'`,
            )
            .join(',')
        parts.push(`lore=[${loreItems}]`)
    }

    if (enchantments.value.length) {
        // 新版附魔是键值对映射
        const enchMap = enchantments.value.map((e) => `"${e.id}":${e.level}`).join(',')
        parts.push(`enchantments={${enchMap}}`)
    }

    if (unbreakable.value) {
        parts.push('unbreakable={}')
    }

    return parts.length ? `[${parts.join(',')}]` : ''
}

const snbt = computed(() =>
    useDataComponents.value ? serializeDataComponents() : serializeLegacy(),
)

// ============ 双向同步 ============

let syncing = false

// 初始：从 prop 恢复内部状态
if (props.modelValue) {
    parseSnbt(props.modelValue)
}

watch(
    snbt,
    (val) => {
        if (syncing) return
        emit(
            'update:modelValue',
            mergeItemNbt(props.modelValue ?? '', val, useDataComponents.value),
        )
    },
    { flush: 'sync' },
)

watch(
    () => props.modelValue,
    (val) => {
        if (val === snbt.value) return
        syncing = true
        try {
            parseSnbt(val ?? '')
        } finally {
            syncing = false
        }
    },
)

// 版本或用户切换变化时，用新格式重新序列化并 emit
watch(
    () => [props.version, props.forceDataComponents],
    () => {
        emit(
            'update:modelValue',
            mergeItemNbt(props.modelValue ?? '', snbt.value, useDataComponents.value),
        )
    },
)

// ============ 反序列化 ============

function parseSnbt(snbtStr: string): void {
    name.value = ''
    lore.value = []
    enchantments.value = []
    unbreakable.value = false

    if (!snbtStr) return

    try {
        const { nodes, components } = parseNbt(snbtStr)
        const find = (list: NbtTagNode[], key: string) =>
            list.find((node) => node.name.replace(/^minecraft:/, '') === key)
        const readText = (node?: NbtTagNode): string => {
            if (node?.type === 'compound')
                return String(find(node.children ?? [], 'text')?.value ?? '')
            if (!node || node.type !== 'string') return ''
            try {
                const parsed = JSON.parse(String(node.value))
                return typeof parsed === 'string'
                    ? parsed
                    : typeof parsed?.text === 'string'
                      ? parsed.text
                      : ''
            } catch {
                return useInlineText.value ? String(node.value) : ''
            }
        }
        const display = components ? nodes : (find(nodes, 'display')?.children ?? [])
        name.value = readText(find(display, components ? 'custom_name' : 'Name'))
        lore.value = (find(display, components ? 'lore' : 'Lore')?.children ?? []).map(readText)
        const ench = find(nodes, components ? 'enchantments' : 'Enchantments')?.children ?? []
        enchantments.value = components
            ? (find(ench, 'levels')?.children ?? ench).map((node) => ({
                  key: nextKey++,
                  id: node.name,
                  level: Number(node.value),
              }))
            : ench.map((node) => ({
                  key: nextKey++,
                  id: String(find(node.children ?? [], 'id')?.value ?? ''),
                  level: Number(find(node.children ?? [], 'lvl')?.value ?? 1),
              }))
        unbreakable.value = components
            ? !!find(nodes, 'unbreakable')
            : Number(find(nodes, 'Unbreakable')?.value ?? 0) !== 0
    } catch {
        // Keep the supplied value intact when it cannot be represented by this form.
    }
}

// ============ 编辑操作 ============

function addLore() {
    lore.value.push('')
}

function removeLore(index: number) {
    lore.value.splice(index, 1)
}

function addEnchant() {
    const used = new Set(enchantments.value.map((e) => e.id))
    const next = ENCHANTMENTS.find((e) => !used.has(e.id))
    if (!next) return
    enchantments.value.push({ key: nextKey++, id: next.id, level: 1 })
}

function removeEnchant(index: number) {
    enchantments.value.splice(index, 1)
}

function setEnchantId(index: number, id: unknown) {
    const entry = enchantments.value[index]
    if (!entry) return
    entry.id = String(id)
    const def = enchantById.value.get(entry.id)
    if (def && entry.level > def.maxLevel) entry.level = def.maxLevel
}

function setEnchantLevel(index: number, level: number) {
    const entry = enchantments.value[index]
    if (!entry) return
    const def = enchantById.value.get(entry.id)
    const max = def?.maxLevel ?? 10
    const min = 1
    entry.level = Math.min(max, Math.max(min, Math.floor(level || min)))
}
</script>

<template>
    <div class="grid min-w-0 gap-4 md:grid-cols-2">
        <div class="flex min-w-0 flex-col gap-2">
            <label class="text-sm font-medium text-[var(--color-text-primary)]">{{
                formatMessage(messages.nameLabel)
            }}</label>
            <input
                v-model="name"
                class="text-input w-full min-w-0 rounded-xl border-0 bg-surface-4 px-3 py-2 text-sm text-[var(--color-text-primary)] outline-none focus:ring-4 focus:ring-brand-shadow"
                :placeholder="formatMessage(messages.namePlaceholder)"
            />
        </div>
        <div class="flex min-w-0 flex-col gap-2">
            <label class="text-sm font-medium text-[var(--color-text-primary)]">{{
                formatMessage(messages.loreLabel)
            }}</label>
            <div class="lore-list flex min-w-0 flex-col gap-2">
                <div
                    v-for="(_, i) in lore"
                    :key="i"
                    class="lore-row flex min-w-0 items-center gap-2"
                >
                    <input
                        v-model="lore[i]"
                        class="text-input w-full min-w-0 rounded-xl border-0 bg-surface-4 px-3 py-2 text-sm text-[var(--color-text-primary)] outline-none focus:ring-4 focus:ring-brand-shadow"
                        :placeholder="formatMessage(messages.lorePlaceholder)"
                    />
                    <Button
                        v-tooltip="formatMessage(commonMessages.removeLore)"
                        type="quiet"
                        size="sm"
                        icon-only
                        :label="formatMessage(commonMessages.removeLore)"
                        @click="removeLore(i)"
                        ><XIcon
                    /></Button>
                </div>
                <Button class="ghost-add self-start" type="outlined" size="sm" @click="addLore"
                    ><PlusIcon />{{ formatMessage(messages.addLore) }}</Button
                >
            </div>
        </div>
        <section class="flex min-w-0 flex-col gap-3 md:col-span-2">
            <h3 class="m-0 text-sm font-medium text-[var(--color-text-primary)]">
                {{ formatMessage(messages.enchLabel) }}
            </h3>
            <div
                v-for="(ench, i) in enchantments"
                :key="ench.key"
                class="flex min-w-0 flex-wrap items-center gap-2"
            >
                <Combobox
                    :model-value="ench.id"
                    :options="enchantOptions"
                    :aria-label="formatMessage(messages.enchLabel)"
                    class="min-w-0 flex-1"
                    searchable
                    @update:model-value="setEnchantId(i, $event)"
                />
                <label class="flex items-center gap-2 text-xs text-[var(--color-text-tertiary)]"
                    >{{ formatMessage(messages.enchLevel)
                    }}<input
                        class="text-input w-16 rounded-lg border-0 bg-surface-4 px-2 py-2 text-sm text-[var(--color-text-primary)] outline-none focus:ring-4 focus:ring-brand-shadow"
                        type="number"
                        min="1"
                        :max="enchantById.get(ench.id)?.maxLevel ?? 10"
                        :value="ench.level"
                        @input="
                            setEnchantLevel(i, Number(($event.target as HTMLInputElement).value))
                        "
                /></label>
                <Button
                    v-tooltip="formatMessage(commonMessages.removeEnchantment)"
                    type="quiet"
                    size="sm"
                    icon-only
                    :label="formatMessage(commonMessages.removeEnchantment)"
                    @click="removeEnchant(i)"
                    ><XIcon
                /></Button>
            </div>
            <Button type="outlined" size="sm" class="self-start" @click="addEnchant"
                ><PlusIcon />{{ formatMessage(messages.enchAdd) }}</Button
            >
        </section>
        <div class="flex items-center justify-between gap-3 md:col-span-2">
            <div>
                <label class="text-sm font-medium text-[var(--color-text-primary)]">{{
                    formatMessage(messages.unbreakableLabel)
                }}</label>
                <p class="m-0 mt-1 text-xs leading-5 text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.unbreakableHint) }}
                </p>
            </div>
            <Toggle v-model="unbreakable" :aria-label="formatMessage(messages.unbreakableLabel)" />
        </div>
        <details v-if="snbt" class="min-w-0 md:col-span-2">
            <summary class="cursor-pointer text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.previewLabel) }}
            </summary>
            <code
                class="mt-2 block max-h-48 overflow-auto break-all rounded-lg bg-surface-3 p-3 text-xs leading-5"
                >{{ snbt }}</code
            >
        </details>
    </div>
</template>
