<script setup lang="ts">
/**
 * NBT 路径的逐层选择器。
 *
 * 路径是「走进去」的东西：`Inventory[0].id` 要一层层点下去，所以这里给的是
 * 面包屑 + 当前层的字段列表，点复合 / 列表行右边的箭头进下一层，点整行则是
 * 「就用这个标签」。手写仍然是权威来源——`Items[{id:"stone"}]` 这种过滤器
 * 点不出来，输入框随时可以改。
 */

import { ChevronRightIcon, SearchIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue'

import { availableFieldsFor, levelAtPath } from '@/lab/command-generator/nbt/catalog.ts'
import {
    fieldDescription,
    fieldLabel,
    NBT_TYPE_NAME,
    NBT_TYPE_SHORT,
} from '@/lab/command-generator/nbt/labels.ts'
import {
    formatNbtPath,
    type NbtPathSegment,
    parseNbtPath,
} from '@/lab/command-generator/nbt/path.ts'
import {
    childrenOf,
    elementOf,
    formFor,
    type NbtDomain,
    type NbtFieldSpec,
    type NbtValueForm,
    searchFields,
} from '@/lab/command-generator/nbt/schema.ts'

const props = defineProps<{
    modelValue?: string
    version?: string
    /** 没有目录域时（`/data storage`）没有候选可列，只能手写。 */
    domain?: NbtDomain
    targetId?: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: string]
}>()

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
    title: {
        id: 'app.lab.command-generator.nbt.path.title',
        defaultMessage: 'Pick a tag path',
    },
    search: {
        id: 'app.lab.command-generator.nbt.path.search',
        defaultMessage: 'Search tags at this level',
    },
    root: {
        id: 'app.lab.command-generator.nbt.path.root',
        defaultMessage: 'Root',
    },
    use: {
        id: 'app.lab.command-generator.nbt.path.use',
        defaultMessage: 'Use this path',
    },
    clear: {
        id: 'app.lab.command-generator.nbt.path.clear',
        defaultMessage: 'Clear path',
    },
    empty: {
        id: 'app.lab.command-generator.nbt.path.empty',
        defaultMessage: 'The catalog documents no tags here. Type the rest by hand.',
    },
    element: {
        id: 'app.lab.command-generator.nbt.path.element',
        defaultMessage: 'Inside a list entry: these are the tags of one entry.',
    },
    descend: {
        id: 'app.lab.command-generator.nbt.path.descend',
        defaultMessage: 'Open the sub tags of {key}',
    },
})

const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const searchInput = useTemplateRef<InstanceType<typeof StyledInput>>('searchInput')

const version = computed(() => props.version ?? '1.21')
const lookup = computed(() => ({
    domain: props.domain ?? 'entity',
    version: version.value,
    targetId: props.targetId,
}))

/** 已经走到的位置。键名与下标混着存，序列化交给 `formatNbtPath`。 */
const picked = ref<NbtPathSegment[]>([])
const search = ref('')
const activeIndex = ref(0)

/** 列表元素不贡献键名，所以查下一层时只按键名走。 */
const keyPath = computed(() =>
    picked.value
        .filter(
            (segment): segment is Extract<NbtPathSegment, { kind: 'key' }> =>
                segment.kind === 'key',
        )
        .map((segment) => segment.name),
)

const level = computed<{ fields: NbtFieldSpec[]; element?: NbtValueForm }>(() => {
    if (!props.domain) return { fields: [] }
    if (!keyPath.value.length) return { fields: availableFieldsFor(lookup.value) }
    return levelAtPath(lookup.value, keyPath.value) ?? { fields: [] }
})

const results = computed(() =>
    searchFields(level.value.fields, search.value, version.value).map((result) => result.spec),
)

/** 刚走进列表元素时提示一句：这一层列的是单个条目的标签。 */
const insideElement = computed(() => picked.value[picked.value.length - 1]?.kind === 'index')

watch(results, () => {
    activeIndex.value = 0
})

const crumbs = computed(() =>
    picked.value.map((segment, index) => ({ index, text: crumbText(segment) })),
)

