<script setup lang="ts">
import { Card } from '@modrinth/ui'

withDefaults(
    defineProps<{
        title?: string
        description?: string
        /**
         * Anchor for the settings search. Sections are the scroll targets, so
         * the id belongs on the heading; passing it here keeps callers from
         * hand-writing a heading just to carry one.
         */
        titleId?: string
    }>(),
    {
        title: undefined,
        description: undefined,
        titleId: undefined,
    },
)
</script>

<template>
    <section class="flex min-w-0 flex-col gap-3">
        <header
            v-if="title || description || $slots.header || $slots.extra"
            class="settings-section-header flex items-start justify-between gap-4"
        >
            <div class="min-w-0">
                <h2
                    v-if="title"
                    :id="titleId"
                    :tabindex="titleId ? -1 : undefined"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ title }}
                </h2>
                <p
                    v-if="description"
                    class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]"
                >
                    {{ description }}
                </p>
                <slot name="header" />
            </div>
            <slot name="extra" />
        </header>

        <Card class="settings-section-card">
            <slot />
        </Card>
    </section>
</template>

<style scoped>
.settings-section-card {
    margin: 0;
    padding: 0;
    /* One step above the shell panels (`surface-2`) so a section still reads as
                raised once it sits on them -- at the same level the card would be
                invisible against its own panel. This also matches `.base-card`, the
                shared card class used elsewhere. The border matters in the light theme,
                where `surface-2` and `surface-3` differ by only ~1.03:1. */
    background: var(--surface-3);
    border-color: var(--surface-4);
    border-radius: var(--radius-md);
}

@media (max-width: 700px) {
    .settings-section-header {
        flex-direction: column;
        gap: var(--gap-sm);
    }
}
</style>
