<script setup lang="ts">
import { ChevronDownIcon, ChevronUpIcon, GripVerticalIcon, XIcon } from '@modrinth/assets'
import { Button, Combobox, type ComboboxOption, defineMessages, useVIntl } from '@modrinth/ui'
import { usePreferredReducedMotion, useTimeoutFn } from '@vueuse/core'
import { type Component, computed, nextTick, ref, watch } from 'vue'
import Draggable from 'vuedraggable'

import { commandCatalogMessage } from '@/lab/command-generator/catalog-messages'
import { moveChainItem } from '@/lab/command-generator/chain-order'
import type {
    ExecuteChainCategory,
    ExecuteChainSubcommand,
    ExecuteChainVariants,
} from '@/lab/command-generator/commands/types.ts'
import { subcommandIcon, variantIcon } from '@/lab/command-generator/subcommand-icons'
import {
    activeStepArguments,
    migrateLegacyStep,
    resolveVariant,
    stepTemplate,
    switchVariant,
} from '@/lab/command-generator/variants'

import CommandGeneratorArgument from './CommandGeneratorArgument.vue'

interface ChainStep {
    id: number
    subcommandId: string
    values: Record<string, string | number | boolean>
    expanded: boolean
}

export interface ExecuteChainState {
    chain: Array<{
        subcommandId: string
        values: Record<string, string | number | boolean>
    }>
    finalCommand: string
}

const props = defineProps<{
    categories: ExecuteChainCategory[]
    modelValue: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    executeQuickExamples: {
        id: 'app.lab.command-generator.execute.quick-examples',
        defaultMessage: 'Quick examples',
    },
    executeChainTitle: {
        id: 'app.lab.command-generator.execute.chain-title',
        defaultMessage: 'Subcommand chain',
    },
    executeChainEmpty: {
        id: 'app.lab.command-generator.execute.chain-empty',
        defaultMessage: 'Click a subcommand below to add it to the chain.',
    },
    executeClear: { id: 'app.lab.command-generator.execute.clear', defaultMessage: 'Clear' },
    executeFinalTitle: {
        id: 'app.lab.command-generator.execute.final-title',
        defaultMessage: 'Command to run',
    },
    executeFinalPlaceholder: {
        id: 'app.lab.command-generator.execute.final-placeholder',
        defaultMessage: 'say hello',
    },
    executeFinalHint: {
        id: 'app.lab.command-generator.execute.final-hint',
        defaultMessage: 'Type the actual command without the leading slash.',
    },
    executePickerTitle: {
        id: 'app.lab.command-generator.execute.picker-title',
        defaultMessage: 'Available subcommands',
    },
    variantPlaceholder: {
        id: 'app.lab.command-generator.execute.variant.placeholder',
        defaultMessage: 'Choose one',
    },
    variantUnset: {
        id: 'app.lab.command-generator.execute.variant.unset',
        defaultMessage: 'No form picked yet',
    },
    variantCount: {
        id: 'app.lab.command-generator.execute.variant.count',
        defaultMessage: '{count} forms',
    },
    moveUp: { id: 'app.lab.command-generator.execute.move-up', defaultMessage: 'Move up' },
    moveDown: { id: 'app.lab.command-generator.execute.move-down', defaultMessage: 'Move down' },
    removeStep: {
        id: 'app.lab.command-generator.execute.remove-step',
        defaultMessage: 'Remove subcommand',
    },
    quickPresetAllSayHello: {
        id: 'app.lab.command-generator.execute.quick.all-say-hello',
        defaultMessage: 'All players say hello',
    },
    quickPresetCheckZombies: {
        id: 'app.lab.command-generator.execute.quick.check-zombies',
        defaultMessage: 'Check for zombies',
    },
    quickPresetKillNearbyZombies: {
        id: 'app.lab.command-generator.execute.quick.kill-nearby-zombies',
        defaultMessage: 'Kill nearby zombies and count',
    },
    quickPresetGrassBelow: {
        id: 'app.lab.command-generator.execute.quick.grass-below',
        defaultMessage: 'Say when grass is below',
    },
    quickPresetMoveForward: {
        id: 'app.lab.command-generator.execute.quick.move-forward',
        defaultMessage: 'All entities move forward',
    },
})

