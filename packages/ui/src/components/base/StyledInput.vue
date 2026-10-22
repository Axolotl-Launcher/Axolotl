<template>
    <div
        v-bind="wrapperAttrs"
        class="relative"
        :style="$attrs.style"
        :class="[
            $attrs.class,
            wrapperClass,
            multiline ? 'flex' : 'inline-flex',
            { 'opacity-50 cursor-not-allowed': disabled },
            !multiline && variant === 'outlined' ? 'items-stretch' : 'items-center',
        ]"
    >
        <!-- Left icon (filled variant, single-line only) -->
        <component
            :is="icon"
            v-if="icon && variant === 'filled' && !multiline"
            class="absolute left-3 h-5 w-5 z-[1] pointer-events-none transition-colors"
            :class="[
                isFocused
                    ? 'opacity-100 text-[var(--color-text-primary)]'
                    : 'opacity-60 text-[var(--color-text-tertiary)]',
            ]"
            aria-hidden="true"
        />

        <!-- Multiline textarea -->
        <textarea
            v-if="multiline"
            :id="id"
            v-bind="nativeAttrs()"
            ref="inputRef"
            :value="model"
            :placeholder="placeholder"
            :disabled="disabled"
            :readonly="readonly"
            :name="name"
            :autocomplete="autocomplete"
            :autocorrect="autocorrect"
            :autocapitalize="autocapitalize"
            :spellcheck="spellcheck"
            :maxlength="maxlength"
            :rows="rows"
            class="w-full touch-manipulation text-[var(--color-text-default)] placeholder:text-[var(--color-text-tertiary)] focus:text-[var(--color-text-primary)] font-medium transition-[shadow,color] appearance-none shadow-none focus:ring-4 focus:ring-brand-shadow bg-surface-4 border-none rounded-xl"
            :class="[
                inputClass,
                'pl-3 pr-3 py-2 text-base',
                error ? 'outline outline-2 outline-red bg-warning-bg' : 'outline-none',
                disabled ? 'cursor-not-allowed' : '',
                resizeClass,
            ]"
            @focus="isFocused = true"
            @blur="isFocused = false"
        />

        <!-- Single-line input -->
        <input
            v-else
            :id="id"
            v-bind="nativeAttrs()"
            ref="inputRef"
            :type="type"
            :value="model"
            :placeholder="placeholder"
            :disabled="disabled"
            :readonly="readonly"
            :name="name"
            :autocomplete="autocomplete"
            :autocorrect="autocorrect"
            :autocapitalize="autocapitalize"
            :spellcheck="spellcheck"
            :inputmode="inputmode"
            :maxlength="maxlength"
            :min="min"
            :max="max"
            :step="step"
            class="min-w-0 w-full touch-manipulation text-[var(--color-text-default)] placeholder:text-[var(--color-text-tertiary)] focus:text-[var(--color-text-primary)] font-medium transition-[shadow,color] appearance-none shadow-none focus:ring-4 focus:ring-brand-shadow"
            :class="[
                inputClass,
                !multiline && hasRightSlot ? 'flex-1' : '',
                variant === 'filled' && icon ? 'pl-10' : 'pl-3',
                clearable && model && variant === 'filled' ? 'pr-8' : 'pr-3',
                size === 'small' ? 'h-8 py-1.5 text-sm' : 'h-9 py-2 text-base',
                error ? 'outline outline-2 outline-red bg-warning-bg' : 'outline-none',
                disabled ? 'cursor-not-allowed' : '',
                variant === 'outlined'
                    ? 'bg-transparent border border-solid border-surface-4 rounded-l-xl border-r-0'
                    : 'bg-surface-4 border-none rounded-xl',
            ]"
            @focus="isFocused = true"
            @blur="isFocused = false"
        />

        <!-- Clear button (right side, filled variant, single-line only) -->
        <button
            v-if="
                !multiline && clearable && model && !disabled && !readonly && variant === 'filled'
            "
            type="button"
            class="absolute right-0.5 z-[1] p-2 touch-manipulation bg-transparent border-none text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] transition-colors cursor-pointer select-none"
            :aria-label="clearLabel || formatMessage(commonMessages.clearButton)"
            @click="clear"
        >
            <XIcon class="h-5 w-5" />
        </button>

        <!-- Right icon button (outlined variant, single-line only) -->
        <button
            v-if="!multiline && variant === 'outlined'"
            type="button"
            class="flex touch-manipulation items-center justify-center px-2 bg-transparent border border-solid border-surface-4 rounded-r-xl text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] transition-colors shrink-0"
            :aria-label="
                canClear
                    ? clearLabel || formatMessage(commonMessages.clearButton)
                    : formatMessage(commonMessages.searchLabel)
            "
            :disabled="disabled || readonly"
            :tabindex="clearable && model ? undefined : -1"
            @click="canClear ? clear() : undefined"
        >
            <XIcon v-if="clearable && model" class="h-4 w-4" />
            <component :is="icon" v-else-if="icon" class="h-4 w-4" />
            <SearchIcon v-else class="h-4 w-4" />
        </button>

        <!-- Custom rightside slot -->
        <slot name="right" />
    </div>
