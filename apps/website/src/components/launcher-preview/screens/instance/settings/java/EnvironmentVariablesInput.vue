<script setup lang="ts">
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref, useId, watch } from 'vue'

import { environmentVariablesMessages } from '../../messages'

/** Port of `apps/app-frontend/src/components/ui/EnvironmentVariablesInput.vue`. */
defineOptions({ inheritAttrs: false })
const props = defineProps<{ modelValue: [string, string][] | undefined | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: [string, string][]] }>()
const { formatMessage } = useVIntl()
const errorId = `${useId()}-error`
const invalidFormat = environmentVariablesMessages.environmentVariablesInvalidFormat

/** `serializeEnvVars` of `helpers/settings.ts`. */
function serializeEnvVars(vars: [string, string][] | undefined | null): string {
    return (vars ?? []).map(([key, value]) => `${key}=${value}`).join(' ')
}

/** `parseEnvVars` of `helpers/settings.ts`. */
function parseEnvVars(input: string | undefined | null): [string, string][] {
    if (!input?.trim()) return []

    const vars: [string, string][] = []
    for (const entry of input.trim().split(/\s+/)) {
        const separator = entry.indexOf('=')
        if (separator <= 0 || entry.includes('\0')) {
            throw new Error(
                'Environment variables must use NAME=value and cannot contain null characters.',
            )
        }
        vars.push([entry.slice(0, separator), entry.slice(separator + 1)])
    }
    return vars
}

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
