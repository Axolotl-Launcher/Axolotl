<script setup lang="ts">
/**
 * NBT 路径输入（`/data` 的 `path` 参数）。
 *
 * 输入框是权威来源：路径可以带下标、区间、复合标签过滤器（`Items[{id:"stone"}]`），
 * 这些没法靠点选拼出来。右边的按钮打开逐层选择器，把能查到的键点进来。
 */

import { TagIcon } from '@modrinth/assets'
import { Button, defineMessages, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, ref, useTemplateRef, watch } from 'vue'

import { availableFieldsFor } from '@/lab/command-generator/nbt/catalog.ts'
import { parseNbtPath, validateNbtPath } from '@/lab/command-generator/nbt/path.ts'
import type { NbtDomain } from '@/lab/command-generator/nbt/schema.ts'

import CommandGeneratorNbtPathPicker from './CommandGeneratorNbtPathPicker.vue'

const props = defineProps<{
    modelValue?: string
    version?: string
    /** 目录域；`storage` 没有字段目录，此时只做语法校验。 */
    domain?: NbtDomain
    targetId?: string
    ariaLabel?: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    placeholder: {
        id: 'app.lab.command-generator.nbt.path.placeholder',
        defaultMessage: 'Inventory[0].id',
    },
    hint: {
        id: 'app.lab.command-generator.nbt.path.hint',
        defaultMessage: 'A path points at a tag: Foo.Bar, Items[0], Items[{id:"stone"}]',
    },
    pick: {
        id: 'app.lab.command-generator.nbt.path.pick',
        defaultMessage: 'Pick tag',
    },
    unknownRoot: {
        id: 'app.lab.command-generator.nbt.path.unknown-root',
        defaultMessage: 'This tag is not in the catalog; it will be written as typed.',
    },
})

const inputRef = useTemplateRef<InstanceType<typeof StyledInput>>('input')
const picker = useTemplateRef<InstanceType<typeof CommandGeneratorNbtPathPicker>>('picker')

const text = ref(props.modelValue ?? '')

watch(
    () => props.modelValue,
    (value) => {
        const next = value ?? ''
        if (next !== text.value) text.value = next
    },
)

const issues = computed(() => validateNbtPath(text.value))
const errors = computed(() => issues.value.filter((issue) => issue.level === 'error'))

const unknownRoot = computed(() => {
    if (!props.domain) return false
    const keys = parseNbtPath(text.value).filter((segment) => segment.kind === 'key')
    if (keys.length === 0) return false
    const root = (keys[0] as { name: string }).name
    return !availableFieldsFor({
        domain: props.domain,
        version: props.version ?? '1.21',
        targetId: props.targetId,
    }).some((field) => field.key === root)
})

/** 把路径拆成片段展示，让用户看清自己写的是下标还是键名。 */
const segments = computed(() =>
    parseNbtPath(text.value).map((segment) => {
        switch (segment.kind) {
            case 'key':
                return { text: segment.name, kind: 'key' as const }
            case 'index':
                return { text: `[${segment.index}]`, kind: 'index' as const }
            case 'slice':
                return {
                    text: `[${segment.from ?? ''}..${segment.to ?? ''}]`,
                    kind: 'index' as const,
                }
            case 'all':
                return { text: '[]', kind: 'index' as const }
            case 'filter':
                return { text: segment.raw, kind: 'filter' as const }
        }
    }),
)

function publish(value: string) {
    text.value = value
    emit('update:modelValue', value)
}

function onInput(value: string | number | undefined) {
    publish(String(value ?? ''))
}

function openPicker() {
    picker.value?.show()
}

defineExpose({ focus: () => inputRef.value?.focus() })
</script>

<template>
    <div class="flex min-w-0 flex-col gap-2">
        <div class="flex min-w-0 gap-2">
            <StyledInput
                ref="input"
                :model-value="text"
                :aria-label="ariaLabel"
                :placeholder="formatMessage(messages.placeholder)"
                class="min-w-0 flex-1 font-mono"
                :spellcheck="false"
                clearable
                @update:model-value="onInput"
            />
            <Button type="outlined" @click="openPicker">
                <TagIcon class="h-4 w-4" />
                {{ formatMessage(messages.pick) }}
            </Button>
        </div>

        <div v-if="segments.length" class="flex flex-wrap items-center gap-1">
            <span
                v-for="(segment, index) in segments"
                :key="`${segment.kind}-${index}`"
                class="rounded px-1.5 py-0.5 font-mono text-[0.625rem]"
                :class="
                    segment.kind === 'key'
                        ? 'bg-surface-4 text-[var(--color-text-primary)]'
                        : segment.kind === 'index'
                          ? 'bg-surface-5 text-[var(--color-text-secondary)]'
                          : 'bg-surface-5 text-[var(--color-text-tertiary)]'
                "
                >{{ segment.text }}</span
            >
        </div>

        <p v-for="issue in errors" :key="issue.message" class="m-0 text-xs text-red">
            {{ issue.message }}
        </p>
        <p v-if="unknownRoot" class="m-0 text-xs text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.unknownRoot) }}
        </p>
        <p v-else-if="!errors.length" class="m-0 text-xs text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.hint) }}
        </p>

        <CommandGeneratorNbtPathPicker
            ref="picker"
            :model-value="text"
            :version="version"
            :domain="domain"
            :target-id="targetId"
            @update:model-value="publish"
        />
    </div>
</template>
