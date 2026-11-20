<script setup lang="ts">
import { ChevronDownIcon, PlusIcon, XIcon } from '@modrinth/assets'
import {
    Button,
    Combobox,
    type ComboboxOption,
    defineMessages,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue'

import entitiesData from '@/lab/command-generator/data/entities.json'
import {
    argumentsForVariable,
    PLAYER_ONLY_ARGUMENTS,
    SELECTOR_GROUP_ORDER,
    SELECTOR_VARIABLES,
    type SelectorArgumentGroup,
    type SelectorArgumentSpec,
    selectorArgumentSpec,
    type SelectorVariable,
    suggestSelectorArguments,
} from '@/lab/command-generator/selector/arguments.ts'
import {
    SELECTOR_ISSUE_MESSAGE,
    selectorIssueMessages,
} from '@/lab/command-generator/selector/messages.ts'
import {
    completionContext,
    formatNumericRange,
    parseNumericRange,
    parseSelector,
    type SelectorIssue,
    validateSelector,
} from '@/lab/command-generator/selector/parse.ts'
import { nbtTargetFromSelector } from '@/lab/command-generator/selector/serialize.ts'

import CommandGeneratorOptionPickerModal, {
    type PickerGroup,
} from './CommandGeneratorOptionPickerModal.vue'

const props = defineProps<{
    modelValue?: string
    version?: string
    /** 用于生成无障碍标签，例如 `目标`。 */
    ariaLabel?: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
    /** 选择器内的 `type=` 变化时通知父组件，用来收窄 NBT 面板。 */
    'update:nbt-target': [targetId: string | undefined]
}>()

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
    inputLabel: {
        id: 'app.lab.command-generator.selector.input-label',
        defaultMessage: 'Target selector',
    },
    placeholder: {
        id: 'app.lab.command-generator.selector.placeholder',
        defaultMessage: '@e[type=zombie,distance=..10]',
    },
    filters: {
        id: 'app.lab.command-generator.selector.filters',
        defaultMessage: 'Filters',
    },
    addFilter: {
        id: 'app.lab.command-generator.selector.add-filter',
        defaultMessage: 'Add filter',
    },
    addFilterHint: {
        id: 'app.lab.command-generator.selector.add-filter-hint',
        defaultMessage: 'Write a target variable first, for example @e',
    },
    noFilters: {
        id: 'app.lab.command-generator.selector.no-filters',
        defaultMessage: 'No filters yet. Type in the box above, or add one.',
    },
    removeFilter: {
        id: 'app.lab.command-generator.selector.remove-filter',
        defaultMessage: 'Remove filter',
    },
    suggestions: {
        id: 'app.lab.command-generator.selector.suggestions',
        defaultMessage: 'Argument suggestions',
    },
    variable: {
        id: 'app.lab.command-generator.selector.variable',
        defaultMessage: 'Target variable',
    },
    playersOnlyHint: {
        id: 'app.lab.command-generator.selector.players-only-hint',
        defaultMessage: 'This filter only matches players, so the selector ignores other entities.',
    },
    min: { id: 'app.lab.command-generator.selector.min', defaultMessage: 'Min' },
    max: { id: 'app.lab.command-generator.selector.max', defaultMessage: 'Max' },
    value: { id: 'app.lab.command-generator.selector.value', defaultMessage: 'Value' },
    entitySearch: {
        id: 'app.lab.command-generator.selector.entity-search',
        defaultMessage: 'Entity id',
    },
    negationOn: {
        id: 'app.lab.command-generator.selector.negation-on',
        defaultMessage: 'Excludes matching targets',
    },
    negationOff: {
        id: 'app.lab.command-generator.selector.negation-off',
        defaultMessage: 'Includes matching targets',
    },
    pickEntity: {
        id: 'app.lab.command-generator.selector.pick-entity',
        defaultMessage: 'Pick',
    },
    filterPickerTitle: {
        id: 'app.lab.command-generator.selector.filter-picker.title',
        defaultMessage: 'Add a filter',
    },
    filterPickerSearch: {
        id: 'app.lab.command-generator.selector.filter-picker.search',
        defaultMessage: 'Search filters',
    },
    filterPickerEmpty: {
        id: 'app.lab.command-generator.selector.filter-picker.empty',
        defaultMessage: 'No filter matches.',
    },
    entityPickerTitle: {
        id: 'app.lab.command-generator.selector.entity-picker.title',
        defaultMessage: 'Pick an entity type',
    },
    entityPickerSearch: {
        id: 'app.lab.command-generator.selector.entity-picker.search',
        defaultMessage: 'Search by name or ID',
    },
    entityPickerEmpty: {
        id: 'app.lab.command-generator.selector.entity-picker.empty',
        defaultMessage: 'No entity matches.',
    },
    groupSpace: {
        id: 'app.lab.command-generator.selector.group.space',
        defaultMessage: 'Position',
    },
    groupScoreboard: {
        id: 'app.lab.command-generator.selector.group.scoreboard',
        defaultMessage: 'Scoreboard',
    },
    groupSpecies: {
        id: 'app.lab.command-generator.selector.group.species',
        defaultMessage: 'Species',
    },
    groupEntityData: {
        id: 'app.lab.command-generator.selector.group.entity-data',
        defaultMessage: 'Entity data',
    },
    groupPlayerData: {
        id: 'app.lab.command-generator.selector.group.player-data',
        defaultMessage: 'Player data',
    },
    groupLimit: {
        id: 'app.lab.command-generator.selector.group.limit',
        defaultMessage: 'Count',
    },
})

