<script setup lang="ts">
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { ref, watch } from 'vue'

import type { NbtKind } from '@/lab/command-generator/commands/types.ts'
import type { NbtDomain } from '@/lab/command-generator/nbt/schema.ts'

import CommandGeneratorItemNbt from './CommandGeneratorItemNbt.vue'
import CommandGeneratorNbtAdvanced from './CommandGeneratorNbtAdvanced.vue'

const props = defineProps<{
    modelValue?: string
    kind?: NbtKind
    version?: string
    forceDataComponents?: boolean
    /** 目录域与目标实体 / 物品，决定高级模式里搜索面板的候选字段。 */
    domain?: NbtDomain
    targetId?: string
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
            :domain="domain"
            :target-id="targetId"
            @update:model-value="emit('update:modelValue', $event)"
        />
    </div>
</template>
