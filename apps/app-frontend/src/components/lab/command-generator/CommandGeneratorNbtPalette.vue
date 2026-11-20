<script setup lang="ts">
import { PlusIcon, SearchIcon, SparklesIcon } from '@modrinth/assets'
import { Button, Checkbox, defineMessages, NewModal, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue'

import { availableFieldsFor, groupsFor } from '@/lab/command-generator/nbt/catalog.ts'
import {
    fieldDescription,
    fieldLabel,
    NBT_TYPE_NAME,
    NBT_TYPE_SHORT,
} from '@/lab/command-generator/nbt/labels.ts'
import {
    groupByCategory,
    type NbtCategory,
    type NbtDomain,
    type NbtFieldSpec,
    searchFields,
} from '@/lab/command-generator/nbt/schema.ts'

const props = defineProps<{
    domain: NbtDomain
    version: string
    /** 已知实体 / 物品 ID 时才能做「仅显示适用」过滤。 */
    targetId?: string
    /** 文档里已有的键名，列表上打个标记，不阻止再加一次。 */
    existingKeys?: string[]
    /** 复合标签内部再用一次：直接给出这一层可选的子字段。 */
    fields?: NbtFieldSpec[]
}>()

const emit = defineEmits<{
    pick: [spec: NbtFieldSpec]
    manual: []
}>()

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
    titleEntity: {
        id: 'app.lab.command-generator.nbt.palette.title.entity',
        defaultMessage: 'Add entity tag',
    },
    titleItem: {
        id: 'app.lab.command-generator.nbt.palette.title.item',
        defaultMessage: 'Add item component',
    },
    titleBlockEntity: {
        id: 'app.lab.command-generator.nbt.palette.title.block-entity',
        defaultMessage: 'Add block entity tag',
    },
    searchPlaceholder: {
        id: 'app.lab.command-generator.nbt.palette.search',
        defaultMessage: 'Search tags by name or description',
    },
    onlyApplicable: {
        id: 'app.lab.command-generator.nbt.palette.only-applicable',
        defaultMessage: 'Only tags that apply to {target}',
    },
    onlyApplicableUnknown: {
        id: 'app.lab.command-generator.nbt.palette.only-applicable-unknown',
        defaultMessage: 'Pick an entity first to filter by it',
    },
    showAdvanced: {
        id: 'app.lab.command-generator.nbt.palette.show-advanced',
        defaultMessage: 'Show advanced tags',
    },
    empty: {
        id: 'app.lab.command-generator.nbt.palette.empty',
        defaultMessage: 'No tag matches. Add it by hand instead.',
    },
    added: {
        id: 'app.lab.command-generator.nbt.palette.added',
        defaultMessage: 'added',
    },
    manual: {
        id: 'app.lab.command-generator.nbt.palette.manual',
        defaultMessage: 'Add a custom tag by hand',
    },
    categoryBasic: {
        id: 'app.lab.command-generator.nbt.category.basic',
        defaultMessage: 'General',
    },
    categoryAppearance: {
        id: 'app.lab.command-generator.nbt.category.appearance',
        defaultMessage: 'Appearance',
    },
    categoryCombat: {
        id: 'app.lab.command-generator.nbt.category.combat',
        defaultMessage: 'Combat',
    },
    categoryMovement: {
        id: 'app.lab.command-generator.nbt.category.movement',
        defaultMessage: 'Movement',
    },
    categoryAi: {
        id: 'app.lab.command-generator.nbt.category.ai',
        defaultMessage: 'Behaviour',
    },
    categoryInventory: {
        id: 'app.lab.command-generator.nbt.category.inventory',
        defaultMessage: 'Inventory',
    },
    categoryEffect: {
        id: 'app.lab.command-generator.nbt.category.effect',
        defaultMessage: 'Effects',
    },
    categoryData: {
        id: 'app.lab.command-generator.nbt.category.data',
        defaultMessage: 'Data',
    },
    categoryAdvanced: {
        id: 'app.lab.command-generator.nbt.category.advanced',
        defaultMessage: 'Advanced',
    },
})