const GROUP_LABELS: Record<SelectorArgumentGroup, keyof typeof messages> = {
    space: 'groupSpace',
    scoreboard: 'groupScoreboard',
    species: 'groupSpecies',
    'entity-data': 'groupEntityData',
    'player-data': 'groupPlayerData',
    limit: 'groupLimit',
}

const VARIABLE_HINTS: Record<SelectorVariable, { en: string; zh: string }> = {
    p: { en: 'nearest player', zh: '最近的玩家' },
    r: { en: 'random player', zh: '随机玩家' },
    a: { en: 'all players', zh: '所有玩家' },
    e: { en: 'all entities', zh: '所有实体' },
    s: { en: 'this entity', zh: '执行者自己' },
    n: { en: 'nearest entity', zh: '最近的实体' },
}

const entities = entitiesData.entities as Array<{ id: string; en?: string; zh?: string }>

const inputRef = useTemplateRef<HTMLInputElement>('input')
const filterPicker =
    useTemplateRef<InstanceType<typeof CommandGeneratorOptionPickerModal>>('filterPicker')
const entityPicker =
    useTemplateRef<InstanceType<typeof CommandGeneratorOptionPickerModal>>('entityPicker')

const text = ref(props.modelValue ?? '@p')
const caret = ref(0)
const showSuggestions = ref(false)
const activeSuggestion = ref(0)
/** 正在等弹窗回填的卡片下标。 */
const entityPickIndex = ref<number | undefined>(undefined)

const isChinese = computed(() => locale.value.toLowerCase().startsWith('zh'))

const parsed = computed(() => parseSelector(text.value))

/**
 * 文本能认出的 Java 版变量。文本不是选择器、或变量不是 Java 版六个之一时为
 * `undefined`——此时不选中任何标签，也不给筛选候选，而不是假装它是 `@p`。
 */
const activeVariable = computed<SelectorVariable | undefined>(() =>
    parsed.value && !parsed.value.unknownVariable ? parsed.value.variable : undefined,
)

const issues = computed<SelectorIssue[]>(() => {
    if (!text.value.trim()) return []
    if (!parsed.value) return [{ level: 'error', id: 'not-a-selector' }]
    return validateSelector(parsed.value)
})
const errors = computed(() => issues.value.filter((issue) => issue.level === 'error'))
const warnings = computed(() => issues.value.filter((issue) => issue.level === 'warning'))

/** 已占用「取值相等」的参数名；取反过的出现不算占用。 */
const usedKeys = computed(
    () =>
        parsed.value?.arguments.filter((argument) => !argument.negated).map((arg) => arg.key) ?? [],
)

const completion = computed(() => {
    if (!showSuggestions.value) return undefined
    return completionContext(text.value, caret.value)
})

const suggestions = computed(() => {
    const context = completion.value
    if (!context?.insideBrackets || context.expectsValueFor) return []
    return suggestSelectorArguments(context.variable, context.prefix, context.usedKeys)
})