function crumbText(segment: NbtPathSegment): string {
    switch (segment.kind) {
        case 'key':
            return segment.name
        case 'index':
            return `[${segment.index}]`
        case 'slice':
            return `[${segment.from ?? ''}..${segment.to ?? ''}]`
        case 'all':
            return '[]'
        case 'filter':
            return segment.raw
    }
}

const describe = (spec: NbtFieldSpec) =>
    props.domain ? fieldDescription(spec, props.domain, formatMessage) : (spec.desc ?? '')

/**
 * 能下钻的写法：复合有子字段，列表的元素有子字段。
 *
 * 只认**默认写法**——下一层是 `levelAtPath` 按默认写法算出来的，如果这里挑
 * 另一种写法下钻（`pattern` 是字符串、也能写成复合），进去会是空列表。
 */
function descendForm(spec: NbtFieldSpec): NbtValueForm | undefined {
    const form = formFor(spec, version.value)
    if (!form) return undefined
    const descendable = childrenOf(form).length > 0 || childrenOf(elementOf(form)).length > 0
    return descendable ? form : undefined
}

function canDescend(spec: NbtFieldSpec): boolean {
    return Boolean(descendForm(spec))
}

function typeBadge(spec: NbtFieldSpec): string {
    const form = formFor(spec, version.value)
    if (!form) return ''
    if (form.type === 'list' && form.element) return `[${NBT_TYPE_SHORT[form.element.type]}]`
    return NBT_TYPE_SHORT[form.type]
}

function typeTitle(spec: NbtFieldSpec): string {
    const form = formFor(spec, version.value)
    if (!form) return ''
    if (form.type === 'list' && form.element) return `List of ${NBT_TYPE_NAME[form.element.type]}`
    return NBT_TYPE_NAME[form.type]
}

function show() {
    picked.value = parseNbtPath(props.modelValue ?? '')
    search.value = ''
    activeIndex.value = 0
    modal.value?.show()
    void nextTick(() => searchInput.value?.focus())
}

function hide() {
    modal.value?.hide()
}

/** 整行点击：把这一层选中的键接到路径末尾，然后收工。 */
function useField(spec: NbtFieldSpec) {
    emit('update:modelValue', formatNbtPath([...picked.value, keySegment(spec)]))
    hide()
}

/** 右边箭头：走进去，列表再补一个 `[0]`，下一层就是元素的标签。 */
function descend(spec: NbtFieldSpec) {
    const form = descendForm(spec)
    if (!form) return
    const next: NbtPathSegment[] = [...picked.value, keySegment(spec)]
    if (form.type === 'list') next.push({ kind: 'index', index: 0 })
    picked.value = next
    search.value = ''
    void nextTick(() => searchInput.value?.focus())
}

function keySegment(spec: NbtFieldSpec): NbtPathSegment {
    return { kind: 'key', name: spec.key, quoted: false }
}

/** 点面包屑回到某一层；`-1` 是根。 */
function jumpTo(index: number) {
    picked.value = index < 0 ? [] : picked.value.slice(0, index + 1)
    search.value = ''
}

function useCurrent() {
    emit('update:modelValue', formatNbtPath(picked.value))
    hide()
}

function clearPath() {
    picked.value = []
    emit('update:modelValue', '')
    hide()
}

function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
        event.preventDefault()
        const spec = results.value[activeIndex.value]
        if (spec) useField(spec)
        return
    }
    if (event.key === 'ArrowDown') {
        event.preventDefault()
        const total = results.value.length
        if (total) activeIndex.value = (activeIndex.value + 1) % total
        return
    }
    if (event.key === 'ArrowUp') {
        event.preventDefault()
        const total = results.value.length
        if (total) activeIndex.value = (activeIndex.value - 1 + total) % total
        return
    }
    if (event.key === 'Escape') hide()
}

