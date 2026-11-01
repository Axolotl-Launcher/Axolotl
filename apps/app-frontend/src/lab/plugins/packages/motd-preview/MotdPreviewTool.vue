<script setup lang="ts">
import { defineMessages, MinecraftFormattedText, useVIntl } from '@modrinth/ui'
import { ref, watch } from 'vue'

import { useLabPluginContext } from '@/lab/plugins/types'

const { formatMessage } = useVIntl()
const context = useLabPluginContext()

const messages = defineMessages({
    label: { id: 'app.lab.plugins.motd-preview.label', defaultMessage: 'Server list text' },
    placeholder: {
        id: 'app.lab.plugins.motd-preview.placeholder',
        defaultMessage: '§6Axolotl §7server',
    },
    preview: { id: 'app.lab.plugins.motd-preview.preview', defaultMessage: 'Preview' },
})

/** Text the user types. Stored per plugin, so it survives a restart. */
const text = ref(context.storage.read('text', '§6Axolotl §7server\n§eWelcome, §bplayer§e!'))

watch(text, (value) => context.storage.write('text', value))
</script>

<template>
    <div class="grid grid-cols-1 gap-4 min-[900px]:grid-cols-2">
        <section class="flex flex-col gap-2">
            <label class="text-sm font-bold text-[var(--color-text-primary)]" for="motd-input">
                {{ formatMessage(messages.label) }}
            </label>
            <textarea
                id="motd-input"
                v-model="text"
                :placeholder="formatMessage(messages.placeholder)"
                spellcheck="false"
                class="min-h-[9rem] w-full resize-y rounded-[var(--radius-lg)] border border-surface-5 bg-surface-2 p-3 font-mono text-sm text-[var(--color-text-primary)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand"
            />
        </section>

        <section class="flex flex-col gap-2">
            <h2 class="m-0 text-sm font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.preview) }}
            </h2>
            <div
                class="min-h-[9rem] whitespace-pre-wrap break-words rounded-[var(--radius-lg)] border border-surface-5 bg-surface-2 p-3 font-mono text-sm leading-6"
            >
                <MinecraftFormattedText :text="text" />
            </div>
        </section>
    </div>
</template>
