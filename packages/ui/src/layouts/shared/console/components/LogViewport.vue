<template>
    <div
        ref="viewportRef"
        class="log-viewport font-mono"
        :class="{ 'log-viewport-wrap': wrap, 'overflow-x-hidden': wrap }"
        :style="{ fontSize: fontSize + 'px' }"
        @scroll="handleScroll"
    >
        <div v-if="lines.length === 0" class="flex items-center justify-center h-full">
            <EmptyState
                v-if="emptyStateType === 'instance'"
                :heading="formatMessage(consoleMessages.emptyInstanceTitle)"
                :description="formatMessage(consoleMessages.emptyInstanceDescription)"
            />
            <EmptyState
                v-else-if="emptyStateType === 'server'"
                :heading="formatMessage(consoleMessages.emptyServerTitle)"
                :description="formatMessage(consoleMessages.emptyServerDescription)"
            />
        </div>

        <div
            v-else
            class="log-viewport-spacer relative w-full min-w-max"
            :style="{ height: totalHeight + 'px' }"
        >
            <div
                class="absolute inset-x-0 top-0"
                :style="{ transform: 'translateY(' + topOffset + 'px)' }"
            >
                <div
                    v-for="item in windowItems"
                    :key="item.originalIndex"
                    :data-line="item.originalIndex + 1"
                    class="log-line flex items-stretch whitespace-pre"
                    :class="entryClass(item.line)"
                    :style="{ height: estimateHeight(item) + 'px' }"
                >
                    <span
                        class="flex shrink-0 w-[52px] items-center justify-end leading-none text-right text-[var(--color-text-tertiary)] bg-surface-3 border-r border-solid border-surface-3 select-none overflow-hidden"
                        >{{ item.originalIndex + 1 }}</span
                    >
                    <span
                        class="log-line-content flex-1 px-2"
                        :class="wrap ? 'break-all [overflow-wrap:anywhere]' : ''"
                        v-html="renderLine(item)"
                    ></span>
                </div>
            </div>
        </div>

        <Transition name="scroll-to-bottom-fade">
            <div v-if="lines.length > 0 && !stickToBottom" class="absolute bottom-4 right-4 z-10">
                <Button
                    type="highlight"
                    size="xl"
                    circular
                    icon-only
                    :aria-label="formatMessage(consoleMessages.scrollToBottom)"
                    @click="scrollToBottom"
                    ><ChevronDownIcon />
                </Button>
            </div>
        </Transition>
    </div>
</template>

<script setup lang="ts">
// 日志查看器：交互与布局参考 LogShare-Web-UI (src/views/LogView.vue)，
// 逐行正则高亮来自 ./composables/log-highlight.ts（移植自 logParser.worker.ts）。
// LogShare-Web-UI 为 MIT License, Copyright (c) 2024 LogShare.CN Team，详见 packages/ui/COPYING.md。
import { ChevronDownIcon } from '@modrinth/assets'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import Button from '#ui/components/base/buttons/Button.vue'
import EmptyState from '#ui/components/base/EmptyState.vue'
import { useVIntl } from '#ui/composables/i18n'

import { highlightLine, markSearchMatches } from '../composables/log-highlight'
import { consoleMessages } from '../messages'
import type { LogLine } from '../types'

const { formatMessage } = useVIntl()

interface ViewportLine {
    line: LogLine
    originalIndex: number
}

const props = withDefaults(
    defineProps<{
        lines: ViewportLine[]
        searchQuery?: string
        wrap?: boolean
        fontSize?: number
        emptyStateType?: 'server' | 'instance'
    }>(),
    {
        searchQuery: '',
        wrap: false,
        fontSize: 12,
        emptyStateType: undefined,
    },
)

const viewportRef = ref<HTMLElement | null>(null)
const scrollTop = ref(0)
const viewportHeight = ref(0)
const stickToBottom = ref(true)