const addFilterGroups = computed(() => {
    if (!activeVariable.value) return []
    const available = argumentsForVariable(activeVariable.value).filter(
        (spec) => !usedKeys.value.includes(spec.key) || spec.repeatable,
    )
    return SELECTOR_GROUP_ORDER.map((group) => ({
        group,
        label: formatMessage(messages[GROUP_LABELS[group]]),
        specs: available.filter((spec) => spec.group === group),
    })).filter((entry) => entry.specs.length)
})

const filterPickerGroups = computed<PickerGroup[]>(() =>
    addFilterGroups.value.map((entry) => ({
        id: entry.group,
        label: entry.label,
        options: entry.specs.map((spec) => ({
            id: spec.key,
            label: `${spec.key}=`,
            detail: isChinese.value ? spec.zh : undefined,
            hint: isChinese.value ? spec.hint : undefined,
            keywords: `${spec.key} ${spec.zh} ${spec.hint}`,
        })),
    })),
)

const entityPickerGroups = computed<PickerGroup[]>(() => [
    {
        id: 'entities',
        options: entities.map((entry) => ({
            id: entry.id,
            label: entry.id,
            detail: isChinese.value ? entry.zh : entry.en,
            keywords: `${entry.id} ${entry.en ?? ''} ${entry.zh ?? ''}`,
        })),
    },
])

/** 已有参数在文本里的位置，用于就地改写而不重排整串。 */
const argumentCards = computed(() =>
    (parsed.value?.arguments ?? []).map((argument) => {
        const spec = selectorArgumentSpec(argument.key)
        return { argument, spec }
    }),
)

const playerOnlyWarning = computed(() => {
    const variable = activeVariable.value
    if (!variable || variable === 'a' || variable === 'p' || variable === 'r') return false
    return (parsed.value?.arguments ?? []).some((argument) =>
        PLAYER_ONLY_ARGUMENTS.includes(argument.key),
    )
})

const nbtTarget = computed(() => nbtTargetFromSelector(text.value))

// 外部改动（切换命令、套用预设）要同步回输入框，否则两边各说各话
watch(
    () => props.modelValue,
    (value) => {
        if (value !== undefined && value !== text.value) text.value = value
    },
)

watch(nbtTarget, (target) => emit('update:nbt-target', target), { immediate: true })

// 候选变了就把高亮拉回第一项，否则会停在一个已经不存在的下标上
watch(
    () => suggestions.value.length,
    () => {
        activeSuggestion.value = 0
    },
)

function issueText(issue: SelectorIssue): string {
    return formatMessage(selectorIssueMessages[SELECTOR_ISSUE_MESSAGE[issue.id]], issue.values)
}

function specLabel(spec: SelectorArgumentSpec): string {
    return isChinese.value ? spec.zh : spec.key
}

function variableHint(variableId: SelectorVariable): string {
    const hint = VARIABLE_HINTS[variableId]
    return isChinese.value ? hint.zh : hint.en
}

function publish(next: string) {
    text.value = next
    emit('update:modelValue', next)
}

function onInput(event: Event) {
    const target = event.target as HTMLInputElement
    caret.value = target.selectionStart ?? 0
    showSuggestions.value = true
    publish(target.value)
}

/** 光标移动或点击之后重新计算补全上下文。 */
function onCaretMove(event: Event) {
    const target = event.target as HTMLInputElement
    caret.value = target.selectionStart ?? 0
    showSuggestions.value = true
}

function onKeyup(event: KeyboardEvent) {
    if (['ArrowLeft', 'ArrowRight', 'Home', 'End', 'Tab', 'Escape', 'Enter'].includes(event.key))
        return
    onCaretMove(event)
}

function setVariable(next: SelectorVariable) {
    // 文本不是选择器时没有任何可保留的部分，直接重写，避免拼出 `@efoo`
    if (!parsed.value) {
        publish(`@${next}`)
        return
    }
    // 换变量后可能留下不合法的参数（例如 @a 上的 type），先保留让校验提示出来
    publish(`@${next}${text.value.trim().replace(/^@[A-Za-z_]+/, '')}`)
}

function focusInput(position?: number) {
    void nextTick(() => {
        const element = inputRef.value
        if (!element) return
        element.focus()
        const at = position ?? element.value.length
        element.setSelectionRange(at, at)
        caret.value = at
    })
}

function applySuggestion(spec: SelectorArgumentSpec) {
    const context = completion.value
    if (!context) return
    const insertion = `${spec.key}=`
    const prefixLength = context.prefix.length
    const at = caret.value
    const next = `${text.value.slice(0, at - prefixLength)}${insertion}${text.value.slice(at)}`
    showSuggestions.value = false
    publish(next)
    focusInput(at - prefixLength + insertion.length)
}

