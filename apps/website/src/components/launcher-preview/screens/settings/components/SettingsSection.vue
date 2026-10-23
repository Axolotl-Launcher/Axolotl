<script setup lang="ts">
import Card from '@modrinth/ui/src/components/base/Card.vue'

/**
 * Settings section of the desktop app (components/ui/settings/SettingsSection.vue):
 * an optional header (title carrying the search anchor, description, `#header`
 * and `#extra` slots) above the shared card, restyled flush.
 */
withDefaults(
    defineProps<{
        title?: string
        description?: string
        /** Anchor for settings search; it belongs on the heading. */
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
       raised once it sits on them; the border carries the light theme, where
       `surface-2` and `surface-3` differ by only ~1.03:1. */
    background: var(--surface-3);
    border-color: var(--surface-4);
    border-radius: var(--radius-md);
}
</style>