function formatDynamic(id: string, fallback = ''): string {
    return formatMessage(commandCatalogMessage(id, fallback))
}

let nextId = 1

const chain = ref<ChainStep[]>([])
const finalCommand = ref('')
const finalInputRef = ref<HTMLInputElement | null>(null)
const chainSectionRef = ref<HTMLElement | null>(null)
const dragging = ref(false)
const nudgeState = ref<{ id: number; direction: 'up' | 'down' } | null>(null)
const reducedMotion = usePreferredReducedMotion()
const { start: resetNudge } = useTimeoutFn(
    () => {
        nudgeState.value = null
    },
    220,
    { immediate: false },
)

const QUICK_PRESETS = [
    {
        label: messages.quickPresetAllSayHello,
        build: () => {
            chain.value = [
                { id: nextId++, subcommandId: 'as', values: { target: '@a' }, expanded: false },
                { id: nextId++, subcommandId: 'at', values: { target: '@s' }, expanded: false },
            ]
            finalCommand.value = 'say hello'
        },
    },
    {
        label: messages.quickPresetCheckZombies,
        build: () => {
            chain.value = [
                {
                    id: nextId++,
                    subcommandId: 'if',
                    values: { condition: 'entity', target: '@e[type=zombie]' },
                    expanded: false,
                },
            ]
            finalCommand.value = 'say zombies'
        },
    },
    {
        label: messages.quickPresetKillNearbyZombies,
        build: () => {
            chain.value = [
                {
                    id: nextId++,
                    subcommandId: 'store',
                    values: {
                        mode: 'result',
                        destination: 'score',
                        target: '@s',
                        objective: 'kills',
                    },
                    expanded: false,
                },
            ]
            finalCommand.value = 'kill @e[type=zombie,distance=..10]'
        },
    },
    {
        label: messages.quickPresetGrassBelow,
        build: () => {
            chain.value = [
                { id: nextId++, subcommandId: 'as', values: { target: '@s' }, expanded: false },
                { id: nextId++, subcommandId: 'at', values: { target: '@s' }, expanded: false },
                {
                    id: nextId++,
                    subcommandId: 'if',
                    values: {
                        condition: 'block',
                        pos: '~ ~-1 ~',
                        block: 'minecraft:grass_block',
                    },
                    expanded: false,
                },
            ]
            finalCommand.value = 'say grass below'
        },
    },
    {
        label: messages.quickPresetMoveForward,
        build: () => {
            chain.value = [
                { id: nextId++, subcommandId: 'as', values: { target: '@e' }, expanded: false },
                { id: nextId++, subcommandId: 'at', values: { target: '@s' }, expanded: false },
            ]
            finalCommand.value = 'tp ^ ^ ^1'
        },
    },
]

/** 命令名本身不翻译，label 就是命令名。 */
const QUICK_FINAL_COMMANDS = [
    'say',
    'tellraw',
    'title',
    'tp',
    'kill',
    'give',
    'summon',
    'setblock',
    'fill',
    'effect',
    'playsound',
    'particle',
    'scoreboard',
    'tag',
]

const subcommandMap = computed(() => {
    const map = new Map<string, ExecuteChainSubcommand>()
    for (const cat of props.categories) {
        for (const sub of cat.subcommands) {
            map.set(sub.id, sub)
        }
    }
    return map
})

function renderStep(step: ChainStep): string {
    const sub = subcommandMap.value.get(step.subcommandId)
    if (!sub) return ''
    const template = stepTemplate(sub, step.values)
    if (!template) return ''
    const args = new Map(activeStepArguments(sub, step.values).map((arg) => [arg.name, arg]))
    // 占位符是空格分隔的独立 token；当前条件下不存在的、以及还没填的，连同它的空格一起省掉，
    // 否则 `if items block` 会因为被跳过的 `{target}` 留下双空格。
    return template
        .split(' ')
        .map((token) => {
            const match = /^\{(\w+)\}$/.exec(token)
            if (!match) return token
            const arg = args.get(match[1])
            if (!arg) return ''
            const value = step.values[arg.name]
            return value === undefined || value === null || value === '' ? '' : String(value)
        })
        .filter((token) => token !== '')
        .join(' ')
}

