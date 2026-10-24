<script setup lang="ts">
import ServerOnlinePlayers from '@modrinth/ui/src/components/project/server/ServerOnlinePlayers.vue'
import ServerPing from '@modrinth/ui/src/components/project/server/ServerPing.vue'
import ServerRecentPlays from '@modrinth/ui/src/components/project/server/ServerRecentPlays.vue'
import ServerRegion from '@modrinth/ui/src/components/project/server/ServerRegion.vue'
import { computed } from 'vue'

import type { ProjectDetail } from '../../../data'

const props = defineProps<{ project: ProjectDetail }>()
const server = computed(() => props.project.minecraft_java_server)
const ping = computed(() => {
    const latency = server.value?.ping?.data?.latency
    return latency ? Math.round(latency.secs * 1000 + latency.nanos / 1e6) : undefined
})
</script>

<template>
    <ServerOnlinePlayers
        v-if="server?.ping?.data"
        :online="server.ping.data.players_online"
        status-online
        hide-label
    />
    <ServerRecentPlays :recent-plays="server?.verified_plays_2w ?? 0" hide-label />
    <ServerPing v-if="ping !== undefined" :ping="ping" />
    <ServerRegion
        v-if="project.minecraft_server?.region"
        :region="project.minecraft_server.region"
    />
</template>
