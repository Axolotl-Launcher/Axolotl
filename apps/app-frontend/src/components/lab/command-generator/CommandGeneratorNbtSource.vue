<script setup lang="ts">
import { FoldVerticalIcon, UnfoldVerticalIcon } from '@modrinth/assets'
import {
    attachSnbtCompletion,
    Button,
    CodeEditor,
    defineMessages,
    formatSnbt,
    type Monaco,
    type MonacoEditor,
    type MonacoEditorOptions,
    registerSnbt,
    type SnbtCompletionHandle,
    useVIntl,
} from '@modrinth/ui'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { errorText } from '@/helpers/error-text'
import type { NbtLookup } from '@/lab/command-generator/nbt/catalog.ts'
import { createNbtCompletionSource } from '@/lab/command-generator/nbt/completion-source.ts'
import { compareVersion, type NbtDomain } from '@/lab/command-generator/nbt/schema.ts'
import { parseNbt, serializeNbtDocument } from '@/lab/command-generator/nbt/snbt.ts'

const props = defineProps<{
    modelValue?: string
    kind?: 'item' | 'entity' | 'generic'
    version?: string
    forceDataComponents?: boolean
    domain?: NbtDomain
    targetId?: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
}>()

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
    hint: {
        id: 'app.lab.command-generator.nbt.source.hint',
        defaultMessage: 'Tab completes a tag; Ctrl+S applies the command.',
    },
    expand: {
        id: 'app.lab.command-generator.nbt.source.expand',
        defaultMessage: 'Expand to multiple lines',
    },
    compact: {
        id: 'app.lab.command-generator.nbt.source.compact',
        defaultMessage: 'Collapse to one line',
    },
    invalid: {
        id: 'app.lab.command-generator.nbt.source.invalid',
        defaultMessage: 'Syntax error: {message}',
    },
})

const options: MonacoEditorOptions = {
    minimap: { enabled: false },
    lineNumbers: 'on',
    fontSize: 13,
    tabSize: 4,
    insertSpaces: true,
    wordWrap: 'on',
    scrollBeyondLastLine: false,
    renderLineHighlight: 'none',
    overviewRulerLanes: 0,
    padding: { top: 8, bottom: 8 },
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
    suggestOnTriggerCharacters: true,
    quickSuggestions: { other: true, comments: false, strings: true },
    // Enter 换行、Tab 接受候选：手写时更接近普通编辑器。
    acceptSuggestionOnEnter: 'off',
    tabCompletion: 'on',
    fixedOverflowWidgets: true,
}

const version = computed(() => props.version ?? '1.21')

/**
 * 与高级面板一致：1.20.5 起物品只能是数据组件，版本号先说话；页面上的
 * 「使用 1.20.5+ 数据组件」开关只在 1.20–1.20.4 出现，那时才看开关。
 */
const useComponents = computed(() => {
    if (props.kind !== 'item') return false
    if (compareVersion(version.value, '1.20.5') >= 0) return true
    if (version.value === '1.20' || version.value.startsWith('1.20.')) {
        return Boolean(props.forceDataComponents)
    }
    return false
})

/**
 * 进入源码模式时铺好的骨架。
 *
 * 1.20.5 起物品的 NBT 换成了数据组件，命令里的写法是 `item[组件=值,…]`，
 * 所以根是**列表**；1.20.4 及以前物品写 `item{…}`，实体与方块实体的 NBT
 * 也都是复合标签，根是**花括号**。`/data` 这类路径参数不是容器，不给骨架。
 */
const skeleton = computed(() => {
    if (props.kind === 'item') return useComponents.value ? '[\n    \n]' : '{\n    \n}'
    if (props.kind === 'entity') return '{\n    \n}'
    return ''
})

/** 其他模式给的是一行压好的文本：进源码模式先排成多行，读起来才像代码。 */
function present(value: string): string {
    return value.trim() ? formatSnbt(value) : value
}

const text = ref(present(props.modelValue ?? '') || skeleton.value)
/** 文本就是刚铺的骨架，没有真正的内容。 */
const seeded = text.value === skeleton.value && Boolean(skeleton.value)
const error = ref('')

let completion: SnbtCompletionHandle | null = null
let lastEmitted = props.modelValue ?? ''
let timer: ReturnType<typeof setTimeout> | undefined

