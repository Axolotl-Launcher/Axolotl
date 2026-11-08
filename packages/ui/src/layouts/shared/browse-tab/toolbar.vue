<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import { ref } from 'vue'

import NavTabs from '#ui/components/base/NavTabs.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import { useStickyObserver } from '#ui/composables/sticky-observer'

withDefaults(
    defineProps<{
        variant?: 'app' | 'web'
        tabs: InstanceType<typeof NavTabs>['$props']['links']
        showTabs?: boolean
        placeholder: string
        disabled?: boolean
    }>(),
    { variant: 'app', showTabs: true, disabled: false },
)
const query = defineModel<string>('query', { required: true })
const emit = defineEmits<{ clear: [] }>()
const stickyChromeRef = ref<HTMLElement | null>(null)
const { isStuck: isChromeStuck } = useStickyObserver(stickyChromeRef, 'BrowseToolbarChrome')
</script>

<template>
    <div
        ref="stickyChromeRef"
        class="browse-toolbar-chrome sticky top-0 z-20 flex flex-col gap-3 border-b border-solid border-surface-5"
        :class="[
            isChromeStuck && variant !== 'web'
                ? 'border-t shadow-[0_8px_16px_-12px_rgba(0,0,0,0.35)]'
                : '',
            variant === 'web'
                ? 'mb-3 rounded-xl bg-surface-1 p-3'
                : '-mx-6 -mt-6 border-0 bg-[color-mix(in_srgb,var(--surface-1-opaque,var(--surface-1))_20%,transparent)] backdrop-blur-xl',
        ]"
    >
        <div v-if="$slots['install-header']" class="p-3 pb-0">
            <slot name="install-header" />
        </div>
        <div class="flex flex-col gap-3" :class="variant === 'web' ? '' : 'p-3'">
            <div class="flex min-w-0 flex-wrap items-center gap-2">
                <NavTabs v-if="showTabs" :links="tabs" route-sync class="min-w-0 max-w-full" />
                <div class="ml-auto shrink-0"><slot name="nav-tabs-actions" /></div>
            </div>
            <div class="flex min-w-0 flex-wrap items-center gap-2">
                <StyledInput
                    v-model="query"
                    :disabled="disabled"
                    :icon="SearchIcon"
                    type="text"
                    autocomplete="off"
                    :placeholder="placeholder"
                    clearable
                    wrapper-class="min-w-0 flex-[1_1_16rem]"
                    :input-class="variant === 'web' ? '!h-12' : 'h-12'"
                    @clear="emit('clear')"
                />
                <slot name="search-bar-actions" />
            </div>
            <div v-if="$slots.controls" class="flex min-h-10 flex-wrap items-center gap-2">
                <slot name="controls" />
            </div>
            <slot name="filters" />
        </div>
    </div>
</template>