/** 条件步骤当前跟的判断；普通子命令返回 undefined。 */
function variantsOf(step: ChainStep): ExecuteChainVariants | undefined {
    return getSubcommand(step.subcommandId)?.variants
}

/** 条件步骤要渲染的参数，未选条件时为空。 */
function getStepArguments(step: ChainStep) {
    return activeStepArguments(getSubcommand(step.subcommandId), step.values)
}

function variantOptions(step: ChainStep): ComboboxOption<string>[] {
    const variants = variantsOf(step)
    if (!variants) return []
    return variants.options.map((option) => ({
        value: option.id,
        label: formatDynamic(option.labelKey, option.id),
    }))
}

function variantValue(step: ChainStep): string {
    const variants = variantsOf(step)
    if (!variants) return ''
    return String(step.values[variants.argument] ?? '')
}

function variantLabel(step: ChainStep): string {
    const option = resolveVariant(getSubcommand(step.subcommandId), step.values)
    return option ? formatDynamic(option.labelKey, option.id) : ''
}

function stepIcon(step: ChainStep): Component {
    const option = resolveVariant(getSubcommand(step.subcommandId), step.values)
    return option ? variantIcon(option.id, step.subcommandId) : subcommandIcon(step.subcommandId)
}

function onVariantChange(step: ChainStep, optionId: string) {
    const sub = getSubcommand(step.subcommandId)
    if (!sub?.variants) return
    step.values = switchVariant(sub, step.values, optionId)
}

/** 写法多的子命令用大卡片，说明与写法数量都摊开。 */
function cardSubcommands(category: ExecuteChainCategory): ExecuteChainSubcommand[] {
    return category.subcommands.filter((sub) => sub.variants?.layout === 'card')
}

/** 普通子命令与写法少的家族共用一排小按钮，家族多一个折角提示还要再选。 */
function compactSubcommands(category: ExecuteChainCategory): ExecuteChainSubcommand[] {
    return category.subcommands.filter((sub) => sub.variants?.layout !== 'card')
}

const output = computed(() => {
    const parts = chain.value.map(renderStep).filter(Boolean)
    const final = finalCommand.value.trim()
    if (final) parts.push('run ' + final)
    return parts.join(' ')
})

watch(
    output,
    (v) => {
        emit('update:modelValue', v)
    },
    { immediate: true },
)

function addSubcommand(sub: ExecuteChainSubcommand) {
    const values: Record<string, string | number | boolean> = {}
    // 条件子命令不带预设：后面跟什么由用户在卡片里选，选之前不生成任何命令片段。
    for (const arg of sub.arguments) {
        if (arg.defaultValue !== undefined) {
            values[arg.name] = arg.defaultValue
        }
    }
    chain.value.push({
        id: nextId++,
        subcommandId: sub.id,
        values,
        expanded: true,
    })
}

function removeStep(index: number) {
    chain.value.splice(index, 1)
}

function moveStep(index: number, delta: number) {
    const target = index + delta
    if (target < 0 || target >= chain.value.length) return
    const moved = chain.value[index]
    const before = stepElement(moved.id)?.getBoundingClientRect().top ?? null
    chain.value = moveChainItem(chain.value, index, target)
    nudgeState.value = { id: moved.id, direction: delta < 0 ? 'up' : 'down' }
    resetNudge()
    if (before === null) return
    void nextTick(() => slideStep(moved.id, before))
}

function stepElement(id: number): HTMLElement | null {
    return chainSectionRef.value?.querySelector<HTMLElement>(`[data-step-id="${id}"]`) ?? null
}

/** 箭头与拖拽走同一条排序路径，这里补上拖拽本来就有的位移动画。 */
function slideStep(id: number, previousTop: number) {
    if (reducedMotion.value === 'reduce') return
    const element = stepElement(id)
    if (!element) return
    const offset = previousTop - element.getBoundingClientRect().top
    if (!offset) return
    element.animate([{ transform: `translateY(${offset}px)` }, { transform: 'translateY(0)' }], {
        duration: 200,
        easing: 'cubic-bezier(0.2, 0, 0, 1)',
    })
}

