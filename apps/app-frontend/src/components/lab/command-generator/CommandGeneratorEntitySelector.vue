<script setup lang="ts">
import { PlusIcon, XIcon } from '@modrinth/assets'
import { Button, Combobox, type ComboboxOption, defineMessages, useVIntl } from '@modrinth/ui'
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
    completionContext,
    formatNumericRange,
    parseNumericRange,
    parseSelector,
    validateSelector,
} from '@/lab/command-generator/selector/parse.ts'
import { nbtTargetFromSelector } from '@/lab/command-generator/selector/serialize.ts'

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
const text = ref(props.modelValue ?? '@p')
const caret = ref(0)
const showSuggestions = ref(false)
const activeSuggestion = ref(0)
const showAddFilter = ref(false)

const parsed = computed(() => parseSelector(text.value))
const issues = computed(() => (parsed.value ? validateSelector(parsed.value) : []))
const errors = computed(() => issues.value.filter((issue) => issue.level === 'error'))
const warnings = computed(() => issues.value.filter((issue) => issue.level === 'warning'))

const variable = computed<SelectorVariable>(() => parsed.value?.variable ?? 'p')

const usedKeys = computed(() => parsed.value?.arguments.map((argument) => argument.key) ?? [])

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
    const available = argumentsForVariable(variable.value).filter(
        (spec) => !usedKeys.value.includes(spec.key) || spec.repeatable,
    )
    return SELECTOR_GROUP_ORDER.map((group) => ({
        group,
        label: formatMessage(messages[GROUP_LABELS[group]]),
        specs: available.filter((spec) => spec.group === group),
    })).filter((entry) => entry.specs.length)
})

/** 已有参数在文本里的位置，用于就地改写而不重排整串。 */
const argumentCards = computed(() =>
    (parsed.value?.arguments ?? []).map((argument) => {
        const spec = selectorArgumentSpec(argument.key)
        return { argument, spec }
    }),
)

