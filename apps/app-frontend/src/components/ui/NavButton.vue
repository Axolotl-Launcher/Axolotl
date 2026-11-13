<template>
    <RouterLink
        v-if="typeof to === 'string' && !disabled"
        ref="root"
        :to="to"
        v-bind="$attrs"
        active-class="nav-router-active"
        :class="activeClasses"
        class="w-12 h-12 text-[var(--color-text-default)] rounded-full flex items-center justify-center text-2xl transition-all bg-transparent hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
        @pointerenter="onPointerEnter"
        @pointerleave="onPointerLeave"
        @click="onActivate"
    >
        <slot />
    </RouterLink>
    <button
        v-else-if="typeof to === 'string'"
        v-bind="$attrs"
        type="button"
        aria-disabled="true"
        tabindex="-1"
        :class="activeClasses"
        class="w-12 h-12 text-[var(--color-text-default)] rounded-full flex items-center justify-center text-2xl transition-all bg-transparent hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
        @click.prevent
        @keydown.enter.prevent
        @keyup.enter.prevent
        @keydown.space.prevent
        @keyup.space.prevent
    >
        <slot />
    </button>
    <button
        v-else
        ref="root"
        v-bind="$attrs"
        class="button-animation border-none text-[var(--color-text-default)] cursor-pointer w-12 h-12 rounded-full flex items-center justify-center text-2xl transition-all bg-transparent hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
        :disabled="disabled"
        @click="onClick"
        @pointerenter="onPointerEnter"
        @pointerleave="onPointerLeave"
    >
        <slot />
    </button>
</template>

<script setup lang="ts">
import { type ComponentPublicInstance, computed, useTemplateRef, watch, watchPostEffect } from 'vue'
import type { RouteLocationNormalizedLoaded } from 'vue-router'
import { RouterLink, useLink, useRoute, useRouter } from 'vue-router'

import { provideNavIconInteraction } from './nav-icons/interaction'
import { injectNavRailOptimisticSelection } from './nav-rail-selection'

const route = useRoute()
const router = useRouter()
const optimisticSelection = injectNavRailOptimisticSelection()

type RouteFunction = (route: RouteLocationNormalizedLoaded) => boolean

const props = withDefaults(
    defineProps<{
        to: (() => void) | string
        isPrimary?: RouteFunction
        isSubpage?: RouteFunction
        highlightOverride?: boolean
        disabled?: boolean
    }>(),
    {
        disabled: false,
        isPrimary: undefined,
        isSubpage: undefined,
    },
)

const link = useLink({ to: computed(() => (typeof props.to === 'string' ? props.to : '/')) })
const optimisticallyActive = computed(() => {
    const href = optimisticSelection?.href.value
    if (!href || typeof props.to !== 'string') return false
    return router.resolve(props.to).path === href
})
const activeClasses = computed(() => {
    const primary = props.isPrimary ? props.isPrimary(route) : link.isActive.value
    return {
        'router-link-active': primary || optimisticallyActive.value,
        'subpage-active': !primary && !optimisticallyActive.value && !!props.isSubpage?.(route),
    }
})

// The icon in the slot animates from these; a disabled destination stays still.
const { hovered, presses, activations } = provideNavIconInteraction()

watch(
    () => props.disabled,
    (disabled) => {
        if (disabled) hovered.value = false
    },
)

// Presses are heard through a native listener, not `@pointerdown`. Vue skips a
// listener attached in or after the millisecond in which an event first reached
// a Vue listener, so a `@pointerdown` here would hide the press from `NavRail`
// whenever the rail mounted in that same millisecond.
const root = useTemplateRef<HTMLElement | ComponentPublicInstance>('root')
watchPostEffect((onCleanup) => {
    const element = root.value instanceof HTMLElement ? root.value : root.value?.$el
    if (!(element instanceof HTMLElement)) return
    element.addEventListener('pointerdown', onPointerDown)
    onCleanup(() => element.removeEventListener('pointerdown', onPointerDown))
})

function onPointerEnter() {
    if (!props.disabled) hovered.value = true
}

function onPointerLeave() {
    hovered.value = false
}

function onPointerDown(event: PointerEvent) {
    if (!props.disabled && event.button === 0) presses.value++
}

function onActivate() {
    if (!props.disabled) activations.value++
}

function onClick() {
    onActivate()
    if (typeof props.to === 'function') props.to()
}

defineOptions({
    inheritAttrs: false,
})
</script>

<style lang="scss" scoped>
.router-link-active,
.subpage-active {
    svg {
        filter: drop-shadow(0 0 0.5rem black);
    }
}

.router-link-active {
    @apply text-[--color-button-text-selected] bg-[--color-button-bg-selected];
}

.subpage-active {
    @apply text-[var(--color-text-primary)] bg-surface-4;
}
</style>
