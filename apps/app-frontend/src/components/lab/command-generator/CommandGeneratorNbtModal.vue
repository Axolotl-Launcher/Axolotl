<script setup lang="ts">
import { Button, commonMessages, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref, useTemplateRef } from 'vue'

import type { NbtKind } from '@/lab/command-generator/commands/types.ts'
import type { NbtDomain } from '@/lab/command-generator/nbt/schema.ts'

import CommandGeneratorNbtEditor from './CommandGeneratorNbtEditor.vue'

/**
 * 把 NBT 编辑器搬进弹窗。
 *
 * 选择器的 `nbt=` 挤在一行选择器文本里，放不下树与源码；这里原样复用
 * `CommandGeneratorNbtEditor`，行为与命令参数里的编辑器完全一致，
 * 只是提交边界挪到了「应用」按钮：关闭前不改写选择器文本，取消才有意义。
 */
defineProps<{
    kind?: NbtKind
    version?: string
    forceDataComponents?: boolean
    /** 目录域，决定搜索面板列哪一套字段。 */
    domain?: NbtDomain
    /** 已知实体 / 物品 ID 时，只列出适用于它的字段。 */
    targetId?: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    title: {
        id: 'app.lab.command-generator.selector.nbt.editor-title',
        defaultMessage: 'Edit NBT data',
    },
    context: {
        id: 'app.lab.command-generator.selector.nbt.editor-context',
        defaultMessage: 'Only fields that apply to {target} are listed.',
    },
    apply: {
        id: 'app.lab.command-generator.selector.nbt.apply',
        defaultMessage: 'Apply',
    },
})

const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const editor = useTemplateRef<InstanceType<typeof CommandGeneratorNbtEditor>>('editor')

/** 草稿：只有「应用」才把它写回选择器。 */
const draft = ref('')
/** 打开时的原值，用来判断有没有真的改过。 */
let original = ''

/**
 * 初值由调用方在打开时传入，而不是走 prop：父组件改完 ref 到子组件拿到新
 * prop 要等一次渲染，同一 tick 里 `show()` 会读到上一次的草稿。
 */
function show(initial: string, event?: MouseEvent) {
    original = initial
    draft.value = initial
    modal.value?.show(event)
}

function apply() {
    // 源码模式的写入是防抖 + blur 触发的：点按钮时最后几个字符可能还在等
    const committed = editor.value?.commit()
    // 源码还没写完：留在弹窗里，让用户对着编辑器里的报错改
    if (committed === false) return
    if (draft.value.trim() !== original.trim()) emit('update:modelValue', draft.value.trim())
    modal.value?.hide()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(680px, calc(100vw - 2rem))"
        max-width="680px"
        scrollable
        max-content-height="min(32rem, 76vh)"
    >
        <div class="flex min-w-0 flex-col gap-4">
            <p v-if="targetId" class="m-0 text-xs leading-5 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.context, { target: targetId }) }}
            </p>
            <CommandGeneratorNbtEditor
                ref="editor"
                :model-value="draft"
                :kind="kind ?? 'entity'"
                :version="version"
                :force-data-components="forceDataComponents"
                :domain="domain"
                :target-id="targetId"
                @update:model-value="draft = $event"
            />
        </div>
        <template #actions>
            <Button type="colored" @click="apply">{{ formatMessage(messages.apply) }}</Button>
            <Button @click="modal?.hide()">{{ formatMessage(commonMessages.cancelButton) }}</Button>
        </template>
    </NewModal>
</template>