const playerOnlyWarning = computed(() => {
    if (variable.value === 'a' || variable.value === 'p' || variable.value === 'r') return false
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

function specLabel(spec: SelectorArgumentSpec): string {
    return locale.value.toLowerCase().startsWith('zh') ? spec.zh : spec.key
}

function variableHint(variableId: SelectorVariable): string {
    const hint = VARIABLE_HINTS[variableId]
    return locale.value.toLowerCase().startsWith('zh') ? hint.zh : hint.en
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
    const body = text.value.trim().replace(/^@[A-Za-z_]+/, '')
    // 换变量后可能留下不合法的参数（例如 @a 上的 type），先保留让校验提示出来
    publish(`@${next}${body}`)
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

function addFilter(spec: SelectorArgumentSpec) {
    showAddFilter.value = false
    const hasBrackets = text.value.includes('[')
    const insertion = `${spec.key}=`
    if (!hasBrackets) {
        publish(`${text.value}[${insertion}]`)
        focusInput(text.value.length - 1)
        return
    }
    const close = text.value.lastIndexOf(']')
    const inner = text.value.slice(0, close)
    const needsComma = !/\[\s*$/.test(inner)
    const next = `${inner}${needsComma ? ',' : ''}${insertion}]`
    publish(next)
    focusInput(next.length - 1)
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

const entityDatalistId = `entity-ids-${Math.random().toString(36).slice(2, 8)}`

function entityLabel(entry: { id: string; en?: string; zh?: string }): string {
    const name = locale.value.toLowerCase().startsWith('zh') ? entry.zh : entry.en
    return name ? `${name} (${entry.id})` : entry.id
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
                    variable === variableId
                        ? 'bg-brand text-brand-inverted font-semibold'
                        : 'bg-surface-4 text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'
                "
                :aria-pressed="variable === variableId"
                @click="setVariable(variableId)"
            >
                @{{ variableId }}
            </button>
        </div>

        <div class="relative">
            <input
                ref="input"
                class="w-full rounded-lg border-0 bg-surface-4 px-3 py-2 font-mono text-sm text-[var(--color-text-primary)] outline-none focus:ring-4 focus:ring-brand-shadow"
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

        <p v-for="issue in errors" :key="issue.message" class="m-0 text-xs text-red">
            {{ issue.message }}
        </p>
        <p
            v-for="issue in warnings"
            :key="issue.message"
            class="m-0 text-xs text-[var(--color-text-tertiary)]"
        >
            {{ issue.message }}
        </p>
        <p v-if="playerOnlyWarning" class="m-0 text-xs text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.playersOnlyHint) }}
        </p>

        <div class="flex flex-wrap items-center justify-between gap-2">
            <span
                class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                >{{ formatMessage(messages.filters) }}</span
            >
            <div class="relative">
                <Button type="outlined" size="2xs" @click="showAddFilter = !showAddFilter">
                    <PlusIcon class="h-3 w-3" />
                    {{ formatMessage(messages.addFilter) }}
                </Button>
                <div
                    v-if="showAddFilter"
                    class="absolute right-0 top-full z-30 mt-1 max-h-80 w-64 overflow-y-auto rounded-lg border border-solid border-surface-5 bg-surface-2 p-1 shadow-lg"
                >
                    <div v-for="entry in addFilterGroups" :key="entry.group">
                        <p
                            class="px-2 py-1 text-[0.625rem] uppercase text-[var(--color-text-tertiary)]"
                        >
                            {{ entry.label }}
                        </p>
                        <button
                            v-for="spec in entry.specs"
                            :key="spec.key"
                            type="button"
                            class="flex w-full flex-col rounded px-2 py-1.5 text-left hover:bg-surface-3"
                            @click="addFilter(spec)"
                        >
                            <span class="font-mono text-xs text-[var(--color-text-primary)]"
                                >{{ spec.key }}=</span
                            >
                            <span
                                class="truncate text-[0.625rem] text-[var(--color-text-tertiary)]"
                                >{{ spec.zh }}</span
                            >
                        </button>
                    </div>
                </div>
            </div>
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
                    <span class="truncate text-[0.625rem] text-[var(--color-text-tertiary)]">{{
                        card.spec ? card.spec.zh : card.argument.key
                    }}</span>
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
                    <input
                        class="w-20 rounded-md border-0 bg-surface-4 px-2 py-1.5 font-mono text-xs outline-none focus:ring-2 focus:ring-brand-shadow"
                        :aria-label="formatMessage(messages.min)"
                        :placeholder="formatMessage(messages.min)"
                        :value="rangeOf(card.argument)?.min ?? ''"
                        @change="
                            onRangeChange(index, 'min', ($event.target as HTMLInputElement).value)
                        "
                    />
                    <input
                        class="w-20 rounded-md border-0 bg-surface-4 px-2 py-1.5 font-mono text-xs outline-none focus:ring-2 focus:ring-brand-shadow"
                        :aria-label="formatMessage(messages.max)"
                        :placeholder="formatMessage(messages.max)"
                        :value="rangeOf(card.argument)?.max ?? ''"
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

                <!-- 实体类型 -->
                <template v-else-if="card.spec && card.spec.kind === 'entity-type'">
                    <input
                        class="min-w-0 flex-1 rounded-md border-0 bg-surface-4 px-2 py-1.5 font-mono text-xs outline-none focus:ring-2 focus:ring-brand-shadow"
                        :aria-label="formatMessage(messages.entitySearch)"
                        :list="entityDatalistId"
                        :value="card.argument.value.replace(/^!/, '')"
                        @change="
                            onValueChange(
                                index,
                                card.argument.negated
                                    ? `!${($event.target as HTMLInputElement).value}`
                                    : ($event.target as HTMLInputElement).value,
                            )
                        "
                    />
                </template>

                <!-- 其余按文本编辑 -->
                <input
                    v-else
                    class="min-w-0 flex-1 rounded-md border-0 bg-surface-4 px-2 py-1.5 font-mono text-xs outline-none focus:ring-2 focus:ring-brand-shadow"
                    :aria-label="formatMessage(messages.value)"
                    :value="card.argument.value"
                    @change="onValueChange(index, ($event.target as HTMLInputElement).value)"
                />

                <Button
                    v-if="card.spec?.negatable"
                    type="quiet"
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

        <datalist :id="entityDatalistId">
            <option v-for="entry in entities" :key="entry.id" :value="entry.id">
                {{ entityLabel(entry) }}
            </option>
        </datalist>
    </div>
</template>
