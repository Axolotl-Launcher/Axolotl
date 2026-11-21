<script setup lang="ts">
import { PlusIcon, SearchIcon, SparklesIcon } from '@modrinth/assets'
import { Button, Checkbox, defineMessages, NewModal, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue'

import { groupsFor } from '@/lab/command-generator/nbt/catalog.ts'
import {
    fieldDescription,
    fieldLabel,
    NBT_TYPE_NAME,
    NBT_TYPE_SHORT,
} from '@/lab/command-generator/nbt/labels.ts'
import {
    availableFields,
    formFor,
    formsFor,
    groupByCategory,
    keyForVersion,
    type NbtCategory,
    type NbtDomain,
    type NbtFieldSpec,
    type NbtValueForm,
    searchFields,
} from '@/lab/command-generator/nbt/schema.ts'

const props = defineProps<{
    /** 没有目录域时（`/data storage`）不列候选字段，只保留「自己写一个」。 */
    domain?: NbtDomain
    version: string
    /** 已知实体 / 物品 ID 时才能做「仅显示适用」过滤。 */
    targetId?: string
    /** 文档里已有的键名：列表上打个标记，并且不允许再加一条同名键。 */
    existingKeys?: string[]
    /** 这一层可选的字段：根层，或某个母项的子字段。 */
    fields: NbtFieldSpec[]
    /**
     * 这一层是列表时，元素的写法。列表的元素没有键名，也不能混型，
     * 所以面板只提供「添加元素」，不列元素字段让用户误以为是命名子项。
     */
    element?: NbtValueForm
    /** 正在往哪个标签里加子项，只用于标题。 */
    parentKey?: string
}>()

const emit = defineEmits<{
    /** `form` 是用户显式选的另一种写法；不传表示用默认写法。 */
    pick: [spec: NbtFieldSpec, form?: NbtValueForm]
    element: []
    manual: []
}>()

const { formatMessage, locale } = useVIntl()

/** 说明文字按域取 i18n 译文，没有译文时退回目录里的英文原文。 */
const describe = (spec: NbtFieldSpec) =>
    props.domain ? fieldDescription(spec, props.domain, formatMessage) : (spec.desc ?? '')

const messages = defineMessages({
    titleEntity: {
        id: 'app.lab.command-generator.nbt.palette.title.entity',
        defaultMessage: 'Add entity tag',
    },
    titleChild: {
        id: 'app.lab.command-generator.nbt.palette.title.child',
        defaultMessage: 'Add tag to {parent}',
    },
    titleItem: {
        id: 'app.lab.command-generator.nbt.palette.title.item',
        defaultMessage: 'Add item component',
    },
    titleBlockEntity: {
        id: 'app.lab.command-generator.nbt.palette.title.block-entity',
        defaultMessage: 'Add block entity tag',
    },
    titleElement: {
        id: 'app.lab.command-generator.nbt.palette.title.element',
        defaultMessage: 'Add entry to {parent}',
    },
    titleGeneric: {
        id: 'app.lab.command-generator.nbt.palette.title.generic',
        defaultMessage: 'Add tag',
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
    emptyChild: {
        id: 'app.lab.command-generator.nbt.palette.empty-child',
        defaultMessage: 'The catalog documents no sub-tags here. Add one by hand.',
    },
    added: {
        id: 'app.lab.command-generator.nbt.palette.added',
        defaultMessage: 'added',
    },
    manual: {
        id: 'app.lab.command-generator.nbt.palette.manual',
        defaultMessage: 'Add a custom tag by hand',
    },
    addElement: {
        id: 'app.lab.command-generator.nbt.palette.add-element',
        defaultMessage: 'Add {type} entry',
    },
    elementHint: {
        id: 'app.lab.command-generator.nbt.palette.element-hint',
        defaultMessage:
            'This tag is a list. Entries have no key of their own and must share one type.',
    },
    variantHint: {
        id: 'app.lab.command-generator.nbt.palette.variant-hint',
        defaultMessage: 'This tag has several accepted shapes. Pick one:',
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
    if (props.element) {
        return formatMessage(messages.titleElement, { parent: props.parentKey ?? '' })
    }
    if (props.parentKey) return formatMessage(messages.titleChild, { parent: props.parentKey })
    if (!props.domain) return formatMessage(messages.titleGeneric)
    if (props.domain === 'item') return formatMessage(messages.titleItem)
    if (props.domain === 'block_entity') return formatMessage(messages.titleBlockEntity)
    return formatMessage(messages.titleEntity)
})

const elementTypeName = computed(() =>
    props.element ? NBT_TYPE_NAME[props.element.type] : NBT_TYPE_NAME.compound,
)

const pool = computed(() => {
    if (!props.domain) return props.fields
    return availableFields(
        props.fields,
        { domain: props.domain, version: props.version, targetId: props.targetId },
        {
            onlyApplicable: onlyApplicable.value && Boolean(props.targetId),
            groups: groupsFor(props.domain),
        },
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

/** 子项面板里「一个子字段都没有」和「搜索没命中」是两件事，别用同一句话。 */
const emptyText = computed(() => {
    if (props.fields.length === 0) return formatMessage(messages.emptyChild)
    return formatMessage(messages.empty)
})

const existing = computed(() => new Set(props.existingKeys ?? []))

/**
 * 这个字段是不是已经在这一层了。
 *
 * 比对要用**当前版本**的键名：`ActiveEffects` 这类字段在旧版本叫别的名字，
 * 拿 `spec.key` 去比永远对不上，标记不出来就会让人加第二条。
 */
function isAdded(spec: NbtFieldSpec): boolean {
    return existing.value.has(keyForVersion(spec, props.version))
}

/**
 * 两个开关只在「拨了会变」的时候才出现。
 *
 * 目录里没有任何 `appliesTo`（早先的物品组件就是这样）时，「仅显示适用」拨了
 * 也不会少一行；没有 `advanced` 字段时，「显示高级标签」同理。留着就是假开关。
 */
const canFilterByTarget = computed(() => props.fields.some((field) => field.appliesTo))
const hasAdvanced = computed(() => props.fields.some((field) => field.advanced))

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

function pick(spec: NbtFieldSpec, form?: NbtValueForm) {
    emit('pick', spec, form)
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
    if (props.element) return
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

function addElement() {
    emit('element')
    modal.value?.hide()
}

/** 目录里列表字段带着元素类型，列表里顺便标出来。 */
function typeBadge(spec: NbtFieldSpec): string {
    const form = formFor(spec, props.version)
    if (!form) return ''
    if (form.type === 'list' && form.element) {
        return `[${NBT_TYPE_SHORT[form.element.type]}]`
    }
    return NBT_TYPE_SHORT[form.type]
}

function typeTitle(spec: NbtFieldSpec): string {
    const form = formFor(spec, props.version)
    if (!form) return ''
    if (form.type === 'list' && form.element) {
        return `List of ${NBT_TYPE_NAME[form.element.type]}`
    }
    return NBT_TYPE_NAME[form.type]
}

/** 同一字段的其它写法：`pattern` 既能是 ID 字符串，也能是图案定义复合标签。 */
function variants(spec: NbtFieldSpec): NbtValueForm[] {
    return formsFor(spec, props.version)
}

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
        <!--
            高度必须定死：列表要成为唯一滚动容器。给成 flex-1 而没有高度约束时，
            列表撑到内容高度、自己不滚，滚轮落在这块「假滚动容器」上会被
            overscroll-contain 截断，只剩弹窗内边距那一圈能滚。
            定高同时避免输入时弹窗跟着结果数量跳；复合标签里只有几个子字段时矮一些。
        -->
        <div
            class="flex min-h-0 min-w-0 flex-col gap-3"
            :class="element ? '' : 'h-[min(30rem,64vh)]'"
        >
            <template v-if="element">
                <p class="m-0 text-sm text-[var(--color-text-secondary)]">
                    {{ formatMessage(messages.elementHint) }}
                </p>
                <Button type="outlined" class="self-start" @click="addElement">
                    <PlusIcon class="h-4 w-4" />
                    {{ formatMessage(messages.addElement, { type: elementTypeName }) }}
                </Button>
            </template>

            <template v-else>
                <StyledInput
                    ref="searchInput"
                    v-model="search"
                    class="w-full"
                    :icon="SearchIcon"
                    :placeholder="formatMessage(messages.searchPlaceholder)"
                    clearable
                    @keydown="onKeydown"
                />

                <div
                    v-if="canFilterByTarget || hasAdvanced"
                    class="flex flex-wrap items-center gap-x-4 gap-y-1"
                >
                    <Checkbox
                        v-if="canFilterByTarget"
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
                        v-if="hasAdvanced"
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
                        {{ emptyText }}
                    </p>
                    <section v-for="group in groups" :key="group.category" class="mb-3">
                        <h3
                            class="sticky top-0 z-[1] bg-surface-3 px-1 py-1 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                        >
                            {{ group.label }}
                        </h3>
                        <button
                            v-for="spec in group.items"
                            :key="flatResults.indexOf(spec)"
                            type="button"
                            class="flex w-full min-w-0 items-start gap-3 rounded-lg px-2 py-2 text-left hover:bg-surface-4 focus-visible:outline-brand disabled:cursor-default disabled:hover:bg-transparent"
                            :class="{
                                'bg-surface-4': flatResults[activeIndex] === spec,
                            }"
                            :disabled="isAdded(spec)"
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
                                    <span
                                        class="truncate text-xs text-[var(--color-text-secondary)]"
                                        >{{ fieldLabel(spec, locale) }}</span
                                    >
                                    <span
                                        v-if="isAdded(spec)"
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
                                    v-if="describe(spec)"
                                    class="line-clamp-2 text-xs leading-4 text-[var(--color-text-tertiary)]"
                                    >{{ describe(spec) }}</span
                                >
                                <!--
                                    同一字段有多种合法写法时（`pattern` 是 ID 或图案定义），
                                    默认写法由整行点击触发，其余写法在这里显式选。
                                -->
                                <span
                                    v-if="variants(spec).length > 1"
                                    class="mt-1 flex min-w-0 flex-wrap items-center gap-1"
                                >
                                    <span
                                        class="text-[0.625rem] text-[var(--color-text-tertiary)]"
                                        >{{ formatMessage(messages.variantHint) }}</span
                                    >
                                    <button
                                        v-for="form in variants(spec)"
                                        :key="form.type + (form.desc ?? '')"
                                        type="button"
                                        class="rounded bg-surface-5 px-1.5 py-0.5 font-mono text-[0.625rem] text-[var(--color-text-secondary)] hover:bg-surface-4"
                                        :title="form.desc"
                                        @click.stop="pick(spec, form)"
                                    >
                                        {{ NBT_TYPE_SHORT[form.type] }}
                                    </button>
                                </span>
                            </span>
                        </button>
                    </section>
                </div>
            </template>

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