interface ViewportState {
    scrollTop: number
    stickToBottom: boolean
}

// 行高：单行 = 字号 × 1.4（与等宽字体匹配），wrap 时按估算折行数放大
const lineHeightPx = computed(() => Math.round(props.fontSize * 1.4))
// wrap 折行估算：0.6em 为等宽字符平均宽，乘 0.9 留保守余量（行高宁高勿矮，避免内容溢出重叠）
const charsPerLine = computed(() => {
    const vp = viewportRef.value
    if (!vp) return 120
    return Math.max(20, Math.floor((vp.clientWidth / (props.fontSize * 0.6)) * 0.9))
})

function estimateHeight(item: ViewportLine): number {
    if (!props.wrap) return lineHeightPx.value
    const lines = Math.max(1, Math.ceil(item.line.text.length / charsPerLine.value))
    return lines * lineHeightPx.value
}

// 高度前缀和缓存：lines/wrap/fontSize 变化时重建（O(n)），滚动时二分查找（O(log n)）
// 总高度必须是响应式的：普通变量 + 无依赖 computed 会缓存过期值，
// 清空控制台后模板不再读取它，重启后 spacer 会以旧高度渲染（底部空白）。
let heightPrefix: number[] | null = null
const heightTotal = ref(0)

function rebuildHeights() {
    const n = props.lines.length
    if (!props.wrap) {
        heightPrefix = null
        heightTotal.value = n * lineHeightPx.value
        return
    }
    const prefix = new Array<number>(n)
    let acc = 0
    for (let i = 0; i < n; i++) {
        prefix[i] = acc
        acc += estimateHeight(props.lines[i]!)
    }
    heightPrefix = prefix
    heightTotal.value = acc
}

watch(
    () => [props.lines, props.wrap, props.fontSize] as const,
    ([lines], previous) => {
        rebuildHeights()
        // A fresh stream after an empty console (clear, restart, initial
        // hydration) always resumes bottom-following.
        if (previous && previous[0].length === 0 && lines.length > 0) {
            stickToBottom.value = true
        }
        if (lines.length === 0) {
            // Reset the virtual window state along with the DOM scroll position;
            // browsers may clamp silently without firing a scroll event.
            scrollTop.value = 0
            if (viewportRef.value) viewportRef.value.scrollTop = 0
        }
        if (stickToBottom.value) {
            nextTick(scrollToBottom)
        }
    },
    { immediate: true },
)

const totalHeight = computed(() => heightTotal.value)

// 虚拟窗口：可见行 + 上下缓冲
const WINDOW_BUFFER = 15

function computeWindow(): { items: ViewportLine[]; startIndex: number } {
    const n = props.lines.length
    if (n === 0) return { items: [], startIndex: 0 }

    let start = 0
    let end = n - 1

    if (n > WINDOW_BUFFER * 2) {
        if (props.wrap && heightPrefix) {
            let lo = 0
            let hi = n - 1
            while (lo < hi) {
                const mid = (lo + hi + 1) >> 1
                if (heightPrefix[mid]! <= scrollTop.value) lo = mid
                else hi = mid - 1
            }
            start = Math.max(0, lo - WINDOW_BUFFER)
        } else {
            const first = Math.floor(scrollTop.value / lineHeightPx.value)
            start = Math.max(0, first - WINDOW_BUFFER)
        }
        end = Math.min(
            n - 1,
            start + Math.ceil(viewportHeight.value / lineHeightPx.value) + WINDOW_BUFFER * 2,
        )
    }

    return { items: props.lines.slice(start, end + 1), startIndex: start }
}

const windowState = computed(computeWindow)
const windowItems = computed(() => windowState.value.items)

const topOffset = computed(() => {
    const { startIndex } = windowState.value
    if (startIndex === 0) return 0
    if (props.wrap && heightPrefix) return heightPrefix[startIndex]!
    return startIndex * lineHeightPx.value
})

