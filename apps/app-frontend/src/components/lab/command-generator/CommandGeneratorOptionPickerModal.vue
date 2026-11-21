<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import { NewModal, StyledInput } from '@modrinth/ui'
import Fuse from 'fuse.js'
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue'

/**
 * 通用的「搜索 + 选择」弹窗。
 *
 * 选择器的筛选条件、实体类型、附魔与状态效果都走这一个组件：它们原先各用一套
 * 原生 `<datalist>` 或手写的绝对定位菜单，箭头点不开、样式不可控、也没有搜索。
 *
 * 形态与 `CommandGeneratorNbtPalette.vue` 保持一致（`NewModal` + `StyledInput`
 * 搜索 + 吸顶分组 + ↑↓/Enter），只是去掉了 NBT 专属的过滤开关与类型徽标。
 */

export interface PickerOption {
    /** 选中的值。 */
    id: string
    /** 主文本，等宽字体。例如 `distance=` 或 `minecraft:sharpness`。 */
    label: string
    /** 次要说明（中英文名、中文短标签）。 */
    detail?: string
    /** 第三行说明。 */
    hint?: string
    /** 额外搜索词：把中文名、说明原文都放进来，搜「距离」也能命中 distance。 */
    keywords?: string
}

export interface PickerGroup {
    id: string
    label?: string
    options: PickerOption[]
}

const props = defineProps<{
    title: string
    searchPlaceholder: string
    emptyText: string
    groups: PickerGroup[]
    /** 已经用过的值，列表上打个标记（不阻止再选，只提示）。 */
    takenIds?: string[]
    takenLabel?: string
}>()

const emit = defineEmits<{
    pick: [id: string]
}>()

const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const searchInput = useTemplateRef<InstanceType<typeof StyledInput>>('searchInput')
const search = ref('')
const activeIndex = ref(0)

const flatOptions = computed(() => props.groups.flatMap((group) => group.options))

const fuse = computed(
    () =>
        new Fuse(flatOptions.value, {
            keys: ['label', 'id', 'detail', 'hint', 'keywords'],
            threshold: 0.35,
            ignoreLocation: true,
        }),
)

const matchedIds = computed(() => {
    const query = search.value.trim()
    if (!query) return undefined
    return new Set(fuse.value.search(query).map((result) => result.item.id))
})

const visibleGroups = computed(() => {
    const matched = matchedIds.value
    return props.groups
        .map((group) => ({
            ...group,
            options: matched
                ? group.options.filter((option) => matched.has(option.id))
                : group.options,
        }))
        .filter((group) => group.options.length > 0)
})

/** 键盘上下移动时按展平后的顺序走，和视觉顺序一致。 */
const flatResults = computed(() => visibleGroups.value.flatMap((group) => group.options))

const taken = computed(() => new Set(props.takenIds ?? []))

watch(flatResults, () => {
    activeIndex.value = 0
})

function show(event?: MouseEvent) {
    search.value = ''
    activeIndex.value = 0
    modal.value?.show(event)
    void nextTick(() => searchInput.value?.focus())
}

function hide() {
    modal.value?.hide()
}

function pick(option: PickerOption) {
    emit('pick', option.id)
    modal.value?.hide()
}

function move(delta: number) {
    const total = flatResults.value.length
    if (total === 0) return
    activeIndex.value = (activeIndex.value + delta + total) % total
}

function onKeydown(event: KeyboardEvent) {
    if (event.key === 'ArrowDown') {
        event.preventDefault()
        move(1)
        return
    }
    if (event.key === 'ArrowUp') {
        event.preventDefault()
        move(-1)
        return
    }
    if (event.key === 'Enter') {
        event.preventDefault()
        const option = flatResults.value[activeIndex.value]
        if (option) pick(option)
    }
}

defineExpose({ show, hide })
</script>

<template>
    <NewModal
        ref="modal"
        :header="title"
        width="min(640px, calc(100vw - 2rem))"
        max-width="640px"
        scrollable
        max-content-height="min(36rem, 78vh)"
    >
        <!--
            高度必须定死：列表要成为唯一滚动容器。给成 flex-1 而没有高度约束时，
            列表会撑到内容高度、自己不滚，滚轮落在这块「假滚动容器」上会被
            overscroll-contain 截断。定高同时避免搜索时弹窗跟着结果数量跳。
        -->
        <div class="flex h-[min(30rem,64vh)] min-h-0 min-w-0 flex-col gap-3">
            <StyledInput
                ref="searchInput"
                v-model="search"
                class="w-full shrink-0"
                :icon="SearchIcon"
                :placeholder="searchPlaceholder"
                clearable
                @keydown="onKeydown"
            />

            <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
                <p
                    v-if="!flatResults.length"
                    class="px-1 py-6 text-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ emptyText }}
                </p>
                <section v-for="group in visibleGroups" :key="group.id" class="mb-3">
                    <h3
                        v-if="group.label"
                        class="sticky top-0 z-[1] bg-surface-3 px-1 py-1 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                    >
                        {{ group.label }}
                    </h3>
                    <button
                        v-for="option in group.options"
                        :key="`${group.id}-${option.id}`"
                        type="button"
                        class="flex w-full min-w-0 items-start gap-3 rounded-lg px-2 py-2 text-left hover:bg-surface-4 focus-visible:outline-brand"
                        :class="{
                            'bg-surface-4': flatResults[activeIndex]?.id === option.id,
                        }"
                        @click="pick(option)"
                        @mouseenter="activeIndex = flatResults.indexOf(option)"
                    >
                        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span class="flex min-w-0 flex-wrap items-baseline gap-2">
                                <span
                                    class="truncate font-mono text-sm text-[var(--color-text-primary)]"
                                    >{{ option.label }}</span
                                >
                                <span
                                    v-if="option.detail"
                                    class="truncate text-xs text-[var(--color-text-secondary)]"
                                    >{{ option.detail }}</span
                                >
                                <span
                                    v-if="takenLabel && taken.has(option.id)"
                                    class="rounded bg-surface-5 px-1 text-[0.625rem] text-[var(--color-text-tertiary)]"
                                    >{{ takenLabel }}</span
                                >
                            </span>
                            <span
                                v-if="option.hint"
                                class="line-clamp-2 text-xs leading-4 text-[var(--color-text-tertiary)]"
                                >{{ option.hint }}</span
                            >
                        </span>
                    </button>
                </section>
            </div>
        </div>
    </NewModal>
</template>