function onKeydown(event: KeyboardEvent) {
    if (!suggestions.value.length) return
    if (event.key === 'ArrowDown') {
        event.preventDefault()
        activeSuggestion.value = (activeSuggestion.value + 1) % suggestions.value.length
        return
    }
    if (event.key === 'ArrowUp') {
        event.preventDefault()
        activeSuggestion.value =
            (activeSuggestion.value - 1 + suggestions.value.length) % suggestions.value.length
        return
    }
    if (event.key === 'Tab' || (event.key === 'Enter' && suggestions.value.length)) {
        event.preventDefault()
        applySuggestion(suggestions.value[activeSuggestion.value])
        return
    }
    if (event.key === 'Escape') {
        showSuggestions.value = false
    }
}

/** 用新值替换某个参数，保留其它参数的原样写法。 */
function replaceValue(index: number, value: string) {
    const argument = parsed.value?.arguments[index]
    if (!argument) return
    const next = `${text.value.slice(0, argument.start)}${value}${text.value.slice(argument.end)}`
    publish(next)
}

function removeArgument(index: number) {
    const argument = parsed.value?.arguments[index]
    if (!argument) return
    let start = argument.start
    let end = argument.end
    const after = text.value.slice(end)
    if (after.startsWith(',')) {
        end += 1
    } else {
        const before = text.value.slice(0, start)
        const trimmed = before.replace(/,\s*$/, '')
        if (trimmed !== before) start = trimmed.length
    }
    publish(`${text.value.slice(0, start)}${text.value.slice(end)}`)
}

