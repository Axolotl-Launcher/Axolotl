<script setup lang="ts">
import { ChevronDownIcon, ChevronUpIcon, PlusIcon, XIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import type {
    ExecuteChainCategory,
    ExecuteChainSubcommand,
} from '@/lab/command-generator/commands/types.ts'

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
    return formatMessage({ id, defaultMessage: fallback || id })
}

let nextId = 1

const chain = ref<ChainStep[]>([])
const finalCommand = ref('')
const finalInputRef = ref<HTMLInputElement | null>(null)

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
                    subcommandId: 'if-entity',
                    values: { target: '@e[type=zombie]' },
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
                    subcommandId: 'store-result-score',
                    values: { target: '@s', objective: 'kills' },
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
                    subcommandId: 'if-block',
                    values: { pos: '~ ~-1 ~', block: 'minecraft:grass_block' },
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
    let text = sub.template
    for (const arg of sub.arguments) {
        const value = step.values[arg.name]
        const replacement =
            value !== undefined && value !== null && value !== '' ? String(value) : ''
        text = text.split(`{${arg.name}}`).join(replacement)
    }
    return text.trim()
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
    const [item] = chain.value.splice(index, 1)
    chain.value.splice(target, 0, item)
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
    chain.value = state.chain.map((s) => ({
        id: nextId++,
        subcommandId: s.subcommandId,
        values: { ...s.values },
        expanded: false,
    }))
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
        <section class="flex min-w-0 flex-col gap-3">
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
            <ol v-else class="m-0 flex min-w-0 list-none flex-col gap-3 p-0">
                <li
                    v-for="(step, index) in chain"
                    :key="step.id"
                    class="min-w-0 rounded-lg border border-solid border-surface-4 bg-surface-3 p-3"
                >
                    <div class="flex flex-wrap items-center gap-2">
                        <span class="text-sm tabular-nums text-[var(--color-text-tertiary)]"
                            >{{ index + 1 }}.</span
                        >
                        <button
                            class="min-w-0 flex-1 bg-transparent p-0 text-left text-sm font-semibold text-[var(--color-text-primary)]"
                            :aria-expanded="step.expanded"
                            @click="step.expanded = !step.expanded"
                        >
                            {{
                                getSubcommand(step.subcommandId)
                                    ? formatDynamic(
                                          getSubcommand(step.subcommandId)!.labelKey,
                                          step.subcommandId,
                                      )
                                    : step.subcommandId
                            }}
                        </button>
                        <div class="flex items-center gap-1" data-tooltip-group="execute-actions">
                            <Button
                                v-tooltip="formatMessage(messages.moveUp)"
                                type="quiet"
                                size="sm"
                                icon-only
                                :label="formatMessage(messages.moveUp)"
                                :disabled="index === 0"
                                @click="moveStep(index, -1)"
                                ><ChevronUpIcon
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
                        class="mt-2 block break-all text-xs leading-5 text-[var(--color-text-tertiary)]"
                        >{{ renderStep(step) }}</code
                    >
                    <div v-if="step.expanded" class="mt-4 grid min-w-0 gap-4 md:grid-cols-2">
                        <CommandGeneratorArgument
                            v-for="arg in getSubcommand(step.subcommandId)?.arguments ?? []"
                            :key="arg.name"
                            v-model="step.values[arg.name]"
                            :arg="arg"
                        />
                    </div>
                </li>
            </ol>
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
                <div v-for="cat in categories" :key="cat.id">
                    <p class="m-0 mb-2 text-xs font-medium text-[var(--color-text-tertiary)]">
                        {{ formatDynamic(cat.labelKey, cat.id) }}
                    </p>
                    <div class="flex flex-wrap gap-2">
                        <Button
                            v-for="sub in cat.subcommands"
                            :key="sub.id"
                            v-tooltip="formatDynamic(sub.descriptionKey, sub.id)"
                            type="outlined"
                            size="sm"
                            @click="addSubcommand(sub)"
                            ><PlusIcon class="h-4 w-4" />{{
                                formatDynamic(sub.labelKey, sub.id)
                            }}</Button
                        >
                    </div>
                </div>
            </div>
        </details>
    </div>
</template>