function isNudged(id: number, direction: 'up' | 'down'): boolean {
    return nudgeState.value?.id === id && nudgeState.value.direction === direction
}

function onDragStart() {
    dragging.value = true
    const active = document.activeElement
    if (active instanceof HTMLElement) active.blur()
    // sortablejs 的克隆体是在 start 之前就 clone 好的，拿不到上面那个 inert，这里补一下
    chainSectionRef.value?.querySelector('.sortable-fallback')?.setAttribute('inert', '')
}

function onDragEnd() {
    dragging.value = false
}

function getSubcommand(id: string): ExecuteChainSubcommand | undefined {
    return subcommandMap.value.get(id)
}

function applyQuickPreset(preset: (typeof QUICK_PRESETS)[number]) {
    preset.build()
}

function clearChain() {
    chain.value = []
    finalCommand.value = ''
}

function applyQuickFinalCommand(prefix: string) {
    finalCommand.value = prefix + ' '
    queueMicrotask(() => {
        finalInputRef.value?.focus()
    })
}

function getState(): ExecuteChainState {
    return {
        chain: chain.value.map((s) => ({
            subcommandId: s.subcommandId,
            values: { ...s.values },
        })),
        finalCommand: finalCommand.value,
    }
}

function setState(state: ExecuteChainState) {
    chain.value = state.chain.map((step) => {
        // 旧预置里存的是 if-biome 这种 id，换成 if + condition=biome 才能打开
        const migrated = migrateLegacyStep(step, subcommandMap.value) ?? step
        return {
            id: nextId++,
            subcommandId: migrated.subcommandId,
            values: { ...migrated.values },
            expanded: false,
        }
    })
    finalCommand.value = state.finalCommand
}

defineExpose({ getState, setState })
</script>

