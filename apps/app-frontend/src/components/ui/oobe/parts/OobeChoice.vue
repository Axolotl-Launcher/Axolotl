<script setup lang="ts">
/**
 * One selectable row of an OOBE list. `role` is `radio` inside a radiogroup and
 * `button` for a row that starts an action.
 */
withDefaults(
    defineProps<{
        selected?: boolean
        disabled?: boolean
        role?: 'radio' | 'button'
    }>(),
    { selected: false, disabled: false, role: 'button' },
)

defineEmits<{ select: [] }>()
</script>

<template>
    <button
        type="button"
        class="oobe-choice group flex w-full min-w-0 cursor-pointer items-center gap-3 rounded-[var(--radius-lg)] border border-solid px-4 py-3 text-left transition-[background-color,border-color,box-shadow] duration-150 disabled:cursor-not-allowed disabled:opacity-60"
        :class="
            selected
                ? 'border-brand bg-brand-highlight'
                : 'border-surface-4 bg-surface-3 hover:border-surface-5 hover:bg-surface-4'
        "
        :role="role"
        :aria-checked="role === 'radio' ? selected : undefined"
        :disabled="disabled"
        @click="$emit('select')"
    >
        <slot name="leading" />
        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
            <slot />
        </span>
        <slot name="trailing" />
    </button>
</template>

<style scoped lang="scss">
.oobe-choice:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px var(--color-focus-ring);
}
</style>
