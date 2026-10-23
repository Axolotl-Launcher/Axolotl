<script setup lang="ts">
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import { computed } from 'vue'

import { usePreview } from '../lib/state'
import InstanceIcon from './InstanceIcon.vue'
import NavButton from './NavButton.vue'

/** Port of the app's QuickInstanceSwitcher: recent instances under the nav rail. */
const { instances, settings } = usePreview()

const recent = computed(() => {
    const sorted = [...instances.instances.value].sort((a, b) => {
        const dateA = Math.max(Date.parse(a.created), Date.parse(a.last_played ?? a.created))
        const dateB = Math.max(Date.parse(b.created), Date.parse(b.last_played ?? b.created))
        if (dateA === dateB) return a.name.localeCompare(b.name)
        return dateB - dateA
    })
    const count = settings.sidebar_instance_count
    return count > 0 ? sorted.slice(0, count) : sorted
})
</script>

<template>
    <div v-for="instance in recent" :key="instance.id" v-tooltip.right="instance.name">
        <NavButton :to="`/instance/${encodeURIComponent(instance.id)}`" class="relative">
            <InstanceIcon
                :icon-path="instance.icon_path"
                :instance-id="instance.id"
                :loader="instance.loader"
                size="28px"
                :class="`transition-all ${instance.install_stage !== 'installed' ? 'scale-[0.85] brightness-[0.25]' : 'group-hover:brightness-75'}`"
            />
            <div
                v-if="instance.install_stage !== 'installed'"
                class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center"
            >
                <SpinnerIcon class="size-4 animate-spin" />
            </div>
        </NavButton>
    </div>
    <div v-if="recent.length > 0" class="mx-auto my-2 h-px w-6 bg-divider"></div>
</template>
