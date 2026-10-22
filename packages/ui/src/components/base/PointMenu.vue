<script setup lang="ts">
import '../../styles/overlays.css'

import {
    DropdownMenuContent,
    DropdownMenuPortal,
    DropdownMenuRoot,
    DropdownMenuTrigger,
} from 'reka-ui'
import { nextTick, onMounted, onScopeDispose, ref, watch } from 'vue'

import type { MenuOption } from './menu-options'
import MenuItems from './MenuItems.vue'

defineProps<{ options?: MenuOption[]; label?: string }>()
const emit = defineEmits<{ close: [] }>()
const open = ref(false)
const point = ref({ x: 0, y: 0 })
const boundary = ref<HTMLElement>()
const body = ref<HTMLElement>()
let previousFocus: HTMLElement | undefined
let disposed = false
let restorePreviousFocus = true
let originObserver: MutationObserver | undefined
const closeAll = 'close-all-context-menus'
function close() {
    open.value = false
    originObserver?.disconnect()
}
function dismissOnEscape(event: KeyboardEvent) {
    if (event.key !== 'Escape' || event.defaultPrevented) return
    event.preventDefault()
    event.stopPropagation()
    close()
}
function show(event: Pick<MouseEvent, 'clientX' | 'clientY' | 'target'>) {
    if (disposed) return
    window.dispatchEvent(new CustomEvent(closeAll))
    previousFocus =
        document.activeElement instanceof HTMLElement ? document.activeElement : undefined
    restorePreviousFocus = true
    boundary.value =
        event.target instanceof Element
            ? (event.target.closest<HTMLElement>('.app-viewport') ?? undefined)
            : undefined
    const rect = boundary.value?.getBoundingClientRect() ?? {
        left: 0,
        top: 0,
        right: window.innerWidth,
        bottom: window.innerHeight,
    }
    point.value = {
        x: Math.max(rect.left + 10, Math.min(event.clientX, rect.right - 10)),
        y: Math.max(rect.top + 10, Math.min(event.clientY, rect.bottom - 10)),
    }
    open.value = true
    originObserver?.disconnect()
    const origin = event.target instanceof Element ? event.target : null
    if (origin?.isConnected) {
        originObserver = new MutationObserver(() => {
            if (!origin.isConnected) close()
        })
        originObserver.observe(document.body, { childList: true, subtree: true })
    }
}
function restoreFocus(event: Event) {
    event.preventDefault()
    if (restorePreviousFocus && previousFocus?.isConnected)
        previousFocus.focus({ preventScroll: true })
}
function focusMenu(event?: Event) {
    event?.preventDefault()
    void nextTick(() => {
        if (!open.value || disposed) return
        const target =
            body.value?.querySelector<HTMLElement>(
                '[role="menuitem"]:not([data-disabled]):not(:disabled)',
            ) ?? body.value?.closest<HTMLElement>('[role="menu"]')
        target?.focus({ preventScroll: true })
    })
}
function closeForReplacement() {
    restorePreviousFocus = false
    close()
}
onMounted(() => window.addEventListener(closeAll, closeForReplacement))
watch(open, (value, old) => {
    if (!value) originObserver?.disconnect()
    if (!value && old) emit('close')
})
onScopeDispose(() => {
    disposed = true
    originObserver?.disconnect()
    window.removeEventListener(closeAll, closeForReplacement)
})
defineExpose({ show, close })
</script>

<template>
    <DropdownMenuRoot v-model:open="open" :modal="false">
        <DropdownMenuTrigger as-child>
            <span
                aria-hidden="true"
                tabindex="-1"
                class="pointer-events-none fixed size-0"
                :style="{ left: `${point.x}px`, top: `${point.y}px` }"
            />
        </DropdownMenuTrigger>
        <DropdownMenuPortal to="body">
            <DropdownMenuContent
                v-if="open"
                side="bottom"
                align="start"
                :side-offset="0"
                :collision-padding="10"
                :collision-boundary="boundary"
                :aria-label="label"
                class="menu-surface box-border max-w-[calc(100vw-20px)] max-h-[var(--reka-dropdown-menu-content-available-height)] overflow-y-auto"
                @close-auto-focus="restoreFocus"
                @interact-outside="restorePreviousFocus = false"
                @open-auto-focus="focusMenu"
                @keydown="dismissOnEscape"
                @escape-key-down="dismissOnEscape"
            >
                <div ref="body" role="none">
                    <slot>
                        <MenuItems :options="options ?? []">
                            <template v-for="(_, name) in $slots" #[name]
                                ><slot :name="name"
                            /></template>
                        </MenuItems>
                    </slot>
                </div>
            </DropdownMenuContent>
        </DropdownMenuPortal>
    </DropdownMenuRoot>
</template>
