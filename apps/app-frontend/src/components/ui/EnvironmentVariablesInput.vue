<script setup lang="ts">
import { defineMessage, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, ref, useId, watch } from 'vue'

import { parseEnvVars, serializeEnvVars } from '@/helpers/settings'

defineOptions({ inheritAttrs: false })
const props = defineProps<{ modelValue: [string, string][] | undefined | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: [string, string][]] }>()
const { formatMessage } = useVIntl()
const errorId = `${useId()}-error`
const invalidFormat = defineMessage({
    id: 'app.settings.environment-variables.invalid-format',
    defaultMessage:
        'Use NAME=value, separated by spaces. A name and an equals sign are required; the value may be empty or contain more equals signs.',
})
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
            {{ formatMessage(invalidFormat) }}
        </p>
    </div>
</template>