const CATEGORY_LABELS: Record<NbtCategory, keyof typeof messages> = {
    basic: 'categoryBasic',
    appearance: 'categoryAppearance',
    combat: 'categoryCombat',
    movement: 'categoryMovement',
    ai: 'categoryAi',
    inventory: 'categoryInventory',
    effect: 'categoryEffect',
    data: 'categoryData',
    advanced: 'categoryAdvanced',
}

const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const search = ref('')
const onlyApplicable = ref(true)
const showAdvanced = ref(false)
const activeIndex = ref(0)
const searchInput = useTemplateRef<InstanceType<typeof StyledInput>>('searchInput')

const header = computed(() => {
    if (props.domain === 'item') return formatMessage(messages.titleItem)
    if (props.domain === 'block_entity') return formatMessage(messages.titleBlockEntity)
    return formatMessage(messages.titleEntity)
})

const pool = computed(() => {
    // 复合标签内部：候选就是这一层的子字段，版本过滤已经在生成目录时做过
    if (props.fields) return props.fields
    return availableFieldsFor(
        { domain: props.domain, version: props.version, targetId: props.targetId },
        { onlyApplicable: onlyApplicable.value && Boolean(props.targetId) },
    )
})

const results = computed(() => {
    const fields = showAdvanced.value ? pool.value : pool.value.filter((field) => !field.advanced)
    return searchFields(fields, search.value, props.version).map((result) => result.spec)
})

const groups = computed(() => {
    const byCategory = groupByCategory(results.value.map((spec) => ({ spec, score: 0 })))
    return byCategory.map((group) => ({
        category: group.category,
        label: formatMessage(messages[CATEGORY_LABELS[group.category]]),
        items: group.items,
    }))
})

/** 键盘上下移动时按展平后的顺序走，和视觉顺序一致。 */
const flatResults = computed(() => groups.value.flatMap((group) => group.items))

const existing = computed(() => new Set(props.existingKeys ?? []))

watch(results, () => {
    activeIndex.value = 0
})

function show() {
    search.value = ''
    onlyApplicable.value = Boolean(props.targetId)
    showAdvanced.value = false
    activeIndex.value = 0
    modal.value?.show()
    void nextTick(() => searchInput.value?.focus())
}

function hide() {
    modal.value?.hide()
}

function pick(spec: NbtFieldSpec) {
    emit('pick', spec)
    modal.value?.hide()
}

function pickActive() {
    const spec = flatResults.value[activeIndex.value]
    if (spec) pick(spec)
}

function move(delta: number) {
    const total = flatResults.value.length
    if (total === 0) return
    activeIndex.value = (activeIndex.value + delta + total) % total
}

function onKeydown(event: KeyboardEvent) {
    if (event.key === 'ArrowDown') {
        event.preventDefault()
        move(1)
        return
    }
    if (event.key === 'ArrowUp') {
        event.preventDefault()
        move(-1)
        return
    }
    if (event.key === 'Enter') {
        event.preventDefault()
        pickActive()
    }
}

function pickManual() {
    emit('manual')
    modal.value?.hide()
}

/** 目录里列表字段带着元素类型，列表里顺便标出来。 */
function typeBadge(spec: NbtFieldSpec): string {
    if (spec.type === 'list' && spec.elementType) {
        return `[${NBT_TYPE_SHORT[spec.elementType]}]`
    }
    return NBT_TYPE_SHORT[spec.type]
}

function typeTitle(spec: NbtFieldSpec): string {
    if (spec.type === 'list' && spec.elementType) {
        return `List of ${NBT_TYPE_NAME[spec.elementType]}`
    }
    return NBT_TYPE_NAME[spec.type]
}

/** 分组表只为实体与方块实体存在；物品组件的适用范围在组件本身。 */
const hasGroups = computed(() => Object.keys(groupsFor(props.domain)).length > 0)

defineExpose({ show, hide })
</script>

