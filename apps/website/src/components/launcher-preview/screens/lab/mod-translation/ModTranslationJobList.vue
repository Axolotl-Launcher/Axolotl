<script setup lang="ts">
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

/**
 * The empty state of `components/lab/mod-translation/ModTranslationJobList.vue`:
 * translation jobs run in the desktop app, so this window never has any.
 */
const { formatMessage } = useVIntl()

const messages = defineMessages({
    noJobs: {
        id: 'app.lab.mod-translation.no-jobs',
        defaultMessage: 'Started jobs will appear here.',
    },
})
</script>

<template>
    <div class="flex flex-col gap-2">
        <EmptyState :heading="formatMessage(messages.noJobs)" type="empty" />
    </div>
</template>
