<script setup lang="ts">
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref, useId, watch } from 'vue'

import { messages } from '../../messages'
import { parseEnvVars, serializeEnvVars } from './env-vars'

/**
 * Environment variable field of the app (apps/app-frontend/src/components/ui/
 * EnvironmentVariablesInput.vue): edits `NAME=value` entries as one line and
 * only emits a parsed list while the text is valid, flagging it otherwise.
 */
defineOptions({ inheritAttrs: false })
const props = defineProps<{ modelValue: [string, string][] | undefined | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: [string, string][]] }>()
const { formatMessage } = useVIntl()
const errorId = `${useId()}-error`
const draft = ref(serializeEnvVars(props.modelValue))
const parsedDraft = computed(() => {
    try {
        return parseEnvVars(draft.value)
    } catch {
        return null
    }
})

watch(draft, () => {
    if (
        parsedDraft.value !== null &&
        JSON.stringify(parsedDraft.value) !== JSON.stringify(props.modelValue)
    ) {
        emit('update:modelValue', parsedDraft.value)
    }
})

watch(
    () => props.modelValue,
    (value) => {
        if (JSON.stringify(value) !== JSON.stringify(parsedDraft.value))
            draft.value = serializeEnvVars(value)
    },
    { deep: true },
)
</script>

<template>
    <div class="flex w-full flex-col gap-2">
        <StyledInput
            v-bind="$attrs"
            v-model="draft"
            wrapper-class="w-full"
            :error="parsedDraft === null"
            :input-attrs="{
                'aria-invalid': parsedDraft === null || undefined,
                'aria-describedby': parsedDraft === null ? errorId : undefined,
            }"
        />
        <p
            v-if="parsedDraft === null"
            :id="errorId"
            class="m-0 text-sm text-[var(--color-red)]"
            role="alert"
        >
            {{ formatMessage(messages.appSettingsEnvironmentVariablesInvalidFormat) }}
        </p>
    </div>
</template>