<template>
    <NewModal
        ref="modal"
        :header="header"
        width="min(640px, calc(100vw - 2rem))"
        max-width="640px"
        scrollable
        max-content-height="min(36rem, 78vh)"
    >
        <div class="flex min-h-0 min-w-0 flex-col gap-3">
            <StyledInput
                ref="searchInput"
                v-model="search"
                class="w-full"
                :icon="SearchIcon"
                :placeholder="formatMessage(messages.searchPlaceholder)"
                clearable
                @keydown="onKeydown"
            />

            <div class="flex flex-wrap items-center gap-x-4 gap-y-1">
                <Checkbox
                    v-if="!fields && (hasGroups || domain !== 'item')"
                    :model-value="onlyApplicable"
                    :disabled="!targetId"
                    :label="
                        targetId
                            ? formatMessage(messages.onlyApplicable, { target: targetId })
                            : formatMessage(messages.onlyApplicableUnknown)
                    "
                    @update:model-value="onlyApplicable = Boolean($event)"
                />
                <Checkbox
                    :model-value="showAdvanced"
                    :label="formatMessage(messages.showAdvanced)"
                    @update:model-value="showAdvanced = Boolean($event)"
                />
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
                <p
                    v-if="!flatResults.length"
                    class="px-1 py-6 text-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.empty) }}
                </p>
                <section v-for="group in groups" :key="group.category" class="mb-3">
                    <h3
                        class="sticky top-0 z-[1] bg-surface-3 px-1 py-1 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                    >
                        {{ group.label }}
                    </h3>
                    <button
                        v-for="spec in group.items"
                        :key="spec.key"
                        type="button"
                        class="flex w-full min-w-0 items-start gap-3 rounded-lg px-2 py-2 text-left hover:bg-surface-4 focus-visible:outline-brand"
                        :class="{
                            'bg-surface-4': flatResults[activeIndex]?.key === spec.key,
                        }"
                        @click="pick(spec)"
                        @mouseenter="activeIndex = flatResults.indexOf(spec)"
                    >
                        <span
                            v-tooltip="typeTitle(spec)"
                            class="mt-0.5 w-9 shrink-0 rounded bg-surface-5 px-1 py-0.5 text-center font-mono text-[0.625rem] leading-4 text-[var(--color-text-secondary)]"
                            >{{ typeBadge(spec) }}</span
                        >
                        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span class="flex min-w-0 flex-wrap items-baseline gap-2">
                                <span
                                    class="truncate font-mono text-sm text-[var(--color-text-primary)]"
                                    >{{ spec.key }}</span
                                >
                                <span class="truncate text-xs text-[var(--color-text-secondary)]">{{
                                    fieldLabel(spec, locale)
                                }}</span>
                                <span
                                    v-if="existing.has(spec.key)"
                                    class="rounded bg-surface-5 px-1 text-[0.625rem] text-[var(--color-text-tertiary)]"
                                    >{{ formatMessage(messages.added) }}</span
                                >
                                <span
                                    v-if="spec.since"
                                    class="rounded bg-surface-5 px-1 font-mono text-[0.625rem] text-[var(--color-text-tertiary)]"
                                    >{{ spec.since }}+</span
                                >
                            </span>
                            <span
                                v-if="fieldDescription(spec)"
                                class="line-clamp-2 text-xs leading-4 text-[var(--color-text-tertiary)]"
                                >{{ fieldDescription(spec) }}</span
                            >
                        </span>
                    </button>
                </section>
            </div>

            <div class="border-0 border-t border-solid border-surface-5 pt-3">
                <Button type="outlined" size="sm" class="w-full" @click="pickManual">
                    <PlusIcon class="h-4 w-4" />
                    {{ formatMessage(messages.manual) }}
                </Button>
                <p
                    class="mt-2 flex items-center justify-center gap-1 text-[0.625rem] text-[var(--color-text-tertiary)]"
                >
                    <SparklesIcon class="h-3 w-3" />
                    Minecraft Wiki
                </p>
            </div>
        </div>
    </NewModal>
</template>
