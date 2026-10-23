<script setup lang="ts">
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import RightArrowIcon from '@modrinth/assets/icons/right-arrow.svg?component'
import ButtonLink from '@modrinth/ui/src/components/base/buttons/ButtonLink.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'
import { RouterLink } from 'vue-router'

import { usePreview } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'
import type { HomeWidgetSize } from '../home-dashboard'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import HomeInstanceCard from './HomeInstanceCard.vue'

const props = defineProps<{
    instances: GameInstance[]
    dashboard?: boolean
    dashboardSize?: HomeWidgetSize | null
}>()

const { formatMessage } = useVIntl()
const preview = usePreview()
const { runningInstanceIds } = useHomeRuntime()
const messages = homeMessages

/** Pinned instances, most recently pinned first (the app sorts by `pinned_at`). */
const pinnedInstances = computed(() => {
    const order = preview.instances.pinnedIds.value
    return props.instances
        .filter((instance) => order.includes(instance.id))
        .sort((a, b) => order.indexOf(b.id) - order.indexOf(a.id))
})
const cardLayout = computed(() => {
    if (props.dashboardSize === '1x1') return 'spotlight' as const
    if (props.dashboardSize === '2x2') return 'tile' as const
    return 'row' as const
})

/** `set_pinned(instance.id, pinned)`. */
function updatePinned(instance: GameInstance, pinned: boolean) {
    if (pinned) preview.instances.pin(instance.id)
    else preview.instances.unpin(instance.id)
}
</script>

<template>
    <section
        class="home-pinned-instances flex h-full min-h-0 min-w-0 flex-col gap-3"
        :data-size="dashboardSize"
    >
        <div class="home-widget-heading flex h-8 min-w-0 flex-none items-center gap-2">
            <h2>
                {{ formatMessage(messages.instancesPinned) }}
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
                    formatMessage(messages.instancesViewAll)
                }}</span>
                <RightArrowIcon aria-hidden="true" />
            </ButtonLink>
        </div>
        <div
            v-if="pinnedInstances.length > 0"
            class="home-instance-list grid-auto-rows-max grid min-h-0 min-w-0 flex-1 gap-1 overflow-y-auto overflow-x-hidden pr-1"
        >
            <HomeInstanceCard
                v-for="instance in pinnedInstances"
                :key="instance.id"
                :instance="instance"
                :pinned="true"
                :layout="cardLayout"
                :playing="runningInstanceIds.includes(instance.id)"
                @pinned-change="updatePinned"
            />
        </div>
        <div v-else class="home-widget-empty">
            <GridIcon aria-hidden="true" />
            <span>{{ formatMessage(messages.instancesPinnedEmpty) }}</span>
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
