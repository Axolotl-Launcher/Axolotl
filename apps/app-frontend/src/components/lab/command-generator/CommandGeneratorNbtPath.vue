<script setup lang="ts">
import { defineMessages, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, ref, useTemplateRef, watch } from 'vue'

import { availableFieldsFor, fieldsAtPath } from '@/lab/command-generator/nbt/catalog.ts'
import { fieldLabel } from '@/lab/command-generator/nbt/labels.ts'
import {
    appendPathKey,
    parseNbtPath,
    pathCompletionPrefix,
    validateNbtPath,
} from '@/lab/command-generator/nbt/path.ts'
import {
    type NbtDomain,
    type NbtFieldSpec,
    searchFields,
} from '@/lab/command-generator/nbt/schema.ts'

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

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
    placeholder: {
        id: 'app.lab.command-generator.nbt.path.placeholder',
        defaultMessage: 'Inventory[0].id',
    },
    hint: {
        id: 'app.lab.command-generator.nbt.path.hint',
        defaultMessage: 'A path points at a tag: Foo.Bar, Items[0], Items[{id:"stone"}]',
    },
    suggestions: {
        id: 'app.lab.command-generator.nbt.path.suggestions',
        defaultMessage: 'Tag suggestions',
    },
    unknownRoot: {
        id: 'app.lab.command-generator.nbt.path.unknown-root',
        defaultMessage: 'This tag is not in the catalog; it will be written as typed.',
    },
})

const inputRef = useTemplateRef<InstanceType<typeof StyledInput>>('input')
const text = ref(props.modelValue ?? '')
const showSuggestions = ref(false)
const activeSuggestion = ref(0)

watch(
    () => props.modelValue,
    (value) => {
        const next = value ?? ''
        if (next !== text.value) text.value = next
    },
)

const issues = computed(() => validateNbtPath(text.value))
const errors = computed(() => issues.value.filter((issue) => issue.level === 'error'))

const completion = computed(() => pathCompletionPrefix(text.value))

/** 走到第几层就列第几层的字段；断链时退回根字段。 */
const candidateFields = computed<NbtFieldSpec[]>(() => {
    if (!props.domain) return []
    const { depth } = completion.value
    const keys = parseNbtPath(text.value)
        .filter((segment) => segment.kind === 'key')
        .map((segment) => (segment as { name: string }).name)

    if (depth <= 1) {
        return availableFieldsFor({
            domain: props.domain,
            version: props.version ?? '1.21',
            targetId: props.targetId,
        })
    }
    // 去掉最后一段（正在输入的那个），用剩下的去定位父级
    const parentKeys = keys.slice(0, Math.max(0, depth - 1))
    return (
        fieldsAtPath(props.domain, parentKeys) ??
        availableFieldsFor({
            domain: props.domain,
            version: props.version ?? '1.21',
            targetId: props.targetId,
        })
    )
})

const suggestions = computed(() => {
    if (!showSuggestions.value) return []
    const { prefix } = completion.value
    const fields = candidateFields.value
    const needle = prefix.toLowerCase()
    const matched = needle
        ? fields.filter(
              (field) =>
                  field.key.toLowerCase().includes(needle) ||
                  fieldLabel(field, locale.value).toLowerCase().includes(needle),
          )
        : fields
    return searchFields(matched, '', props.version ?? '1.21').map((result) => result.spec)
})

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

function publish(value: string) {
    text.value = value
    emit('update:modelValue', value)
}

function onInput(value: string | number | undefined) {
    showSuggestions.value = true
    activeSuggestion.value = 0
    publish(String(value ?? ''))
}

function applySuggestion(spec: NbtFieldSpec) {
    const next = appendPathKey(text.value, spec.key)
    showSuggestions.value = false
    publish(next)
    void inputRef.value?.focus()
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
    if (event.key === 'Tab' || event.key === 'Enter') {
        event.preventDefault()
        applySuggestion(suggestions.value[activeSuggestion.value])
        return
    }
    if (event.key === 'Escape') showSuggestions.value = false
}

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

defineExpose({ focus: () => inputRef.value?.focus() })
</script>

<template>
    <div class="relative flex min-w-0 flex-col gap-2">
        <StyledInput
            ref="input"
            :model-value="text"
            :aria-label="ariaLabel"
            :placeholder="formatMessage(messages.placeholder)"
            class="w-full font-mono"
            :spellcheck="false"
            clearable
            @update:model-value="onInput"
            @keydown="onKeydown"
            @focus="showSuggestions = true"
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
                    <span class="font-mono text-sm text-[var(--color-text-primary)]">{{
                        spec.key
                    }}</span>
                    <span class="truncate text-xs text-[var(--color-text-tertiary)]">{{
                        fieldLabel(spec, locale)
                    }}</span>
                </button>
            </li>
        </ul>

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
    </div>
</template>
