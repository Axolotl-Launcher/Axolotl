<script setup lang="ts">
import { ServerIcon, UsersIcon } from '@modrinth/assets'
import { defineMessages, NavTabs, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'

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
// 服务器详情页用固定高度布局：控制台内部滚动，命令输入框始终可见
const isStudioMode = computed(() => route.name === 'MultiplayerServerFileStudio')
const isFixedRender = computed(
    () => route.name === 'MultiplayerServerDetail' || route.name === 'MultiplayerServerFileStudio',
)
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
            <!--
				This bar drives routes rather than panels, so it has no tabpanel of
				its own: the outlet below renders child routes, and the detail and
				rooms views own their own tablists. Navigation mode would fit the
				routes but renders links rather than buttons, which normalise
				underlines, so the switch waits on a visual review.
			-->
            <NavTabs
                mode="local"
                :active-index="activeTab === 'rooms' ? 1 : 0"
                :links="tabLinks"
                @tab-click="handleTabClick"
            />
        </template>

        <RouterView />
    </div>
</template>