defineExpose({ show, hide })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(640px, calc(100vw - 2rem))"
        max-width="640px"
        scrollable
        max-content-height="min(36rem, 78vh)"
    >
        <div class="flex h-[min(30rem,64vh)] min-h-0 min-w-0 flex-col gap-3">
            <nav
                class="flex min-w-0 flex-wrap items-center gap-1"
                :aria-label="formatMessage(messages.title)"
            >
                <button
                    type="button"
                    class="rounded px-1.5 py-0.5 font-mono text-[0.625rem] text-[var(--color-text-tertiary)] hover:bg-surface-4"
                    @click="jumpTo(-1)"
                >
                    {{ formatMessage(messages.root) }}
                </button>
                <template v-for="crumb in crumbs" :key="crumb.index">
                    <ChevronRightIcon
                        class="h-3 w-3 shrink-0 text-[var(--color-text-tertiary)]"
                        aria-hidden="true"
                    />
                    <button
                        type="button"
                        class="rounded bg-surface-4 px-1.5 py-0.5 font-mono text-[0.625rem] text-[var(--color-text-primary)] hover:bg-surface-5"
                        @click="jumpTo(crumb.index)"
                    >
                        {{ crumb.text }}
                    </button>
                </template>
            </nav>

            <StyledInput
                ref="searchInput"
                v-model="search"
                class="w-full"
                :icon="SearchIcon"
                :placeholder="formatMessage(messages.search)"
                clearable
                @keydown="onKeydown"
            />

            <p v-if="insideElement" class="m-0 text-xs leading-5 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.element) }}
            </p>

            <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
                <p
                    v-if="!results.length"
                    class="px-1 py-6 text-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.empty) }}
                </p>
                <div
                    v-for="(spec, index) in results"
                    :key="`${index}-${spec.key}`"
                    class="flex min-w-0 items-center gap-1 rounded-lg"
                    :class="index === activeIndex ? 'bg-surface-4' : 'hover:bg-surface-3'"
                >
                    <button
                        type="button"
                        class="flex min-w-0 flex-1 items-start gap-3 rounded-lg px-2 py-2 text-left focus-visible:outline-brand"
                        @click="useField(spec)"
                        @mouseenter="activeIndex = index"
                    >
                        <span
                            v-tooltip="typeTitle(spec)"
                            class="mt-0.5 w-9 shrink-0 rounded bg-surface-5 px-1 py-0.5 text-center font-mono text-[0.625rem] leading-4 text-[var(--color-text-secondary)]"
                            >{{ typeBadge(spec) }}</span
                        >
                        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span class="flex min-w-0 flex-wrap items-baseline gap-2">
                                <span
                                    class="truncate font-mono text-sm text-[var(--color-text-primary)]"
                                    >{{ spec.key }}</span
                                >
                                <span class="truncate text-xs text-[var(--color-text-secondary)]">{{
                                    fieldLabel(spec, locale)
                                }}</span>
                                <span
                                    v-if="spec.since"
                                    class="rounded bg-surface-5 px-1 font-mono text-[0.625rem] text-[var(--color-text-tertiary)]"
                                    >{{ spec.since }}+</span
                                >
                            </span>
                            <span
                                v-if="describe(spec)"
                                class="line-clamp-2 text-xs leading-4 text-[var(--color-text-tertiary)]"
                                >{{ describe(spec) }}</span
                            >
                        </span>
                    </button>
                    <Button
                        v-if="canDescend(spec)"
                        v-tooltip="formatMessage(messages.descend, { key: spec.key })"
                        type="quiet"
                        size="sm"
                        icon-only
                        :label="formatMessage(messages.descend, { key: spec.key })"
                        @click="descend(spec)"
                    >
                        <ChevronRightIcon />
                    </Button>
                </div>
            </div>

            <div
                class="flex min-w-0 items-center gap-2 border-0 border-t border-solid border-surface-5 pt-3"
            >
                <Button type="outlined" class="min-w-0 flex-1" @click="useCurrent">
                    {{ formatMessage(messages.use) }}
                </Button>
                <Button type="quiet" @click="clearPath">
                    {{ formatMessage(messages.clear) }}
                </Button>
            </div>
        </div>
    </NewModal>
</template>