const components = computed(() => {
    if (props.kind !== 'item') return false
    const first = text.value.trimStart()[0]
    if (first === '[') return true
    if (first === '{') return false
    return useComponents.value
})

const domain = computed<NbtDomain | undefined>(() => {
    if (props.domain) return props.domain
    if (props.kind === 'item') return 'item'
    if (props.kind === 'entity') return 'entity'
    return undefined
})

const lookup = computed<NbtLookup | null>(() =>
    domain.value
        ? {
              domain: domain.value,
              version: version.value,
              targetId: props.targetId,
              components: components.value,
          }
        : null,
)

const source = createNbtCompletionSource(() => ({
    lookup: lookup.value,
    version: version.value,
    locale: locale.value,
    formatMessage: (descriptor) => formatMessage(descriptor),
}))

function onReady(editor: MonacoEditor, monaco: Monaco) {
    registerSnbt(monaco)
    const model = editor.getModel()
    if (!model) return
    completion = attachSnbtCompletion(editor, model, source)
    if (!seeded) return
    // 骨架是空的容器：光标放进最后一行，顺手把候选列表弹出来，省掉第一次 Tab。
    const offset = Math.max(0, skeleton.value.lastIndexOf('\n'))
    editor.setPosition(model.getPositionAt(offset))
    requestAnimationFrame(() => {
        editor.focus()
        editor.trigger('snbt-skeleton', 'editor.action.triggerSuggest', {})
    })
}

/** 解析并压成一行；语法不完整时抛错，由调用方显示。 */
function compact(): string {
    const raw = text.value.trim()
    // 骨架只是脚手架，没往里填东西时不该出现在命令里。
    if (!raw || raw === skeleton.value.trim()) return ''
    const parsed = parseNbt(raw)
    return serializeNbtDocument(parsed.nodes, parsed.components)
}

function publish(next: string) {
    error.value = ''
    if (next === lastEmitted) return
    lastEmitted = next
    emit('update:modelValue', next)
}

function commit() {
    try {
        publish(compact())
    } catch (cause) {
        error.value = errorText(cause)
    }
}

/** 按钮：把编辑器里的排版也换掉，方便确认最终会发出去什么。 */
function collapse() {
    try {
        const next = compact()
        // 还没填内容时别把骨架也抹掉，留着方便接着写。
        if (!next) return
        publish(next)
        text.value = next
    } catch (cause) {
        error.value = errorText(cause)
    }
}

/** 按钮：把一行展开成缩进好的多行，方便逐项改。 */
function expand() {
    const next = formatSnbt(text.value)
    if (next !== text.value) text.value = next
}

watch(text, () => {
    if (timer) clearTimeout(timer)
    timer = setTimeout(commit, 400)
})

// 版本跨过 1.20.5（或改回开关）时根形态会变：还停在旧骨架上就直接换掉。
watch(skeleton, (next, previous) => {
    const current = text.value.trim()
    if (!current || current === previous.trim()) text.value = next
})

watch(
    () => props.modelValue,
    (value) => {
        const next = value ?? ''
        if (next === lastEmitted) return
        lastEmitted = next
        // 别的模式改过之后拿到的还是一行，重新排版。
        text.value = present(next)
    },
)

onBeforeUnmount(() => {
    if (timer) clearTimeout(timer)
    completion?.dispose()
})
</script>

<template>
    <div class="flex min-w-0 flex-col gap-2">
        <div class="flex flex-wrap items-center justify-between gap-2">
            <span class="text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.hint) }}
            </span>
            <div class="flex items-center gap-2">
                <Button type="outlined" size="sm" @click="expand">
                    <UnfoldVerticalIcon class="h-4 w-4" />
                    {{ formatMessage(messages.expand) }}
                </Button>
                <Button type="outlined" size="sm" @click="collapse">
                    <FoldVerticalIcon class="h-4 w-4" />
                    {{ formatMessage(messages.compact) }}
                </Button>
            </div>
        </div>
        <div class="h-72 overflow-hidden rounded-xl border border-solid border-surface-4">
            <CodeEditor
                :content="text"
                language="snbt"
                file-path="nbt.snbt"
                :options="options"
                @update:content="text = $event"
                @blur="commit"
                @save="commit"
                @ready="onReady"
            />
        </div>
        <p v-if="error" class="m-0 text-sm text-red">
            {{ formatMessage(messages.invalid, { message: error }) }}
        </p>
    </div>
</template>
