<script setup lang="ts">
import { EyeIcon } from '@modrinth/assets'
import { computed } from 'vue'

import Combobox from '#ui/components/base/Combobox.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { MAX_RESULTS_OPTIONS } from '#ui/utils/browse-max-results'

const props = defineProps<{ options?: number[]; disabled?: boolean }>()
const pageSize = defineModel<number>({ required: true })
const sizes = computed(() =>
    (props.options ?? MAX_RESULTS_OPTIONS).map((value) => ({ value, label: String(value) })),
)
const { formatMessage } = useVIntl()
const messages = defineMessages({
    viewPrefix: { id: 'browse.view-prefix', defaultMessage: 'View:' },
})
</script>

<template>
    <Combobox
        v-model="pageSize"
        v-tooltip="{ content: formatMessage(messages.viewPrefix), triggers: ['hover'] }"
        :disabled="disabled"
        width="content"
        :options="sizes"
        trigger-class="!min-h-10"
    >
        <template #prefix>
            <EyeIcon class="size-5 shrink-0 text-[var(--color-text-default)]" />
        </template>
        <template #selected="{ label }">
            <span class="font-semibold text-[var(--color-text-tertiary)]">{{ label }}</span>
        </template>
    </Combobox>
</template>