function entryClass(line: LogLine): string {
    if (line.level === 'error') return 'entry-error'
    if (line.level === 'warn') return 'entry-warning'
    return 'entry-no-error'
}

function renderLine(item: ViewportLine): string {
    return markSearchMatches(highlightLine(item.line.text), props.searchQuery)
}

function handleScroll() {
    const vp = viewportRef.value
    if (!vp) return
    scrollTop.value = vp.scrollTop
    viewportHeight.value = vp.clientHeight
    stickToBottom.value = vp.scrollTop + vp.clientHeight >= vp.scrollHeight - lineHeightPx.value * 2
}

function scrollToBottom() {
    const vp = viewportRef.value
    if (!vp) return
    vp.scrollTop = vp.scrollHeight
    scrollTop.value = vp.scrollTop
    stickToBottom.value = true
}

function syncViewportSize() {
    const vp = viewportRef.value
    if (!vp) return
    viewportHeight.value = vp.clientHeight
    // 窗口宽度影响 wrap 折行估算，resize 时重建高度缓存
    if (props.wrap) rebuildHeights()
}

function captureViewState(): ViewportState {
    return {
        scrollTop: viewportRef.value?.scrollTop ?? scrollTop.value,
        stickToBottom: stickToBottom.value,
    }
}

function restoreViewState(state: ViewportState) {
    const vp = viewportRef.value
    if (!vp) return
    syncViewportSize()
    if (state.stickToBottom) {
        scrollToBottom()
        return
    }
    vp.scrollTop = Math.min(state.scrollTop, Math.max(0, vp.scrollHeight - vp.clientHeight))
    scrollTop.value = vp.scrollTop
    stickToBottom.value = false
}

let resizeObserver: ResizeObserver | null = null

onMounted(() => {
    syncViewportSize()
    if (stickToBottom.value) nextTick(scrollToBottom)
    resizeObserver = new ResizeObserver(syncViewportSize)
    if (viewportRef.value) resizeObserver.observe(viewportRef.value)
    window.addEventListener('resize', syncViewportSize)
})

onBeforeUnmount(() => {
    resizeObserver?.disconnect()
    resizeObserver = null
    window.removeEventListener('resize', syncViewportSize)
})

defineExpose({
    captureViewState,
    restoreViewState,
    scrollToBottom,
})
</script>

<style>
.log-viewport {
    height: 100%;
    overflow-y: auto;
    overflow-x: auto;
    background-color: var(--surface-2);
    color: var(--color-text-default);
    line-height: 1.4;
    -webkit-font-smoothing: antialiased;
    text-rendering: optimizeLegibility;
    user-select: text;
}

.scroll-to-bottom-fade-enter-active,
.scroll-to-bottom-fade-leave-active {
    transition: opacity 250ms ease-in-out;
}

.scroll-to-bottom-fade-enter-from,
.scroll-to-bottom-fade-leave-to {
    opacity: 0;
}

.log-viewport-wrap .log-viewport-spacer {
    min-width: 0;
}

.log-viewport-wrap .log-line {
    white-space: normal;
}

.log-viewport-wrap .log-line-content {
    min-width: 0;
}

/* The `-bg` tints are rungs of the opacity model and already carry the theme
   (light 10%, dark/oled 20%), so one rule each replaces the old fixed 12%/18%
   pair -- the `[data-theme='dark']` overrides duplicated a distinction the token
   now makes on its own. */
.log-line.entry-error {
    background-color: var(--color-red-bg);
}

.log-line.entry-warning {
    background-color: var(--color-orange-bg);
}

.log-line mark {
    padding: 0 0.1em;
    background-color: color-mix(
        in srgb,
        var(--color-blue) var(--opacity-ratio-keep-45),
        transparent
    );
    color: var(--color-text-primary);
    border-radius: 2px;
    font-weight: 500;
}

/* ===== LogShare token 高亮（LogsAnalysis.css 移植，前景色用主题变量） ===== */

