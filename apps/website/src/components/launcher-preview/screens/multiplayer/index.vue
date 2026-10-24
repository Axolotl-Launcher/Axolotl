<script setup lang="ts">
import './server-preview-messages.ts'

import ServerIcon from '@modrinth/assets/icons/server.svg?component'
import UsersIcon from '@modrinth/assets/icons/users.svg?component'
import NavTabs from '@modrinth/ui/src/components/base/NavTabs.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, defineAsyncComponent } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import ServersOverview from './servers/ServersOverview.vue'

const MultiplayerRooms = defineAsyncComponent(() => import('./components/MultiplayerRooms.vue'))
const ServerDetail = defineAsyncComponent(() => import('./servers/ServerDetail.vue'))
const ServerFileStudio = defineAsyncComponent(() => import('./servers/ServerFileStudio.vue'))

/**
 * Port of `pages/Multiplayer.vue`: the page title and the Servers/Rooms tabs
 * over the active view. The preview's route table mounts this file for
 * `/multiplayer/servers`, `/multiplayer/servers/:id`, its Studio and `/multiplayer/rooms`,
 * so the view is chosen from the route name instead of a child `RouterView`.
 */
const { formatMessage } = useVIntl()
const route = useRoute()
const router = useRouter()

const messages = defineMessages({
    title: { id: 'app.multiplayer.title', defaultMessage: 'Multiplayer' },
    serversTab: { id: 'app.multiplayer.tab.servers', defaultMessage: 'Servers' },
    roomsTab: { id: 'app.multiplayer.tab.rooms', defaultMessage: 'Rooms' },
})

const activeTab = computed(() =>
    route.path.startsWith('/multiplayer/rooms') ? 'rooms' : 'servers',
)
const isStudioMode = computed(() => route.name === 'MultiplayerServerFileStudio')
// The server detail page has a fixed height: its console scrolls inside.
const isFixedRender = computed(
    () => route.name === 'MultiplayerServerDetail' || route.name === 'MultiplayerServerFileStudio',
)
const isRooms = computed(() => route.name === 'MultiplayerRooms')

const tabLinks = computed(() => [
    { label: formatMessage(messages.serversTab), href: '/multiplayer/servers', icon: ServerIcon },
    { label: formatMessage(messages.roomsTab), href: '/multiplayer/rooms', icon: UsersIcon },
])

function handleTabClick(index: number) {
    void router.push(tabLinks.value[index]?.href ?? '/multiplayer/servers')
}
</script>

<template>
    <div
        :class="
            isStudioMode
                ? 'flex h-full min-h-0 w-full flex-col'
                : isFixedRender
                  ? 'box-border flex h-full min-h-0 w-full flex-col gap-3 p-6'
                  : 'box-border flex min-h-full w-full flex-col gap-3 p-6'
        "
    >
        <template v-if="!isStudioMode">
            <h1 class="m-0 shrink-0 text-2xl font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h1>
            <NavTabs
                mode="local"
                :active-index="activeTab === 'rooms' ? 1 : 0"
                :links="tabLinks"
                @tab-click="handleTabClick"
            />
        </template>
        <MultiplayerRooms v-if="isRooms" />
        <ServerFileStudio v-else-if="isStudioMode" :key="`studio-${String(route.params.id)}`" />
        <ServerDetail v-else-if="isFixedRender" :key="String(route.params.id)" />
        <ServersOverview v-else />
    </div>
</template>
