<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref, watch } from 'vue'

import type { NbtKind } from '../commands/types.ts'
import CommandGeneratorItemNbt from './CommandGeneratorItemNbt.vue'
import CommandGeneratorNbtAdvanced from './CommandGeneratorNbtAdvanced.vue'

const props = defineProps<{
    modelValue?: string
    kind?: NbtKind
    version?: string
    forceDataComponents?: boolean
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    simple: { id: 'app.lab.command-generator.nbt.simple', defaultMessage: 'Simple' },
    advanced: { id: 'app.lab.command-generator.nbt.advanced', defaultMessage: 'Advanced' },
})

const mode = ref<'simple' | 'advanced'>(props.kind === 'item' ? 'simple' : 'advanced')

watch(
    () => props.kind,
    (kind) => {
        mode.value = kind === 'item' ? 'simple' : 'advanced'
    },
)
</script>

<template>
    <div class="flex min-w-0 flex-col gap-4">
        <div v-if="kind === 'item'" class="flex flex-wrap gap-1">
            <Button
                type="quiet"
                size="sm"
                class="nbt-mode-tab"
                :highlighted="mode === 'simple'"
                :aria-pressed="mode === 'simple'"
                @click="mode = 'simple'"
            >
                {{ formatMessage(messages.simple) }}
            </Button>
            <Button
                type="quiet"
                size="sm"
                class="nbt-mode-tab"
                :highlighted="mode === 'advanced'"
                :aria-pressed="mode === 'advanced'"
                @click="mode = 'advanced'"
            >
                {{ formatMessage(messages.advanced) }}
            </Button>
        </div>

        <CommandGeneratorItemNbt
            v-if="kind === 'item' && mode === 'simple'"
            :model-value="modelValue"
            :version="version"
            :force-data-components="forceDataComponents"
            @update:model-value="emit('update:modelValue', $event)"
        />
        <CommandGeneratorNbtAdvanced
            v-else
            :model-value="modelValue"
            :kind="kind"
            :version="version"
            :force-data-components="forceDataComponents"
            @update:model-value="emit('update:modelValue', $event)"
        />
    </div>
</template>
