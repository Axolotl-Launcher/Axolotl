<script setup lang="ts">
import '@modrinth/ui/src/styles/overlays.css'

import type {
    MenuAction,
    MenuColor,
    MenuOption,
} from '@modrinth/ui/src/components/base/menu-options.ts'
import MenuItems from '@modrinth/ui/src/components/base/MenuItems.vue'
import {
    DropdownMenuContent,
    DropdownMenuPortal,
    DropdownMenuRoot,
    DropdownMenuTrigger,
} from 'reka-ui'
import {
    type Component,
    computed,
    nextTick,
    onMounted,
    onScopeDispose,
    ref,
    shallowRef,
    watch,
} from 'vue'

import { injectPreviewWindow, WINDOW_HEIGHT, WINDOW_WIDTH } from '../lib/window'

/**
 * The app's right-click `ContextMenu` (a wrapper around the shared `PointMenu`),
 * opened at the pointer inside the launcher window and clamped to the page
 * viewport like the original.
 */
export type ContextOption = {
    section?: MenuAction['section']
    name?: string
    id?: string
    label?: string
    type?: string
    color?: string
    tone?: string
    shown?: boolean
    disabled?: boolean
    action?: () => void
    icon?: Component
}

defineProps<{ label?: string }>()
const emit = defineEmits<{
    'menu-closed': []
    'option-clicked': [payload: { item: unknown; option: string }]
}>()

const { layer, root, toLocal } = injectPreviewWindow()

const item = shallowRef<unknown>()
const options = shallowRef<ContextOption[]>([])
const entries = computed<MenuOption[]>(() =>
    options.value.flatMap((option): MenuOption[] => {
        if (option.type === 'divider') return [{ divider: true }]
        const id = option.name ?? option.id ?? ''
        return [
            {
                ...option,
                id,
                color: (option.color ??
                    (option.tone === 'red' ? 'danger' : undefined)) as MenuColor,
                action: () => {
                    if (option.action) option.action()
                    else emit('option-clicked', { item: item.value, option: id })
                },
            } as MenuOption,
        ]
    }),
)

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

function showAt(event: Pick<MouseEvent, 'clientX' | 'clientY' | 'target'>) {
    if (disposed) return
    window.dispatchEvent(new CustomEvent(closeAll))
    previousFocus =
        document.activeElement instanceof HTMLElement ? document.activeElement : undefined
    restorePreviousFocus = true
    boundary.value =
        event.target instanceof Element
            ? (event.target.closest<HTMLElement>('.app-viewport') ?? root.value ?? undefined)
            : (root.value ?? undefined)
    const local = toLocal(event.clientX, event.clientY)
    let bounds = { left: 0, top: 0, right: WINDOW_WIDTH, bottom: WINDOW_HEIGHT }
    if (boundary.value) {
        const rect = boundary.value.getBoundingClientRect()
        const topLeft = toLocal(rect.left, rect.top)
        const bottomRight = toLocal(rect.right, rect.bottom)
        bounds = { left: topLeft.x, top: topLeft.y, right: bottomRight.x, bottom: bottomRight.y }
    }
    point.value = {
        x: Math.max(bounds.left + 10, Math.min(local.x, bounds.right - 10)),
        y: Math.max(bounds.top + 10, Math.min(local.y, bounds.bottom - 10)),
    }
    open.value = true
    originObserver?.disconnect()
    const origin = event.target instanceof Element ? event.target : null
    if (origin?.isConnected) {
        originObserver = new MutationObserver(() => {
            if (!origin.isConnected) close()
        })
        originObserver.observe(root.value ?? document.body, { childList: true, subtree: true })
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
    if (!value && old) emit('menu-closed')
})
onScopeDispose(() => {
    disposed = true
    originObserver?.disconnect()
    window.removeEventListener(closeAll, closeForReplacement)
})

function showMenu(event: MouseEvent, target: unknown, actions: ContextOption[]) {
    item.value = target
    options.value = actions
    showAt(event)
}

defineExpose({
    showMenu,
    open: (event: MouseEvent, actions: ContextOption[]) => showMenu(event, null, actions),
    close,
})
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
        <DropdownMenuPortal :to="layer ?? 'body'">
            <DropdownMenuContent
                v-if="open"
                side="bottom"
                align="start"
                :side-offset="0"
                :collision-padding="10"
                :collision-boundary="boundary"
                :aria-label="label"
                class="menu-surface box-border max-h-[var(--reka-dropdown-menu-content-available-height)] max-w-[1260px] overflow-y-auto"
                @close-auto-focus="restoreFocus"
                @interact-outside="restorePreviousFocus = false"
                @open-auto-focus="focusMenu"
                @keydown="dismissOnEscape"
                @escape-key-down="dismissOnEscape"
            >
                <div ref="body" role="none">
                    <slot>
                        <MenuItems :options="entries">
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
