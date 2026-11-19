<script setup>
import { SpinnerIcon } from '@modrinth/assets'
import { injectNotificationManager } from '@modrinth/ui'
import dayjs from 'dayjs'
import { computed } from 'vue'

import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import NavButton from '@/components/ui/NavButton.vue'
import { useInstanceList } from '@/composables/instances/use-instance-list'
import { useTheming } from '@/store/state'

const { handleError } = injectNotificationManager()

const themeStore = useTheming()

const { data: instances, load } = useInstanceList(handleError)
const instanceCount = computed(() => themeStore.sidebarInstanceCount)

const recentInstances = computed(() => {
    const sorted = [...(instances.value ?? [])].sort((a, b) => {
        const dateACreated = dayjs(a.created)
        const dateAPlayed = a.last_played ? dayjs(a.last_played) : dayjs(0)

        const dateBCreated = dayjs(b.created)
        const dateBPlayed = b.last_played ? dayjs(b.last_played) : dayjs(0)

        const dateA = dateACreated.isAfter(dateAPlayed) ? dateACreated : dateAPlayed
        const dateB = dateBCreated.isAfter(dateBPlayed) ? dateBCreated : dateBPlayed

        if (dateA.isSame(dateB)) {
            return a.name.localeCompare(b.name)
        }

        return dateB - dateA
    })
    return instanceCount.value > 0 ? sorted.slice(0, instanceCount.value) : sorted
})

if (!instances.value) await load().catch(() => {})
</script>

<template>
    <div v-for="instance in recentInstances" :key="instance.id" v-tooltip.right="instance.name">
        <NavButton :to="`/instance/${encodeURIComponent(instance.id)}`" class="relative">
            <InstanceIcon
                :icon-path="instance.icon_path"
                :instance-id="instance.id"
                :loader="instance.loader"
                size="28px"
                :class="`transition-all ${instance.install_stage !== 'installed' ? `brightness-[0.25] scale-[0.85]` : `group-hover:brightness-75`}`"
            />
            <div
                v-if="instance.install_stage !== 'installed'"
                class="absolute inset-0 flex items-center justify-center z-10 pointer-events-none"
            >
                <SpinnerIcon class="animate-spin w-4 h-4" />
            </div>
        </NavButton>
    </div>
    <div v-if="recentInstances.length > 0" class="h-px w-6 mx-auto my-2 bg-divider"></div>
</template>

<style scoped lang="scss"></style>
