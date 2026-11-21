<script setup lang="ts">
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { ref, watch } from 'vue'

import type { NbtKind } from '@/lab/command-generator/commands/types.ts'
import type { NbtDomain } from '@/lab/command-generator/nbt/schema.ts'

import CommandGeneratorItemNbt from './CommandGeneratorItemNbt.vue'
import CommandGeneratorNbtAdvanced from './CommandGeneratorNbtAdvanced.vue'
import CommandGeneratorNbtSource from './CommandGeneratorNbtSource.vue'

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
    source: { id: 'app.lab.command-generator.nbt.source', defaultMessage: 'Source' },
})

type Mode = 'simple' | 'advanced' | 'source'

const mode = ref<Mode>(props.kind === 'item' ? 'simple' : 'advanced')
/** 源码模式一旦打开就留在 DOM 里：切走再切回来不能丢掉没提交的草稿。 */
const sourceMounted = ref(mode.value === 'source')

watch(
    () => props.kind,
    (kind) => {
        mode.value = kind === 'item' ? 'simple' : 'advanced'
    },
)

watch(mode, (next) => {
    if (next === 'source') sourceMounted.value = true
})
</script>

<template>
    <div class="flex min-w-0 flex-col gap-4">
        <div class="flex flex-wrap gap-1">
            <Button
                v-if="kind === 'item'"
                :type="mode === 'simple' ? 'chip' : 'quiet'"
                color="brand"
                size="sm"
                class="nbt-mode-tab"
                :aria-pressed="mode === 'simple'"
                @click="mode = 'simple'"
            >
                {{ formatMessage(messages.simple) }}
            </Button>
            <Button
                :type="mode === 'advanced' ? 'chip' : 'quiet'"
                color="brand"
                size="sm"
                class="nbt-mode-tab"
                :aria-pressed="mode === 'advanced'"
                @click="mode = 'advanced'"
            >
                {{ formatMessage(messages.advanced) }}
            </Button>
            <Button
                :type="mode === 'source' ? 'chip' : 'quiet'"
                color="brand"
                size="sm"
                class="nbt-mode-tab"
                :aria-pressed="mode === 'source'"
                @click="mode = 'source'"
            >
                {{ formatMessage(messages.source) }}
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
            v-else-if="mode === 'advanced'"
            :model-value="modelValue"
            :kind="kind"
            :version="version"
            :force-data-components="forceDataComponents"
            :domain="domain"
            :target-id="targetId"
            @update:model-value="emit('update:modelValue', $event)"
        />
        <CommandGeneratorNbtSource
            v-if="sourceMounted"
            v-show="mode === 'source'"
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
