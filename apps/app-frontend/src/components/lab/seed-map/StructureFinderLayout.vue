<script setup lang="ts">
/**
 * Shared shell for the structure finder pages.
 *
 * The seed-map finders differ only in their parameters and their result columns;
 * everything around them — the availability banner, the scan button, the empty /
 * error / loading states, the notification on completion and the layout itself
 * (results left, controls right) — is the same. Keeping one copy means a change
 * to that shell cannot drift between the two tools.
 */
import { CircleAlertIcon, SearchIcon, SpinnerIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    scan: { id: 'app.lab.structure-finder.scan', defaultMessage: 'Scan' },
    scanning: { id: 'app.lab.structure-finder.scanning', defaultMessage: 'Scanning…' },
    results: { id: 'app.lab.structure-finder.results', defaultMessage: 'Results' },
    emptyTitle: {
        id: 'app.lab.structure-finder.empty-title',
        defaultMessage: 'No results yet',
    },
    emptyBody: {
        id: 'app.lab.structure-finder.empty-body',
        defaultMessage: 'Fill in the parameters and start a scan.',
    },
    failed: {
        id: 'app.lab.structure-finder.failed',
        defaultMessage: 'The scan failed: {reason}',
    },
    unavailableTitle: {
        id: 'app.lab.structure-finder.unavailable-title',
        defaultMessage: 'Cannot scan',
    },
})

const props = defineProps<{
    title: string
    description: string
    /** Whether the tool can run here; `null` while it is still being probed. */
    available: boolean | null
    unavailableBody: string
    scanning: boolean
    error: string | null
    /** Whether a scan has produced a result (even an empty one). */
    hasResult: boolean
    canScan: boolean
}>()

const emit = defineEmits<{ scan: [] }>()

const blocked = computed(() => props.available === false)
</script>

<template>
    <div class="flex min-h-0 flex-1 flex-col gap-4 p-6">
        <header class="flex flex-col gap-1">
            <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                {{ props.title }}
            </h1>
            <p class="m-0 max-w-3xl text-sm text-[var(--color-text-default)]">
                {{ props.description }}
            </p>
        </header>

        <section
            v-if="blocked"
            class="flex items-start gap-3 rounded-lg border border-solid border-surface-4 bg-surface-2 p-4"
        >
            <CircleAlertIcon class="mt-0.5 size-5 shrink-0 text-[var(--color-orange)]" />
            <div class="flex flex-col gap-1">
                <strong class="text-sm">{{ formatMessage(messages.unavailableTitle) }}</strong>
                <span class="text-sm text-[var(--color-text-default)]">
                    {{ props.unavailableBody }}
                </span>
            </div>
        </section>

        <!-- Results on the left, controls on the right. -->
        <div class="flex min-h-0 flex-1 gap-4">
            <section class="flex min-w-0 flex-1 flex-col gap-2">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <h2 class="m-0 text-lg font-semibold">
                        {{ formatMessage(messages.results) }}
                    </h2>
                    <div class="flex flex-wrap items-center gap-2">
                        <slot name="summary" />
                    </div>
                </div>

                <div
                    v-if="props.error"
                    class="rounded-lg border border-solid border-surface-4 bg-surface-2 p-4 text-sm"
                >
                    {{ formatMessage(messages.failed, { reason: props.error }) }}
                </div>

                <div
                    v-else-if="!props.hasResult"
                    class="flex flex-1 flex-col items-center justify-center gap-1 rounded-lg border border-solid border-surface-4 bg-surface-2 p-8 text-center"
                >
                    <strong class="text-sm">{{ formatMessage(messages.emptyTitle) }}</strong>
                    <span class="text-sm text-[var(--color-text-default)]">
                        {{ formatMessage(messages.emptyBody) }}
                    </span>
                </div>

                <div v-else class="min-h-0 flex-1 overflow-auto">
                    <table class="w-full border-collapse text-sm">
                        <thead class="sticky top-0 bg-surface-1">
                            <tr class="text-left text-xs text-[var(--color-text-tertiary)]">
                                <slot name="columns" />
                            </tr>
                        </thead>
                        <tbody>
                            <slot name="rows" />
                        </tbody>
                    </table>
                </div>
            </section>

            <aside
                class="flex w-96 shrink-0 flex-col gap-3 self-start rounded-lg border border-solid border-surface-4 bg-surface-2 p-4"
            >
                <slot name="parameters" />

                <Button type="colored" :disabled="!props.canScan" @click="emit('scan')">
                    <SpinnerIcon v-if="props.scanning" class="size-4 animate-spin" />
                    <SearchIcon v-else class="size-4" />
                    {{
                        props.scanning
                            ? formatMessage(messages.scanning)
                            : formatMessage(messages.scan)
                    }}
                </Button>
            </aside>
        </div>
    </div>
</template>