function addFilter(key: string) {
    const insertion = `${key}=`
    if (!text.value.includes('[')) {
        publish(`${text.value.trim()}[${insertion}]`)
        focusInput(text.value.length - 1)
        return
    }

    const close = text.value.lastIndexOf(']')
    // 方括号还没闭合：追加在末尾，且不替用户补 `]`
    if (close === -1) {
        const separator = /,\s*$/.test(text.value) ? '' : ','
        const next = `${text.value}${separator}${insertion}`
        publish(next)
        focusInput(next.length)
        return
    }

    const inner = text.value.slice(0, close)
    const separator = /\[\s*$/.test(inner) ? '' : ','
    const next = `${inner}${separator}${insertion}]`
    publish(next)
    focusInput(next.length - 1)
}

function openFilterPicker(event: MouseEvent) {
    filterPicker.value?.show(event)
}

function openEntityPicker(index: number, event: MouseEvent) {
    entityPickIndex.value = index
    entityPicker.value?.show(event)
}

function onEntityPicked(id: string) {
    const index = entityPickIndex.value
    entityPickIndex.value = undefined
    if (index === undefined) return
    const argument = parsed.value?.arguments[index]
    if (!argument) return
    replaceValue(index, argument.negated ? `!${id}` : id)
}

// ---------------------------------------------------------------- 值控件

function rangeOf(argument: { value: string }) {
    return parseNumericRange(argument.value)
}

function onRangeChange(index: number, side: 'min' | 'max', raw: string) {
    const argument = parsed.value?.arguments[index]
    if (!argument) return
    const range = rangeOf(argument) ?? {}
    const value = raw.trim() === '' ? undefined : Number(raw)
    const next = side === 'min' ? { ...range, min: value } : { ...range, max: value }
    replaceValue(index, formatNumericRange(next) || '0')
}

function onValueChange(index: number, raw: string) {
    replaceValue(index, raw)
}

function toggleNegation(index: number) {
    const argument = parsed.value?.arguments[index]
    if (!argument) return
    replaceValue(index, argument.negated ? argument.value.slice(1) : `!${argument.value}`)
}

function enumOptions(spec: SelectorArgumentSpec | undefined): ComboboxOption<string>[] {
    return (spec?.enumValues ?? []).map((value) => ({ value, label: value }))
}

function onEnumPick(index: number, value: unknown) {
    const argument = parsed.value?.arguments[index]
    if (!argument) return
    replaceValue(index, argument.negated ? `!${String(value)}` : String(value))
}

defineExpose({ focus: () => focusInput() })
</script>

<template>
    <div class="flex min-w-0 flex-col gap-3">
        <div
            class="flex flex-wrap items-center gap-1.5"
            role="group"
            :aria-label="formatMessage(messages.variable)"
        >
            <button
                v-for="variableId in SELECTOR_VARIABLES"
                :key="variableId"
                v-tooltip="variableHint(variableId)"
                type="button"
                class="rounded-md px-2 py-1 font-mono text-xs transition-colors"
                :class="
                    activeVariable === variableId
                        ? 'bg-brand text-brand-inverted font-semibold'
                        : 'bg-surface-4 text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'
                "
                :aria-pressed="activeVariable === variableId"
                @click="setVariable(variableId)"
            >
                @{{ variableId }}
            </button>
        </div>

        <div class="relative">
            <!--
                这里保留原生 input：补全要把光标放到 `key=` 之后，需要 selectionStart /
                setSelectionRange，StyledInput 没有暴露这两个操作。
            -->
            <input
                ref="input"
                class="w-full rounded-xl border-0 bg-surface-4 px-3 py-2 font-mono text-sm text-[var(--color-text-primary)] outline-none focus:ring-4 focus:ring-brand-shadow"
                :aria-label="ariaLabel ?? formatMessage(messages.inputLabel)"
                :placeholder="formatMessage(messages.placeholder)"
                :spellcheck="false"
                :value="text"
                @input="onInput"
                @keydown="onKeydown"
                @keyup="onKeyup"
                @click="onCaretMove"
                @blur="showSuggestions = false"
            />
            <ul
                v-if="suggestions.length"
                class="absolute left-0 top-full z-20 mt-1 max-h-64 w-full overflow-y-auto rounded-lg border border-solid border-surface-5 bg-surface-2 p-1 shadow-lg"
                role="listbox"
                :aria-label="formatMessage(messages.suggestions)"
            >
                <li v-for="(spec, index) in suggestions" :key="spec.key">
                    <button
                        type="button"
                        class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left"
                        :class="index === activeSuggestion ? 'bg-surface-4' : 'hover:bg-surface-3'"
                        role="option"
                        :aria-selected="index === activeSuggestion"
                        @mousedown.prevent="applySuggestion(spec)"
                    >
                        <span class="font-mono text-sm text-[var(--color-text-primary)]"
                            >{{ spec.key }}=</span
                        >
                        <span class="truncate text-xs text-[var(--color-text-tertiary)]">{{
                            specLabel(spec)
                        }}</span>
                    </button>
                </li>
            </ul>
        </div>

        <p
            v-for="issue in errors"
            :key="`${issue.id}-${issue.argument ?? ''}`"
            class="m-0 text-xs text-red"
        >
            {{ issueText(issue) }}
        </p>
        <p
            v-for="issue in warnings"
            :key="`${issue.id}-${issue.argument ?? ''}`"
            class="m-0 text-xs text-[var(--color-text-tertiary)]"
        >
            {{ issueText(issue) }}
        </p>
        <p v-if="playerOnlyWarning" class="m-0 text-xs text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.playersOnlyHint) }}
        </p>

        <div class="flex flex-wrap items-center justify-between gap-2">
            <span
                class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                >{{ formatMessage(messages.filters) }}</span
            >
            <Button
                v-tooltip="activeVariable ? undefined : formatMessage(messages.addFilterHint)"
                type="outlined"
                size="2xs"
                :disabled="!activeVariable"
                @click="openFilterPicker"
            >
                <PlusIcon class="h-3 w-3" />
                {{ formatMessage(messages.addFilter) }}
            </Button>
        </div>

        <p
            v-if="!argumentCards.length"
            class="m-0 rounded-lg bg-surface-3 px-3 py-2 text-xs text-[var(--color-text-tertiary)]"
        >
            {{ formatMessage(messages.noFilters) }}
        </p>

        <div v-else class="flex min-w-0 flex-col gap-2">
            <div
                v-for="(card, index) in argumentCards"
                :key="`${card.argument.key}-${index}`"
                class="flex min-w-0 flex-wrap items-end gap-2 rounded-lg bg-surface-3 p-2"
            >
                <div class="flex min-w-0 flex-1 flex-col gap-1">
                    <span class="font-mono text-xs text-[var(--color-text-secondary)]">{{
                        card.argument.key
                    }}</span>
                    <!-- 英文界面没有独立的参数名，重复显示一遍 key 没有意义 -->
                    <span
                        v-if="card.spec && specLabel(card.spec) !== card.argument.key"
                        class="truncate text-[0.625rem] text-[var(--color-text-tertiary)]"
                        >{{ specLabel(card.spec) }}</span
                    >
                </div>

                <!-- 区间 / 数字 -->
                <template
                    v-if="
                        card.spec &&
                        (card.spec.kind === 'range' ||
                            card.spec.kind === 'unsigned-range' ||
                            card.spec.kind === 'number')
                    "
                >
                    <StyledInput
                        class="w-24"
                        size="small"
                        inputmode="numeric"
                        :aria-label="formatMessage(messages.min)"
                        :placeholder="formatMessage(messages.min)"
                        :model-value="rangeOf(card.argument)?.min ?? ''"
                        @change="
                            onRangeChange(index, 'min', ($event.target as HTMLInputElement).value)
                        "
                    />
                    <StyledInput
                        class="w-24"
                        size="small"
                        inputmode="numeric"
                        :aria-label="formatMessage(messages.max)"
                        :placeholder="formatMessage(messages.max)"
                        :model-value="rangeOf(card.argument)?.max ?? ''"
                        @change="
                            onRangeChange(index, 'max', ($event.target as HTMLInputElement).value)
                        "
                    />
                </template>

                <!-- 枚举 -->
                <div
                    v-else-if="
                        card.spec && (card.spec.kind === 'enum' || card.spec.kind === 'gamemode')
                    "
                    class="min-w-0"
                >
                    <Combobox
                        :model-value="card.argument.value.replace(/^!/, '')"
                        :options="enumOptions(card.spec)"
                        width="full"
                        @update:model-value="onEnumPick(index, $event)"
                    />
                </div>

                <!-- 实体类型：输入框 + 弹窗选择 -->
                <template v-else-if="card.spec && card.spec.kind === 'entity-type'">
                    <StyledInput
                        class="min-w-0 flex-1"
                        size="small"
                        :aria-label="formatMessage(messages.entitySearch)"
                        :model-value="card.argument.value.replace(/^!/, '')"
                        @change="
                            onValueChange(
                                index,
                                card.argument.negated
                                    ? `!${($event.target as HTMLInputElement).value}`
                                    : ($event.target as HTMLInputElement).value,
                            )
                        "
                    />
                    <Button type="outlined" size="2xs" @click="openEntityPicker(index, $event)">
                        <ChevronDownIcon class="h-3 w-3" />
                        {{ formatMessage(messages.pickEntity) }}
                    </Button>
                </template>

                <!-- 其余按文本编辑 -->
                <StyledInput
                    v-else
                    class="min-w-0 flex-1"
                    size="small"
                    :aria-label="formatMessage(messages.value)"
                    :model-value="card.argument.value"
                    @change="onValueChange(index, ($event.target as HTMLInputElement).value)"
                />

                <Button
                    v-if="card.spec?.negatable"
                    v-tooltip="
                        card.argument.negated
                            ? formatMessage(messages.negationOn)
                            : formatMessage(messages.negationOff)
                    "
                    :type="card.argument.negated ? 'chip' : 'quiet'"
                    color="brand"
                    size="2xs"
                    :aria-pressed="card.argument.negated"
                    @click="toggleNegation(index)"
                >
                    !
                </Button>
                <Button
                    v-tooltip="formatMessage(messages.removeFilter)"
                    type="quiet"
                    size="2xs"
                    icon-only
                    :label="formatMessage(messages.removeFilter)"
                    @click="removeArgument(index)"
                >
                    <XIcon class="h-3 w-3" />
                </Button>
            </div>
        </div>

        <CommandGeneratorOptionPickerModal
            ref="filterPicker"
            :title="formatMessage(messages.filterPickerTitle)"
            :search-placeholder="formatMessage(messages.filterPickerSearch)"
            :empty-text="formatMessage(messages.filterPickerEmpty)"
            :groups="filterPickerGroups"
            @pick="addFilter"
        />
        <CommandGeneratorOptionPickerModal
            ref="entityPicker"
            :title="formatMessage(messages.entityPickerTitle)"
            :search-placeholder="formatMessage(messages.entityPickerSearch)"
            :empty-text="formatMessage(messages.entityPickerEmpty)"
            :groups="entityPickerGroups"
            @pick="onEntityPicked"
        />
    </div>
</template>