</template>

<script setup lang="ts">
import { SearchIcon, XIcon } from '@modrinth/assets'
import {
    type Component,
    computed,
    type HTMLAttributes,
    type InputHTMLAttributes,
    ref,
    type TextareaHTMLAttributes,
    useAttrs,
    useSlots,
} from 'vue'

import { useVIntl } from '../../composables/i18n'
import { commonMessages } from '../../utils/common-messages'

defineOptions({ inheritAttrs: false })

const attrs = useAttrs()
const { formatMessage } = useVIntl()

type NativeInputAttrs = Omit<
    InputHTMLAttributes & TextareaHTMLAttributes,
    'size' | 'value' | 'type' | 'min' | 'max' | 'step'
>

/** Layout class/style stay on the wrapper; other attributes and native listeners describe the field. */
function nativeAttrs() {
    const { class: _class, style: _style, onInput: listener, ...native } = attrs
    const { onInput: inputListener, ...inputAttrs } = props.inputAttrs ?? {}
    return {
        ...native,
        ...inputAttrs,
        onInput: [onInput, listener, inputListener].flat().filter((fn) => typeof fn === 'function'),
    }
}

const model = defineModel<string | number | undefined>()
const hasRightSlot = Boolean(useSlots().right)

const props = withDefaults(
    defineProps<{
        icon?: Component
        type?:
            'text' | 'email' | 'password' | 'number' | 'url' | 'search' | 'date' | 'datetime-local'
        placeholder?: string
        id?: string
        name?: string
        autocomplete?: string
        autocorrect?: 'on' | 'off'
        autocapitalize?: 'none' | 'off' | 'sentences' | 'words' | 'characters'
        spellcheck?: boolean
        inputmode?: 'none' | 'text' | 'decimal' | 'numeric' | 'tel' | 'search' | 'email' | 'url'
        maxlength?: number
        min?: number
        max?: number
        step?: number
        disabled?: boolean
        readonly?: boolean
        error?: boolean
        size?: 'standard' | 'small'
        variant?: 'filled' | 'outlined'
        clearable?: boolean
        clearLabel?: string
        multiline?: boolean
        rows?: number
        resize?: 'none' | 'vertical' | 'both'
        inputClass?: string
        wrapperClass?: string
        inputAttrs?: NativeInputAttrs
        /** Explicit attributes and listeners for the layout wrapper. */
        wrapperAttrs?: HTMLAttributes
    }>(),
    {
        type: 'text',
        size: 'standard',
        variant: 'filled',
        disabled: false,
        readonly: false,
        error: false,
        clearable: false,
        multiline: false,
        rows: 3,
        resize: 'none',
    },
)

const emit = defineEmits<{
    clear: []
}>()

const inputRef = ref<HTMLInputElement | HTMLTextAreaElement>()
const isFocused = ref(false)
const canClear = computed(
    () => props.clearable && !!model.value && !props.disabled && !props.readonly,
)
const resizeClass = computed(
    () => ({ none: 'resize-none', vertical: 'resize-y', both: 'resize' })[props.resize ?? 'none'],
)

defineExpose({
    focus: () => inputRef.value?.focus(),
    select: () => inputRef.value?.select(),
})

function onInput(event: Event) {
    const target = event.target as HTMLInputElement | HTMLTextAreaElement
    model.value =
        props.type === 'number' && !props.multiline
            ? target.value === ''
                ? undefined
                : Number(target.value)
            : target.value
}

function clear() {
    if (!canClear.value) return
    model.value = props.type === 'number' && !props.multiline ? undefined : ''
    emit('clear')
}
</script>
