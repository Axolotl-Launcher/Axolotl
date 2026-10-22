<script setup lang="ts">
withDefaults(
    defineProps<{
        compact?: boolean
        stacked?: boolean
        inlineControl?: boolean
    }>(),
    {
        compact: false,
        stacked: false,
        inlineControl: false,
    },
)
</script>

<template>
    <div
        class="settings-row"
        :class="{
            'settings-row-compact': compact,
            'settings-row-stacked': stacked,
            'settings-row-inline-control': inlineControl,
        }"
    >
        <div class="flex min-w-0 flex-col gap-1">
            <div
                v-if="$slots.label"
                class="text-[var(--color-text-primary)] text-base font-semibold"
            >
                <slot name="label" />
            </div>
            <div
                v-if="$slots.description"
                class="text-[var(--color-text-tertiary)] text-sm leading-[1.45]"
            >
                <slot name="description" />
            </div>
            <slot name="copy" />
        </div>
        <div v-if="$slots.control" class="settings-row-control flex min-w-0 justify-end">
            <slot name="control" />
        </div>
    </div>
</template>

<style scoped>
.settings-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(10rem, 12rem);
    align-items: center;
    gap: var(--gap-xl);
    min-height: 4rem;
    padding: var(--gap-md) var(--gap-lg);
    border-bottom: 1px solid
        var(--settings-divider, color-mix(in srgb, var(--surface-4) 55%, transparent));
}

.settings-row:last-child {
    border-bottom: 0;
}

.settings-row-compact {
    min-height: 3.5rem;
}

.settings-row-stacked {
    grid-template-columns: minmax(0, 1fr);
    align-items: start;
    gap: var(--gap-md);
}

.settings-row-stacked .settings-row-control {
    justify-content: flex-start;
    width: 100%;
}

.settings-row-control :deep(.btn),
.settings-row-control :deep([data-button]),
.settings-row-control :deep(input),
.settings-row-control :deep(select),
.settings-row-control :deep(.combobox) {
    max-width: 100%;
}

@media (max-width: 700px) {
    .settings-row {
        grid-template-columns: minmax(0, 1fr);
        align-items: start;
        gap: var(--gap-md);
    }

    .settings-row-control {
        justify-content: flex-start;
        width: 100%;
    }

    .settings-row-inline-control {
        grid-template-columns: minmax(0, 1fr) auto;
    }

    .settings-row-inline-control .settings-row-control {
        padding-top: 0.25rem;
        justify-content: flex-end;
        width: auto;
    }
}
</style>
