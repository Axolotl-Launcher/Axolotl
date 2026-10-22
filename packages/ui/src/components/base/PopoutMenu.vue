<template>
    <component :is="menu ? DropdownMenuRoot : PopoverRoot" v-model:open="open" :modal="false">
        <component
            :is="menu ? DropdownMenuTrigger : PopoverTrigger"
            as-child
            @keydown="noteTriggerKeydown"
            @pointerdown="noteTriggerPointerdown"
            @mouseenter="enterHover"
            @mouseleave="leaveHover"
            @click.stop
            @pointerdown.stop
            @mousedown.stop
        >
            <slot name="trigger">
                <button v-tooltip="tooltip" v-bind="$attrs">
                    <slot></slot>
                </button>
            </slot>
        </component>

        <component :is="menu ? DropdownMenuPortal : PopoverPortal" :to="portalTarget">
            <component
                :is="menu ? DropdownMenuContent : PopoverContent"
                :id="dropdownId || undefined"
                :side="side"
                :align="align"
                :side-offset="sideOffset"
                :class="[dropdownClass, 'menu-surface']"
                v-bind="contentAttrs"
                :collision-padding="8"
                @mouseenter="enterHover"
                @mouseleave="leaveHover"
                @open-auto-focus="preventPointerFocus"
                @close-auto-focus="preventReplacedFocus"
                @keydown="dismissOnEscape"
                @escape-key-down="dismissOnEscape"
            >
                <slot name="menu" :hide="hide"></slot>
                <component
                    :is="menu ? DropdownMenuArrow : PopoverArrow"
                    class="menu-arrow"
                    :width="14"
                    :height="7"
                />
            </component>
        </component>
    </component>
</template>

<script setup lang="ts">
// Imported here, not only by the tooltip directive: `.menu-surface` styles this
// component's content element, so relying on the directive's import would leave
// the menu unstyled whenever no tooltip had rendered first.
import '../../styles/overlays.css'

import {
    DropdownMenuArrow,
    DropdownMenuContent,
    DropdownMenuPortal,
    DropdownMenuRoot,
    DropdownMenuTrigger,
    PopoverArrow,
    PopoverContent,
    PopoverPortal,
    PopoverRoot,
    PopoverTrigger,
} from 'reka-ui'
import { computed, onMounted, onScopeDispose, useAttrs, watch } from 'vue'

const props = withDefaults(
    defineProps<{
        dropdownId?: string
        dropdownClass?: string
        sideOffset?: number
        tooltip?: string
        placement?: string
        container?: string | HTMLElement | boolean
        /** Registered menu items use DropdownMenu; arbitrary panels use Popover. */
        menu?: boolean
        hoverable?: boolean
        contentAttrs?: Record<string, unknown>
    }>(),
    {
        dropdownId: undefined,
        dropdownClass: undefined,
        sideOffset: 4,
        tooltip: undefined,
        placement: 'bottom-end',
        container: undefined,
        menu: false,
    },
)

defineOptions({
    inheritAttrs: false,
})

const open = defineModel<boolean>('open', { default: false })
const attrs = useAttrs()
let hoverTimer: ReturnType<typeof setTimeout> | undefined
function enterHover() {
    clearTimeout(hoverTimer)
    if (props.hoverable && !attrs.disabled) open.value = true
}
function leaveHover() {
    if (props.hoverable)
        hoverTimer = setTimeout(() => {
            open.value = false
        }, 250)
}
const closeAll = 'close-all-context-menus'
let replaced = false
function preventReplacedFocus(event: Event) {
    if (replaced) event.preventDefault()
}
function closeOtherMenus() {
    if (props.menu && open.value) {
        replaced = true
        open.value = false
    }
}
onMounted(() => window.addEventListener(closeAll, closeOtherMenus))
watch(open, (value) => {
    if (value && props.menu) {
        replaced = false
        window.removeEventListener(closeAll, closeOtherMenus)
        window.dispatchEvent(new CustomEvent(closeAll))
        window.addEventListener(closeAll, closeOtherMenus)
    }
})
onScopeDispose(() => {
    clearTimeout(hoverTimer)
    window.removeEventListener(closeAll, closeOtherMenus)
})

// A pointer open must not move focus: clicking a menu would otherwise pull the
// caret out of whatever the user was editing and drop a focus ring on the first
// item. Only a keyboard open needs focus moved into the menu, which is what
// makes the items reachable with the arrow keys.
let openedFromKeyboard = false

function noteTriggerKeydown(event: KeyboardEvent) {
    openedFromKeyboard = ['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)
    if (!props.menu && ['ArrowDown', 'ArrowUp'].includes(event.key)) {
        event.preventDefault()
        open.value = true
    }
}

function noteTriggerPointerdown() {
    openedFromKeyboard = false
}

function preventPointerFocus(event: Event) {
    if (!openedFromKeyboard) {
        // Leave focus where the user left it; reka would otherwise focus the
        // content itself on a pointer open.
        event.preventDefault()
    }
}

/**
 * Where the menu is portalled to.
 *
 * The app and the website both mount a `#teleports` host; a component mounted
 * on its own (a test, or a future embed) may not, so this falls back to `body`
 * rather than to a selector that matches nothing -- a menu teleported to a
 * missing target renders nowhere at all, which is silent and confusing.
 */
const portalTarget = computed(() => {
    if (typeof document === 'undefined') return 'body'
    const container = props.container
    if (container instanceof HTMLElement) return container
    if (typeof container === 'string' && container !== 'body') {
        return document.querySelector(container) ? container : 'body'
    }
    return document.getElementById('teleports') ? '#teleports' : 'body'
})

/** `bottom-end` and friends are floating-ui placement names. */
const side = computed(() => {
    const [side] = props.placement.split('-')
    return side as 'top' | 'right' | 'bottom' | 'left'
})

const align = computed(() => {
    const [, align] = props.placement.split('-')
    if (align === 'start') return 'start'
    if (align === 'end') return 'end'
    return 'center'
})

function show() {
    open.value = true
}

function hide() {
    open.value = false
}
function dismissOnEscape(event: KeyboardEvent) {
    if (event.key !== 'Escape' || event.defaultPrevented) return
    event.preventDefault()
    event.stopPropagation()
    hide()
}

defineExpose({ show, hide })
</script>
