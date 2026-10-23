<template>
    <RouterLink
        v-if="typeof to === 'string' && !disabled"
        :to="to"
        v-bind="$attrs"
        active-class="nav-router-active"
        :class="activeClasses"
        class="nav-button flex size-12 items-center justify-center rounded-full bg-transparent text-2xl text-[var(--color-text-default)] transition-all hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
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
        class="nav-button flex size-12 items-center justify-center rounded-full bg-transparent text-2xl text-[var(--color-text-default)] transition-all hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
        @click.prevent
    >
        <slot />
    </button>
    <button
        v-else
        v-bind="$attrs"
        type="button"
        class="nav-button button-animation flex size-12 cursor-pointer items-center justify-center rounded-full border-none bg-transparent text-2xl text-[var(--color-text-default)] transition-all hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
        :disabled="disabled"
        @click="to"
    >
        <slot />
    </button>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { type RouteLocationNormalizedLoaded, RouterLink, useLink, useRoute } from 'vue-router'

/** Port of the app's NavButton, highlighting itself for its primary route and subpages. */
type RouteFunction = (route: RouteLocationNormalizedLoaded) => boolean

const props = withDefaults(
    defineProps<{
        to: (() => void) | string
        isPrimary?: RouteFunction
        isSubpage?: RouteFunction
        disabled?: boolean
    }>(),
    {
        disabled: false,
        isPrimary: undefined,
        isSubpage: undefined,
    },
)

const route = useRoute()
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
.router-link-active {
    @apply bg-[--color-button-bg-selected] text-[--color-button-text-selected];
}

.subpage-active {
    @apply bg-surface-4 text-[var(--color-text-primary)];
}
</style>
