<script setup lang="ts">
import { XIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import BannerPreview from '@/components/lab/banner-designer/BannerPreview.vue'
import { colorNameMessages } from '@/lab/banner-designer/messages.ts'
import type { RecentBanner } from '@/lab/banner-designer/storage.ts'

const props = defineProps<{
    record: RecentBanner
}>()

const emit = defineEmits<{
    open: []
    remove: []
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    openRecent: {
        id: 'app.lab.banner-designer.open-recent',
        defaultMessage: 'Open banner',
    },
    removeRecent: {
        id: 'app.lab.banner-designer.remove-recent',
        defaultMessage: 'Remove from recent',
    },
    patternCount: {
        id: 'app.lab.banner-designer.pattern-count',
        defaultMessage: '{count, plural, one {# pattern} other {# patterns}}',
    },
})

const colorName = computed(() => formatMessage(colorNameMessages[props.record.design.baseColor]))
</script>

<template>
    <li
        class="flex min-w-0 items-center gap-3 rounded-[var(--radius-md)] border border-surface-5 bg-surface-2 p-2 transition-[border-color,background-color] hover:border-brand hover:bg-surface-4"
    >
        <button
            type="button"
            class="flex min-w-0 flex-1 cursor-pointer items-center gap-3 border-0 bg-transparent p-0 text-left focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
            :aria-label="formatMessage(messages.openRecent)"
            @click="emit('open')"
        >
            <BannerPreview
                :design="record.design"
                :scale="1"
                :label="colorName"
                class="rounded-[2px]"
            />
            <span class="min-w-0">
                <strong
                    class="block truncate text-sm font-medium text-[var(--color-text-primary)]"
                    >{{ colorName }}</strong
                >
                <span class="block truncate text-xs text-[var(--color-text-tertiary)]">{{
                    formatMessage(messages.patternCount, { count: record.design.layers.length })
                }}</span>
            </span>
        </button>
        <Button
            v-tooltip="formatMessage(messages.removeRecent)"
            type="quiet"
            size="2xs"
            circular
            icon-only
            :aria-label="formatMessage(messages.removeRecent)"
            @click="emit('remove')"
            ><XIcon />
        </Button>
    </li>
</template>