<template>
    <div class="flex min-w-0 flex-col gap-5">
        <details class="rounded-lg border border-solid border-surface-4 p-3">
            <summary class="cursor-pointer text-sm font-medium text-[var(--color-text-primary)]">
                {{ formatMessage(messages.executeQuickExamples) }}
            </summary>
            <div class="mt-3 flex flex-wrap gap-2">
                <Button
                    v-for="preset in QUICK_PRESETS"
                    :key="preset.label.id"
                    type="outlined"
                    size="sm"
                    @click="applyQuickPreset(preset)"
                    >{{ formatMessage(preset.label) }}</Button
                >
            </div>
        </details>
        <section ref="chainSectionRef" class="flex min-w-0 flex-col gap-3">
            <div class="flex items-center justify-between gap-2">
                <h3 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.executeChainTitle) }}
                    <span class="ml-1 font-normal text-[var(--color-text-tertiary)]">{{
                        chain.length
                    }}</span>
                </h3>
                <Button
                    v-if="chain.length || finalCommand"
                    type="quiet"
                    size="sm"
                    @click="clearChain"
                    >{{ formatMessage(messages.executeClear) }}</Button
                >
            </div>
            <p
                v-if="!chain.length"
                class="m-0 rounded-lg bg-surface-3 p-3 text-sm leading-6 text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.executeChainEmpty) }}
            </p>
            <Draggable
                v-else
                :list="chain"
                item-key="id"
                tag="ol"
                class="m-0 flex min-w-0 list-none flex-col gap-3 p-0"
                :class="{ 'select-none': dragging }"
                handle=".execute-chain-drag-handle"
                ghost-class="opacity-30"
                :animation="150"
                :force-fallback="true"
                :fallback-on-body="false"
                @start="onDragStart"
                @end="onDragEnd"
            >
                <template #item="{ element: step, index }">
                    <li
                        :data-step-id="step.id"
                        class="min-w-0 rounded-lg border border-solid border-surface-4 bg-surface-3 p-3"
                    >
                        <!-- 拖动时内容整体 inert：卡片留在原位、克隆体跟着指针，两者都不该再吃焦点 -->
                        <div class="min-w-0" :inert="dragging || undefined">
                            <div class="flex flex-wrap items-center gap-2">
                                <span
                                    class="execute-chain-drag-handle flex shrink-0 cursor-grab touch-none items-center text-[var(--color-text-tertiary)] active:cursor-grabbing"
                                    aria-hidden="true"
                                    ><GripVerticalIcon class="h-4 w-4"
                                /></span>
                                <span class="text-sm tabular-nums text-[var(--color-text-tertiary)]"
                                    >{{ index + 1 }}.</span
                                >
                                <button
                                    class="flex min-w-0 flex-1 items-center gap-2 bg-transparent p-0 text-left text-sm font-semibold text-[var(--color-text-primary)]"
                                    :aria-expanded="step.expanded"
                                    @click="step.expanded = !step.expanded"
                                >
                                    <component
                                        :is="stepIcon(step)"
                                        aria-hidden="true"
                                        class="h-4 w-4 shrink-0 text-[var(--color-text-tertiary)]"
                                    />
                                    <span>
                                        {{
                                            getSubcommand(step.subcommandId)
                                                ? formatDynamic(
                                                      getSubcommand(step.subcommandId)!.labelKey,
                                                      step.subcommandId,
                                                  )
                                                : step.subcommandId
                                        }}
                                    </span>
                                    <template v-if="variantLabel(step)">
                                        <span
                                            class="text-[var(--color-text-tertiary)]"
                                            aria-hidden="true"
                                            >·</span
                                        >
                                        <span
                                            class="font-normal text-[var(--color-text-secondary)]"
                                            >{{ variantLabel(step) }}</span
                                        >
                                    </template>
                                </button>
                                <div
                                    class="flex items-center gap-1"
                                    data-tooltip-group="execute-actions"
                                >
                                    <Button
                                        v-tooltip="formatMessage(messages.moveUp)"
                                        type="quiet"
                                        size="sm"
                                        icon-only
                                        :label="formatMessage(messages.moveUp)"
                                        :disabled="index === 0"
                                        @click="moveStep(index, -1)"
                                        ><ChevronUpIcon
                                            class="transition-transform duration-200 ease-out"
                                            :class="
                                                isNudged(step.id, 'up') ? '-translate-y-1' : ''
                                            "
                                    /></Button>
                                    <Button
                                        v-tooltip="formatMessage(messages.moveDown)"
                                        type="quiet"
                                        size="sm"
                                        icon-only
                                        :label="formatMessage(messages.moveDown)"
                                        :disabled="index === chain.length - 1"
                                        @click="moveStep(index, 1)"
                                        ><ChevronDownIcon
                                            class="transition-transform duration-200 ease-out"
                                            :class="
                                                isNudged(step.id, 'down') ? 'translate-y-1' : ''
                                            "
                                    /></Button>
                                    <Button
                                        v-tooltip="formatMessage(messages.removeStep)"
                                        type="quiet"
                                        size="sm"
                                        icon-only
                                        :label="formatMessage(messages.removeStep)"
                                        @click="removeStep(index)"
                                        ><XIcon
                                    /></Button>
                                </div>
                            </div>
                            <code
                                v-if="renderStep(step)"
                                class="mt-2 block break-all text-xs leading-5 text-[var(--color-text-tertiary)]"
                                >{{ renderStep(step) }}</code
                            >
                            <p
                                v-else
                                class="m-0 mt-2 text-xs leading-5 text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.variantUnset) }}
                            </p>
                            <div v-if="step.expanded" class="mt-4 flex min-w-0 flex-col gap-4">
                                <div v-if="variantsOf(step)" class="flex min-w-0 flex-col gap-2">
                                    <span
                                        class="text-sm font-semibold text-[var(--color-text-primary)]"
                                        >{{ formatDynamic(variantsOf(step)!.labelKey, '') }}</span
                                    >
                                    <Combobox
                                        :model-value="variantValue(step)"
                                        :options="variantOptions(step)"
                                        :aria-label="formatDynamic(variantsOf(step)!.labelKey, '')"
                                        :placeholder="formatMessage(messages.variantPlaceholder)"
                                        width="full"
                                        @update:model-value="
                                            onVariantChange(step, String($event ?? ''))
                                        "
                                    />
                                </div>
                                <div class="grid min-w-0 gap-4 md:grid-cols-2">
                                    <CommandGeneratorArgument
                                        v-for="arg in getStepArguments(step)"
                                        :key="arg.name"
                                        v-model="step.values[arg.name]"
                                        :arg="arg"
                                    />
                                </div>
                            </div>
                        </div>
                    </li>
                </template>
            </Draggable>
        </section>
        <section class="flex min-w-0 flex-col gap-2">
            <label
                for="execute-final-command"
                class="text-sm font-semibold text-[var(--color-text-primary)]"
                >{{ formatMessage(messages.executeFinalTitle) }}</label
            >
            <input
                id="execute-final-command"
                ref="finalInputRef"
                v-model="finalCommand"
                class="w-full min-w-0 rounded-xl border-0 bg-surface-4 px-3 py-2 font-mono text-sm text-[var(--color-text-primary)] outline-none focus:ring-4 focus:ring-brand-shadow"
                :placeholder="formatMessage(messages.executeFinalPlaceholder)"
                autocomplete="off"
                spellcheck="false"
            />
            <p class="m-0 text-xs leading-5 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.executeFinalHint) }}
            </p>
            <div class="flex flex-wrap gap-1">
                <Button
                    v-for="qc in QUICK_FINAL_COMMANDS"
                    :key="qc"
                    type="quiet"
                    size="2xs"
                    @click="applyQuickFinalCommand(qc)"
                    ><code>/{{ qc }}</code></Button
                >
            </div>
        </section>
        <details class="rounded-lg border border-solid border-surface-4 p-3">
            <summary class="cursor-pointer text-sm font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.executePickerTitle) }}
            </summary>
            <div class="mt-3 flex flex-col gap-4">
                <div v-for="cat in categories" :key="cat.id" class="flex flex-col gap-2">
                    <p class="m-0 text-xs font-medium text-[var(--color-text-tertiary)]">
                        {{ formatDynamic(cat.labelKey, cat.id) }}
                    </p>
                    <!-- 写法多的子命令要再选一次跟什么，所以给它们更大的卡，而不是一排小按钮 -->
                    <div
                        v-if="cardSubcommands(cat).length"
                        class="grid min-w-0 gap-2 sm:grid-cols-2"
                    >
                        <button
                            v-for="sub in cardSubcommands(cat)"
                            :key="sub.id"
                            type="button"
                            class="flex min-w-0 items-start gap-3 rounded-xl border border-solid border-surface-4 bg-surface-3 p-3 text-left transition-colors hover:border-brand focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
                            @click="addSubcommand(sub)"
                        >
                            <span
                                class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-surface-4 text-[var(--color-text-secondary)]"
                            >
                                <component
                                    :is="subcommandIcon(sub.id)"
                                    aria-hidden="true"
                                    class="h-5 w-5"
                                />
                            </span>
                            <span class="flex min-w-0 flex-col gap-1">
                                <span
                                    class="font-mono text-sm font-semibold text-[var(--color-text-primary)]"
                                    >{{ formatDynamic(sub.labelKey, sub.id) }}</span
                                >
                                <span class="text-xs leading-5 text-[var(--color-text-tertiary)]">{{
                                    formatDynamic(sub.descriptionKey, sub.id)
                                }}</span>
                                <span class="text-xs text-[var(--color-text-tertiary)]">{{
                                    formatMessage(messages.variantCount, {
                                        count: sub.variants?.options.length ?? 0,
                                    })
                                }}</span>
                            </span>
                        </button>
                    </div>
                    <div v-if="compactSubcommands(cat).length" class="flex flex-wrap gap-2">
                        <Button
                            v-for="sub in compactSubcommands(cat)"
                            :key="sub.id"
                            v-tooltip="formatDynamic(sub.descriptionKey, sub.id)"
                            type="outlined"
                            size="sm"
                            @click="addSubcommand(sub)"
                            ><component
                                :is="subcommandIcon(sub.id)"
                                aria-hidden="true"
                                class="h-4 w-4" />{{ formatDynamic(sub.labelKey, sub.id)
                            }}<ChevronDownIcon
                                v-if="sub.variants"
                                aria-hidden="true"
                                class="h-3 w-3 opacity-60"
                        /></Button>
                    </div>
                </div>
            </div>
        </details>
    </div>
</template>
