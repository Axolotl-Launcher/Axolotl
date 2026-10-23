<script setup lang="ts" generic="T extends string | number">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

/**
 * Port of the app's `HeadlessSelect` (a reka-ui `Select`). The website bundles
 * reka-ui, but its portal would escape the scaled launcher window, so the list
 * is rendered absolutely inside the row instead, with the same trigger and item
 * classes and the same keyboard behaviour.
 */
const props = defineProps<{
    modelValue?: T
    options: { value: T; label: string }[]
    placeholder?: string
    id?: string
    name?: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: T]
}>()

const root = ref<HTMLElement | null>(null)
const open = ref(false)
const highlighted = ref(-1)

const selectedLabel = computed(() => {
    if (props.modelValue === undefined) return undefined
    return props.options.find((option) => option.value === props.modelValue)?.label
})

function toggle() {
    open.value = !open.value
    if (open.value) {
        highlighted.value = props.options.findIndex((option) => option.value === props.modelValue)
    }
}

function choose(option: { value: T }) {
    emit('update:modelValue', option.value)
    open.value = false
}

function onPointerDown(event: PointerEvent) {
    if (!open.value) return
    if (root.value?.contains(event.target as Node)) return
    open.value = false
}

function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && open.value) {
        event.stopPropagation()
        open.value = false
        return
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault()
        if (!open.value) {
            open.value = true
            highlighted.value = -1
        }
        const count = props.options.length
        const delta = event.key === 'ArrowDown' ? 1 : -1
        highlighted.value = (highlighted.value + delta + count) % count
        return
    }
    if ((event.key === 'Enter' || event.key === ' ') && open.value) {
        event.preventDefault()
        const option = props.options[highlighted.value]
        if (option) choose(option)
        return
    }
    if ((event.key === 'Enter' || event.key === ' ') && !open.value) {
        event.preventDefault()
        toggle()
    }
}

watch(open, (isOpen) => {
    if (isOpen) document.addEventListener('pointerdown', onPointerDown, true)
    else document.removeEventListener('pointerdown', onPointerDown, true)
})

onBeforeUnmount(() => document.removeEventListener('pointerdown', onPointerDown, true))
</script>

<template>
    <div ref="root" class="relative w-full">
        <button
            :id="id"
            type="button"
            class="flex h-9 w-full min-w-36 items-center justify-between gap-2 rounded-[var(--radius-md)] border border-surface-5 bg-surface-4 px-3 text-sm text-[var(--color-text-primary)] transition-colors hover:brightness-[var(--hover-brightness)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand"
            :aria-label="name || placeholder"
            :aria-expanded="open"
            aria-haspopup="listbox"
            @click="toggle"
            @keydown="onKeydown"
        >
            <span v-if="selectedLabel" class="min-w-0 truncate">{{ selectedLabel }}</span>
            <span v-else class="min-w-0 truncate text-[var(--color-text-tertiary)]">
                {{ placeholder }}
            </span>
            <ChevronDownIcon
                class="size-4 shrink-0 text-[var(--color-text-tertiary)] transition-transform"
                :class="{ 'rotate-180': open }"
                aria-hidden="true"
            />
        </button>

        <Transition
            enter-active-class="transition-opacity duration-150"
            leave-active-class="transition-opacity duration-150"
            enter-from-class="opacity-0"
            leave-to-class="opacity-0"
        >
            <div
                v-if="open"
                role="listbox"
                class="absolute left-0 right-0 top-full z-50 mt-1 overflow-hidden rounded-[var(--radius-md)] border border-surface-5 bg-surface-3 p-1 text-[var(--color-text-primary)] shadow-md"
            >
                <button
                    v-for="(option, index) in options"
                    :key="String(option.value)"
                    type="button"
                    role="option"
                    :aria-selected="option.value === modelValue"
                    class="relative flex w-full cursor-pointer select-none items-center rounded-[var(--radius-sm)] px-2 py-1.5 text-left text-sm text-[var(--color-text-primary)] outline-none"
                    :class="[
                        index === highlighted ? 'bg-surface-4' : '',
                        option.value === modelValue ? 'font-semibold' : '',
                    ]"
                    @mouseenter="highlighted = index"
                    @click="choose(option)"
                >
                    {{ option.label }}
                    <CheckIcon
                        v-if="option.value === modelValue"
                        class="absolute right-2 size-3.5 text-brand"
                        aria-hidden="true"
                    />
                </button>
            </div>
        </Transition>
    </div>
</template>
