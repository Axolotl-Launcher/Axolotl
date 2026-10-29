<script setup lang="ts">
import '../../styles/overlays.css'

import { PopoverArrow, PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from 'reka-ui'
import { computed } from 'vue'

const props = defineProps<{
    count: number
    wrapperClass?: string
}>()

defineOptions({
    inheritAttrs: false,
})

const portalTarget = computed(() =>
    typeof document !== 'undefined' && document.getElementById('teleports') ? '#teleports' : 'body',
)
</script>

<template>
    <PopoverRoot :modal="false">
        <PopoverTrigger as-child>
            <!--
                A real button, not the div this started as: the trigger has to be
                reachable and openable from the keyboard, and a div is neither
                focusable nor activatable. The class carries the tag styling so the
                trigger keeps looking like the `+N` tag it replaces.
            -->
            <button
                v-bind="$attrs"
                type="button"
                class="inline-flex cursor-pointer rounded-full border-0 bg-transparent p-0 text-inherit focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand"
                :class="props.wrapperClass"
            >
                <slot name="trigger">+{{ props.count }}</slot>
            </button>
        </PopoverTrigger>

        <PopoverPortal :to="portalTarget">
            <PopoverContent class="menu-surface" :side-offset="4">
                <div class="flex max-w-[20rem] flex-wrap gap-1">
                    <slot />
                </div>
                <PopoverArrow class="menu-arrow" :width="14" :height="7" />
            </PopoverContent>
        </PopoverPortal>
    </PopoverRoot>
</template>
