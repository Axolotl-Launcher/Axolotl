<script setup lang="ts">
import { GridIcon, RightArrowIcon } from '@modrinth/assets'
import { ButtonLink, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'
import { RouterLink } from 'vue-router'

import type { HomeWidgetSize } from '@/components/home/home-dashboard'
import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import HomeInstanceCard from '@/components/home/HomeInstanceCard.vue'
import { hasMultipleGameDirectories } from '@/helpers/game-directory'
import { set_pinned } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'

const props = defineProps<{
    instances: GameInstance[]
    dashboard?: boolean
    dashboardSize?: HomeWidgetSize | null
}>()

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { runningInstanceIds } = useHomeDashboardRuntime()
const messages = defineMessages({
    pinnedInstances: {
        id: 'app.home.instances.pinned',
        defaultMessage: 'Pinned instances',
    },
    emptyPinned: {
        id: 'app.home.instances.pinned-empty',
        defaultMessage: 'Pin an instance from its card menu or the library to keep it here.',
    },
    viewAllInstances: {
        id: 'app.home.instances.view-all',
        defaultMessage: 'View all instances',
    },
})

const pinnedInstances = computed(() =>
    props.instances
        .filter((instance) => instance.pinned_at)
        .slice()
        .sort(
            (a, b) => new Date(b.pinned_at ?? 0).getTime() - new Date(a.pinned_at ?? 0).getTime(),
        ),
)
const cardLayout = computed(() => {
    if (props.dashboardSize === '1x1') return 'spotlight' as const
    if (props.dashboardSize === '2x2') return 'tile' as const
    return 'row' as const
})
const showGameDirectory = computed(() => hasMultipleGameDirectories(props.instances))

async function updatePinned(instance: GameInstance, pinned: boolean) {
    await set_pinned(instance.id, pinned).catch(handleError)
}
</script>

<template>
    <section
        class="home-pinned-instances flex min-w-0 min-h-0 h-full flex-col gap-3"
        :data-size="dashboardSize"
    >
        <div class="home-widget-heading flex min-w-0 h-8 flex-none items-center gap-2">
            <h2>
                {{ formatMessage(messages.pinnedInstances) }}
            </h2>
            <ButtonLink
                v-if="dashboardSize !== '1x1'"
                :as="RouterLink"
                to="/library"
                type="quiet"
                size="2xs"
                class="ml-auto"
            >
                <span v-if="dashboardSize === '2x2'">{{
                    formatMessage(messages.viewAllInstances)
                }}</span>
                <RightArrowIcon aria-hidden="true" />
            </ButtonLink>
        </div>
        <div
            v-if="pinnedInstances.length > 0"
            class="home-instance-list grid min-w-0 min-h-0 flex-1 grid-auto-rows-max gap-1 overflow-x-hidden overflow-y-auto pr-1"
        >
            <HomeInstanceCard
                v-for="instance in pinnedInstances"
                :key="instance.id"
                :instance="instance"
                :pinned="true"
                :layout="cardLayout"
                :playing="runningInstanceIds.includes(instance.id)"
                :show-game-directory="showGameDirectory"
                @pinned-change="updatePinned"
            />
        </div>
        <div v-else class="home-widget-empty">
            <GridIcon aria-hidden="true" />
            <span>{{ formatMessage(messages.emptyPinned) }}</span>
        </div>
    </section>
</template>

<style scoped>
.home-widget-heading h2 {
    min-width: 0;
    overflow: hidden;
    margin: 0;
    color: var(--color-text-primary);
    font-size: 1rem;
    font-weight: 700;
    letter-spacing: 0;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.home-pinned-instances[data-size='2x1'] .home-instance-list,
.home-pinned-instances[data-size='2x2'] .home-instance-list {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    column-gap: 0.5rem;
}

.home-pinned-instances[data-size='1x1'] {
    gap: 0.375rem;
}

.home-pinned-instances[data-size='1x1'] .home-widget-heading {
    height: 1.5rem;
}

.home-widget-empty {
    display: flex;
    max-width: 22rem;
    margin: auto;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    color: var(--color-text-tertiary);
    font-size: 0.8125rem;
    line-height: 1.4;
    text-align: center;
}

.home-widget-empty svg {
    width: 1.5rem;
    height: 1.5rem;
    opacity: 0.7;
}
</style>
