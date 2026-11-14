<script setup lang="ts">
import { GridIcon, RightArrowIcon } from '@modrinth/assets'
import {
    ButtonLink,
    commonMessages,
    defineMessages,
    formatLoaderLabel,
    injectNotificationManager,
    useRelativeTime,
    useVIntl,
} from '@modrinth/ui'
import dayjs from 'dayjs'
import { computed } from 'vue'
import { RouterLink } from 'vue-router'

import type { HomeWidgetSize } from '@/components/home/home-dashboard'
import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import { useHomeRowMenu } from '@/components/home/home-row-menu'
import { getHomeItemVariant } from '@/components/home/home-utils'
import HomeFitList from '@/components/home/HomeFitList.vue'
import HomeGameDirectoryBadge from '@/components/home/HomeGameDirectoryBadge.vue'
import HomeItemRow from '@/components/home/HomeItemRow.vue'
import HomeRowMenu from '@/components/home/HomeRowMenu.vue'
import HomeWidgetFrame from '@/components/home/HomeWidgetFrame.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { trackEvent } from '@/helpers/analytics'
import { hasMultipleGameDirectories, resolveGameDirectory } from '@/helpers/game-directory'
import { kill, run, set_pinned } from '@/helpers/instance'
import { instanceLaunchStartedAt } from '@/helpers/instance-launch-state'
import type { GameInstance } from '@/helpers/types'
import { handleSevereError } from '@/store/error'

const props = defineProps<{
    instances: GameInstance[]
    dashboard?: boolean
    dashboardSize?: HomeWidgetSize | null
}>()

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const { runningInstanceIds } = useHomeDashboardRuntime()
const { instanceMenu } = useHomeRowMenu()
const messages = defineMessages({
    pinnedInstances: {
        id: 'app.home.instances.pinned',
        defaultMessage: 'Pinned instances',
    },
    emptyPinnedTitle: {
        id: 'app.home.instances.pinned-empty-title',
        defaultMessage: 'No pinned instances',
    },
    emptyPinned: {
        id: 'app.home.instances.pinned-empty',
        defaultMessage: 'Pin an instance from its card menu or the library to keep it here.',
    },
    viewAllInstances: {
        id: 'app.home.instances.view-all',
        defaultMessage: 'View all instances',
    },
    played: { id: 'app.instance.played', defaultMessage: 'Played {time}' },
    neverPlayed: { id: 'app.instance.never-played', defaultMessage: 'Never played' },
})

const pinnedInstances = computed(() =>
    props.instances
        .filter((instance) => instance.pinned_at)
        .slice()
        .sort(
            (a, b) => new Date(b.pinned_at ?? 0).getTime() - new Date(a.pinned_at ?? 0).getTime(),
        ),
)
const showGameDirectory = computed(() => hasMultipleGameDirectories(props.instances))

function instanceMeta(instance: GameInstance): string {
    const loader = instance.loader === 'vanilla' ? 'Minecraft' : formatLoaderLabel(instance.loader)
    const played = instance.last_played
        ? formatMessage(messages.played, {
              time: formatRelativeTime(dayjs(instance.last_played).toISOString()),
          })
        : formatMessage(messages.neverPlayed)
    return `${loader} ${instance.game_version} · ${played}`
}

async function playInstance(instance: GameInstance) {
    try {
        await run(instance.id)
        trackEvent('InstanceStart', {
            loader: instance.loader,
            game_version: instance.game_version,
            source: 'HomePinnedInstance',
        })
    } catch (error) {
        const handled = await handleMinecraftLaunchError(error, {
            instance_id: instance.id,
            instance_name: instance.name,
        })
        if (!handled) handleSevereError(error, { instanceId: instance.id })
    }
}

async function stopInstance(instance: GameInstance) {
    await kill(instance.id).catch(handleError)
    trackEvent('InstanceStop', {
        loader: instance.loader,
        game_version: instance.game_version,
        source: 'HomePinnedInstance',
    })
}

function rowMenu(instance: GameInstance) {
    return instanceMenu(instance, {
        pinned: true,
        update: () => set_pinned(instance.id, false),
    })
}

const variant = computed(() => getHomeItemVariant(props.dashboardSize))
</script>

<template>
    <HomeWidgetFrame
        class="home-pinned-instances"
        :data-size="dashboardSize"
        :title="formatMessage(messages.pinnedInstances)"
        :icon="GridIcon"
        :badge="pinnedInstances.length || null"
        :dense="dashboardSize?.endsWith('x1')"
        :empty-text="pinnedInstances.length === 0 ? formatMessage(messages.emptyPinnedTitle) : null"
        :empty-hint="formatMessage(messages.emptyPinned)"
        :empty-icon="GridIcon"
    >
        <template v-if="pinnedInstances.length > 0" #actions>
            <ButtonLink
                v-tooltip="formatMessage(messages.viewAllInstances)"
                :as="RouterLink"
                to="/library"
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.viewAllInstances)"
            >
                <RightArrowIcon aria-hidden="true" />
            </ButtonLink>
        </template>
        <template #empty-action>
            <ButtonLink :as="RouterLink" to="/library" size="sm" type="quiet" color="brand">
                <GridIcon aria-hidden="true" />
                {{ formatMessage(messages.viewAllInstances) }}
            </ButtonLink>
        </template>
        <HomeFitList
            v-slot="{ item: instance, variant: itemVariant }"
            :items="pinnedInstances"
            :item-key="(instance) => instance.id"
            :variant="variant"
        >
            <HomeItemRow
                :variant="itemVariant"
                :title="instance.name"
                :to="`/instance/${encodeURIComponent(instance.id)}`"
                :meta="instanceMeta(instance)"
                :instance="{
                    id: instance.id,
                    iconPath: instance.icon_path,
                    loader: instance.loader,
                }"
                :running="runningInstanceIds.includes(instance.id)"
                :starting="instanceLaunchStartedAt(instance.id) !== undefined"
                :play-label="formatMessage(commonMessages.playButton)"
                :stop-label="formatMessage(commonMessages.stopButton)"
                @play="playInstance(instance)"
                @stop="stopInstance(instance)"
            >
                <template v-if="showGameDirectory" #meta-prefix>
                    <HomeGameDirectoryBadge :directory="resolveGameDirectory(instance)" />
                </template>
                <template #actions>
                    <HomeRowMenu :options="rowMenu(instance)" />
                </template>
            </HomeItemRow>
        </HomeFitList>
    </HomeWidgetFrame>
</template>
