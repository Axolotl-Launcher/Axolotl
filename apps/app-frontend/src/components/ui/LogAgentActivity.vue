<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'

import type { ActivityEntry } from '@/logagent/progress'

defineProps<{ entries: ActivityEntry[]; busy: boolean; elapsed: number }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    title: { id: 'app.logagent.activity.title', defaultMessage: 'Analysis progress' },
    elapsed: { id: 'app.logagent.activity.elapsed', defaultMessage: '{seconds, number} s elapsed' },
    model: {
        id: 'app.logagent.activity.model',
        defaultMessage: 'Reviewing the available evidence',
    },
    list: { id: 'app.logagent.activity.list', defaultMessage: 'Checking which logs are available' },
    read: { id: 'app.logagent.activity.read', defaultMessage: 'Reading relevant log context' },
    grep: { id: 'app.logagent.activity.grep', defaultMessage: 'Searching logs for related clues' },
    external: {
        id: 'app.logagent.activity.external',
        defaultMessage: 'Checking additional evidence',
    },
    query: { id: 'app.logagent.activity.query', defaultMessage: 'Search: {query}' },
    lines: { id: 'app.logagent.activity.lines', defaultMessage: '{count, number} lines returned' },
    range: { id: 'app.logagent.activity.range', defaultMessage: 'From line {line, number}' },
    done: { id: 'app.logagent.activity.done', defaultMessage: 'Done' },
    stopped: { id: 'app.logagent.activity.stopped', defaultMessage: 'Stopped' },
    working: { id: 'app.logagent.activity.working', defaultMessage: 'In progress' },
})
function label(entry: ActivityEntry) {
    if (entry.kind === 'model') return formatMessage(messages.model)
    return formatMessage(
        entry.name === 'list_log_files'
            ? messages.list
            : entry.name === 'read_log_file'
              ? messages.read
              : entry.name === 'grep_log_file'
                ? messages.grep
                : messages.external,
    )
}
</script>

<template>
    <details
        v-if="entries.length"
        :open="busy"
        class="mb-4 rounded-lg border border-solid border-surface-4 bg-surface-3 p-3"
    >
        <summary class="cursor-pointer text-sm font-semibold">
            {{ formatMessage(messages.title) }}
            <span class="ml-2 text-xs font-normal text-[var(--color-text-tertiary)]">{{
                formatMessage(messages.elapsed, { seconds: elapsed })
            }}</span>
        </summary>
        <ol
            class="m-0 mt-3 flex max-h-48 list-none flex-col gap-3 overflow-y-auto p-0 text-sm"
            aria-live="polite"
            aria-relevant="additions"
        >
            <li v-for="entry in entries" :key="entry.id" class="flex min-w-0 gap-2">
                <span
                    aria-hidden="true"
                    class="mt-1 size-1.5 shrink-0 rounded-full"
                    :class="entry.state === 'processing' ? 'bg-brand' : 'bg-surface-5'"
                />
                <div class="min-w-0 flex-1">
                    <p
                        v-if="entry.kind === 'note'"
                        class="m-0 whitespace-pre-wrap break-words text-[var(--color-text-tertiary)]"
                    >
                        {{ entry.text }}
                    </p>
                    <template v-else>
                        <div class="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
                            <span>{{ label(entry) }}</span>
                            <span class="text-xs text-[var(--color-text-tertiary)]">{{
                                formatMessage(
                                    entry.state === 'processing'
                                        ? messages.working
                                        : entry.state === 'ready'
                                          ? messages.done
                                          : messages.stopped,
                                )
                            }}</span>
                        </div>
                        <p
                            v-if="entry.file || entry.query || entry.startLine"
                            class="m-0 mt-1 break-words text-xs text-[var(--color-text-tertiary)]"
                        >
                            <span v-if="entry.file">{{ entry.file }}</span>
                            <span v-if="entry.query" class="ml-2">{{
                                formatMessage(messages.query, { query: entry.query })
                            }}</span>
                            <span v-if="entry.startLine" class="ml-2">{{
                                formatMessage(messages.range, { line: entry.startLine })
                            }}</span>
                        </p>
                        <p
                            v-if="entry.lines !== undefined"
                            class="m-0 mt-1 text-xs text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.lines, { count: entry.lines }) }}
                        </p>
                    </template>
                </div>
            </li>
        </ol>
    </details>
</template>
