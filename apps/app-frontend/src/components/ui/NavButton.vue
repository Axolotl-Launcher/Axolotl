<template>
    <RouterLink
        v-if="typeof to === 'string' && !disabled"
        :to="to"
        v-bind="$attrs"
        active-class="nav-router-active"
        :class="activeClasses"
        class="w-12 h-12 text-[var(--color-text-default)] rounded-full flex items-center justify-center text-2xl transition-all bg-transparent hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
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
        v-bind="$attrs"
        class="button-animation border-none text-[var(--color-text-default)] cursor-pointer w-12 h-12 rounded-full flex items-center justify-center text-2xl transition-all bg-transparent hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
        :disabled="disabled"
        @click="to"
    >
        <slot />
    </button>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { RouteLocationNormalizedLoaded } from 'vue-router'
import { RouterLink, useLink, useRoute } from 'vue-router'

const route = useRoute()

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
const activeClasses = computed(() => {
    const primary = props.isPrimary ? props.isPrimary(route) : link.isActive.value
    return {
        'router-link-active': primary,
        'subpage-active': !primary && !!props.isSubpage?.(route),
    }
})

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
