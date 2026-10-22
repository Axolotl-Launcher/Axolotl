<script setup lang="ts">
import { SparklesIcon } from '@modrinth/assets'
import { Button, Combobox, type ComboboxOption, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import CommandGeneratorArgument from '@/components/lab/command-generator/CommandGeneratorArgument.vue'
import { findGuide } from '@/lab/command-generator/command-guides.ts'
import {
    CATEGORY_KEYS,
    findCommand,
    getAllCommands,
    initializeRegistry,
} from '@/lab/command-generator/commands/registry.ts'
import {
    buildCommand,
    getActiveArguments,
    getDefaultValues,
    validateCommand,
} from '@/lab/command-generator/generator.ts'

const { formatMessage } = useVIntl()

function t(key: string, fallback?: string): string {
    return formatMessage({ id: key, defaultMessage: fallback ?? key })
}

const messages = defineMessages({
    title: { id: 'app.lab.command-generator.title', defaultMessage: 'Command generator' },
    selectCommand: {
        id: 'app.lab.command-generator.select-command',
        defaultMessage: 'Choose a command',
    },
    generate: { id: 'app.lab.command-generator.generate', defaultMessage: 'Generate' },
    copy: { id: 'app.lab.command-generator.copy', defaultMessage: 'Copy' },
    result: { id: 'app.lab.command-generator.result', defaultMessage: 'Result' },
})

const selectedCommandId = ref('')
const values = ref<Record<string, string | number | boolean>>({})
const result = ref('')
const errors = ref<string[]>([])

onMounted(() => {
    // 该组件不再自行加载版本清单，但需要保证 registry 至少初始化一次
    initializeRegistry()
})

const commandOptions = computed<ComboboxOption<string>[]>(() =>
    getAllCommands().map((cmd) => ({
        value: cmd.id,
        label: `[${t(CATEGORY_KEYS[cmd.category])}] /${cmd.name} — ${t(cmd.descriptionKey)}`,
    })),
)

const currentCommand = computed(() =>
    selectedCommandId.value ? (findCommand(selectedCommandId.value) ?? null) : null,
)
const currentGuide = computed(() =>
    selectedCommandId.value ? (findGuide(selectedCommandId.value) ?? null) : null,
)

function onCommandChange(id: string) {
    selectedCommandId.value = id
    if (!id) {
        values.value = {}
        result.value = ''
        errors.value = []
        return
    }
    values.value = getDefaultValues(id)
    result.value = ''
    errors.value = []
}

function generate() {
    if (!selectedCommandId.value) return
    const validationErrors = validateCommand(selectedCommandId.value, values.value)
    if (validationErrors.length) {
        errors.value = validationErrors
        result.value = ''
        return
    }
    errors.value = []
    try {
        result.value = buildCommand(selectedCommandId.value, values.value)
    } catch (e) {
        errors.value = [e instanceof Error ? e.message : 'Unknown error']
        result.value = ''
    }
}

async function copy() {
    if (!result.value) return
    try {
        await navigator.clipboard.writeText(result.value)
    } catch {
        // 剪贴板不可用时静默失败
    }
}
</script>

<template>
    <div class="flex flex-col gap-4">
        <h1 class="m-0 text-[1.1rem] font-bold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.title) }}
        </h1>

        <label class="flex flex-col gap-2 font-semibold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.selectCommand) }}
            <Combobox
                :model-value="selectedCommandId"
                :options="commandOptions"
                searchable
                @update:model-value="onCommandChange(String($event))"
            >
                <template #option="{ item, isSelected }">
                    <span
                        class="truncate text-[0.82rem]"
                        :class="isSelected ? 'text-brand' : 'text-[var(--color-text-default)]'"
                    >
                        {{ item.label }}
                    </span>
                </template>
            </Combobox>
        </label>

        <section
            v-if="currentGuide"
            class="flex flex-col gap-2 rounded-[var(--radius-lg)] bg-[var(--surface-3)] p-[0.95rem_1rem]"
        >
            <p class="m-0 text-[0.78rem] text-[var(--color-text-tertiary)]">
                {{ t(currentGuide.summaryKey) }}
            </p>
            <div v-for="ex in currentGuide.examples" :key="ex.command" class="flex flex-col gap-1">
                <code class="font-mono text-[0.75rem] text-[var(--color-brand)]">
                    {{ ex.command }}
                </code>
                <span class="text-[0.7rem] text-[var(--color-text-tertiary)]">
                    {{ t(ex.explanationKey) }}
                </span>
            </div>
            <ul
                v-if="currentGuide.tipKeys?.length"
                class="m-0 list-inside list-disc text-[0.7rem] text-[var(--color-text-tertiary)]"
            >
                <li v-for="tipKey in currentGuide.tipKeys" :key="tipKey">
                    {{ t(tipKey) }}
                </li>
            </ul>
        </section>

        <template v-if="currentCommand">
            <CommandGeneratorArgument
                v-for="arg in getActiveArguments(currentCommand, values).filter(
                    (arg) => arg.literal === undefined,
                )"
                :key="arg.name"
                v-model="values[arg.name]"
                :arg="arg"
            />

            <div
                v-if="errors.length"
                class="flex flex-col gap-1 rounded-[var(--radius-md)] bg-[color-mix(in_srgb,var(--color-red)_10%,transparent)] p-3"
            >
                <p
                    v-for="(err, i) in errors"
                    :key="i"
                    class="m-0 text-[0.75rem] text-[var(--color-red)]"
                >
                    {{ err }}
                </p>
            </div>

            <div class="flex gap-2">
                <Button type="colored" color="brand" @click="generate">
                    <SparklesIcon />
                    {{ formatMessage(messages.generate) }}
                </Button>
                <Button v-if="result" type="outlined" @click="copy">
                    {{ formatMessage(messages.copy) }}
                </Button>
            </div>
        </template>

        <section v-if="result" class="flex flex-col gap-2">
            <h3 class="m-0 text-[0.8rem] font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.result) }}
            </h3>
            <pre class="result m-0">{{ result }}</pre>
        </section>
    </div>
</template>

<style scoped>
.result {
    max-height: 18rem;
    overflow: auto;
    border: 1px solid var(--color-divider);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    padding: 0.7rem 0.8rem;
    font-family: var(--mono-font, monospace);
    font-size: 0.78rem;
    white-space: pre-wrap;
    word-break: break-all;
}
</style>