.level {
    white-space: pre;
    word-break: normal;
    overflow-wrap: normal;
}

.log-viewport-wrap .level {
    white-space: pre-wrap;
    word-break: break-all;
    overflow-wrap: anywhere;
}

.level-error {
    color: var(--color-red);
    font-weight: 600;
}

.level-warning {
    color: var(--color-orange);
}

.level-debug {
    color: var(--color-text-tertiary);
    background-color: color-mix(
        in srgb,
        var(--color-blue) var(--opacity-ratio-keep-5),
        transparent
    );
}

.level-timestamp {
    color: var(--color-blue);
    font-weight: 500;
}

.level-info-prefix {
    color: var(--color-green);
    font-weight: 500;
}

.level-thread {
    color: var(--color-blue);
    opacity: 0.85;
}

.level-error-word {
    color: var(--color-red);
    font-weight: 600;
}

.level-warning-tag {
    color: var(--color-orange);
    font-weight: 600;
}

.level-plugin {
    color: var(--color-green);
    opacity: 0.85;
}

.level-filepath {
    color: var(--color-blue);
    opacity: 0.85;
}

.level-dimmed {
    color: var(--color-text-tertiary);
    opacity: 0.75;
}

.level-stack-frame {
    color: var(--color-text-tertiary);
}

.level-stack-class {
    color: var(--color-orange);
    font-weight: 500;
}

.level-stack-location {
    color: var(--color-blue);
}

.level-stack-caused-by {
    color: var(--color-red);
    font-weight: 600;
}

.level-stack-exception {
    color: var(--color-red);
    font-weight: 600;
}

.level-mod-header {
    color: var(--color-purple);
    font-weight: 700;
}

.level-mod-id {
    color: var(--color-blue);
    font-weight: 500;
}

.level-mod-version {
    color: #d4a72c;
    font-weight: 500;
}

.level-mod-name {
    color: var(--color-text-tertiary);
}

.level-mod-status {
    color: var(--color-text-tertiary);
}

.level-mod-status-ok {
    color: var(--color-green);
    font-weight: 600;
}

.level-mod-status-error {
    color: var(--color-red);
    font-weight: 600;
}

.level-mod-status-warn {
    color: var(--color-orange);
    font-weight: 600;
}

.level-mod-tree {
    color: var(--color-blue);
}

.level-mod-dim {
    color: var(--color-green);
}

.level-env-key {
    color: var(--color-blue);
    font-weight: 500;
}

.level-section-header {
    color: var(--color-purple);
    font-weight: 600;
}

.level-arg-flag {
    color: #d4a72c;
    font-weight: 500;
}

.level-section-marker {
    color: var(--color-orange);
    font-weight: 600;
}

/* Minecraft § 颜色码 */

.format-black {
    color: #000000;
}

.format-darkblue {
    color: #0000aa;
}

.format-darkgreen {
    color: #00aa00;
}

.format-darkaqua {
    color: #00aaaa;
}

.format-darkred {
    color: #aa0000;
}

.format-darkpurple {
    color: #aa00aa;
}

.format-gold {
    color: #ffaa00;
}

.format-gray {
    color: #aaaaaa;
}

.format-darkgray {
    color: #555555;
}

.format-blue {
    color: #5555ff;
}

.format-green {
    color: #55ff55;
}

.format-aqua {
    color: #55ffff;
}

.format-red {
    color: #ff5555;
}

.format-lightpurple {
    color: #ff55ff;
}

.format-yellow {
    color: #ffff55;
}

.format-white {
    color: #ffffff;
}

.format-reset {
    color: var(--color-text-default);
    font-weight: normal;
    text-decoration: none;
    font-style: normal;
}

.format-bold {
    font-weight: bold;
}

.format-underline {
    text-decoration: underline;
}

.format-italic {
    font-style: italic;
}

.format-strike {
    text-decoration: line-through;
}
</style>
