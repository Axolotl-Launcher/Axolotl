<template>
    <SmartClickable ref="cardRef" class="w-full project-card-container">
        <template v-if="link" #clickable>
            <AutoLink
                :to="link"
                class="rounded-xl no-outline no-click-animation custom-focus-indicator"
                @mouseenter="$emit('mouseenter')"
                @mouseleave="$emit('mouseleave')"
            ></AutoLink>
        </template>
        <div
            v-if="layout === 'compact'"
            :class="[
                compactCardStyle,
                'project-card--compact',
                { 'project-card--compact-server': isServerProject },
            ]"
        >
            <Avatar :src="iconUrl" size="48px" class="ease-brightness" no-shadow />
            <div class="project-card--compact__identity flex min-w-0 flex-col justify-center gap-1">
                <div class="flex min-w-0 items-center gap-2">
                    <ProjectCardTitle :title="title" dense />
                    <ProjectStatusBadge v-if="status" :status="status" class="text-sm" />
                </div>
                <p
                    v-if="summary"
                    class="m-0 truncate text-[13px] leading-4 text-[var(--color-text-tertiary)]"
                >
                    {{ summary }}
                </p>
            </div>
            <AutoLink
                v-tooltip="providerLabels.join(', ')"
                :to="link"
                class="project-card--compact__provider smart-clickable:allow-pointer-events min-w-0 truncate text-xs font-semibold text-[var(--color-text-tertiary)]"
            >
                {{ providerLabels.join(', ') }}
            </AutoLink>
            <div
                class="project-card--compact__tags flex min-w-0 items-center gap-1 overflow-hidden"
            >
                <template v-if="isServerProject">
                    <ServerOnlinePlayers
                        v-if="serverOnlinePlayers !== undefined"
                        :online="serverOnlinePlayers"
                        :status-online="serverStatusOnline"
                        :hide-label="true"
                    />
                    <ServerRecentPlays
                        v-if="serverRecentPlays !== undefined"
                        :recent-plays="serverRecentPlays"
                        :hide-label="true"
                    />
                    <ServerPing v-if="serverPing && serverStatusOnline" :ping="serverPing" />
                    <ServerRegion
                        v-if="serverRegion"
                        :region="serverRegion"
                        class="smart-clickable:allow-pointer-events"
                    />
                </template>
                <template v-else>
                    <ProjectCardEnvironment
                        v-if="environment"
                        :client-side="environment.clientSide"
                        :server-side="environment.serverSide"
                    />
                    <ProjectCardTags
                        v-if="tags"
                        :tags="tags"
                        :extra-tags="extraTags"
                        :exclude-loaders="excludeLoaders"
                        :deprioritized-tags="deprioritizedTags"
                        :max-tags="2"
                    />
                </template>
            </div>
            <ProjectCardStats
                v-if="downloads !== undefined"
                class="project-card--compact__downloads gap-1 text-[13px] [&>svg]:size-4"
                :downloads="downloads"
            />
            <ProjectCardDate
                v-if="date && autoDisplayDate"
                class="project-card--compact__date gap-1 text-[13px] [&>svg]:size-4"
                :type="autoDisplayDate"
                :date="date"
            />
            <div
                v-if="!!$slots.actions"
                class="flex justify-end gap-1 empty:hidden smart-clickable:allow-pointer-events"
            >
                <slot name="actions" />
            </div>
        </div>
        <div v-else-if="layout === 'grid'" :class="[baseCardStyle, 'flex flex-col']">
            <div
                :style="{ '--_project-color': cssColor }"
                class="relative bg-project-gradient overflow-clip aspect-[2/1] w-full border-0 border-b-[1px] border-solid border-surface-4"
            >
                <img
                    v-if="banner"
                    :src="banner"
                    alt=""
                    class="absolute w-full h-full inset-0 object-cover object-center"
                />
                <img
                    v-else
                    src="https://cdn-raw.modrinth.com/landing-new/landing.webp"
                    alt=""
                    class="absolute w-full h-full inset-0 object-cover object-center opacity-70 scale-[200%]"
                />
            </div>
            <div class="p-4 flex flex-col gap-3 grow">
                <div class="flex gap-3">
                    <Avatar
                        :src="iconUrl"
                        size="96px"
                        class="project-card__icon ease-brightness"
                        no-shadow
                    />
                    <div class="flex flex-col gap-2 w-full">
                        <div class="grid grid-cols-[1fr_auto] gap-4">
                            <div class="flex min-w-0 flex-col gap-1">
                                <div class="flex min-w-0 gap-2 items-center">
                                    <ProjectCardTitle :title="title" compact />
                                    <span
                                        v-for="label in providerLabels"
                                        :key="label"
                                        class="rounded-full bg-surface-5 px-2 py-0.5 text-xs font-semibold text-[var(--color-text-tertiary)]"
                                    >
                                        {{ label }}
                                    </span>
                                    <ProjectStatusBadge
                                        v-if="status"
                                        :status="status"
                                        class="text-sm"
                                    />
                                </div>
                                <div class="m-0 font-normal line-clamp-2">
                                    {{ summary }}
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                <div
                    class="project-card--grid__footer mt-auto flex gap-3 justify-between items-end"
                    :class="{ 'project-card--grid__footer-server': isServerProject }"
                >
                    <div class="flex flex-col gap-3 grow min-w-0">
                        <div class="flex flex-wrap items-center gap-1 min-w-0">
                            <template v-if="isServerProject">
                                <ServerOnlinePlayers
                                    v-if="serverOnlinePlayers !== undefined"
                                    :online="serverOnlinePlayers"
                                    :status-online="serverStatusOnline"
                                    :hide-label="true"
                                />
                                <ServerRecentPlays
                                    v-if="serverRecentPlays !== undefined"
                                    :recent-plays="serverRecentPlays"
                                    :hide-label="true"
                                />
                                <ServerPing
                                    v-if="serverPing && serverStatusOnline"
                                    :ping="serverPing"
                                />
                                <ServerRegion
                                    v-if="serverRegion"
                                    :region="serverRegion"
                                    class="smart-clickable:allow-pointer-events"
                                />
                            </template>
                            <ProjectCardEnvironment
                                v-if="environment"
                                :client-side="environment.clientSide"
                                :server-side="environment.serverSide"
                            />
                            <ProjectCardTags
                                v-if="tags"
                                :tags="tags"
                                :extra-tags="extraTags"
                                :exclude-loaders="excludeLoaders"
                                :deprioritized-tags="deprioritizedTags"
                                :max-tags="computedMaxTags"
                            />
                            <ServerModpackContent
                                v-if="serverModpackContent"
                                :name="serverModpackContent.name"
                                :icon="serverModpackContent.icon"
                                :onclick="serverModpackContent.onclick"
                                :show-custom-modpack-tooltip="
                                    serverModpackContent.showCustomModpackTooltip
                                "
                                class="text-[var(--color-text-default)]"
                            />
                        </div>
                        <div v-if="downloads !== undefined" class="flex flex-col gap-1 w-fit">
                            <ProjectCardStats :downloads="downloads" />
                            <ProjectCardDate
                                v-if="date && autoDisplayDate"
                                :type="autoDisplayDate"
                                :date="date"
                            />
                        </div>
                    </div>
                    <div
                        class="project-card--grid__actions flex gap-2 shrink-0 empty:hidden smart-clickable:allow-pointer-events"
                    >
                        <slot name="actions" />
                    </div>
                </div>
            </div>
        </div>
        <div
            v-else
            :class="[
                baseCardStyle,
                'p-4 grid grid-project-card-list gap-x-3 gap-y-2',
                { 'has-actions': !!$slots.actions },
            ]"
        >
            <Avatar
                :src="iconUrl"
                size="100px"
                class="project-card__icon grid-project-card-list__icon ease-brightness"
                no-shadow
            />
            <div class="flex min-w-0 flex-col gap-2 grid-project-card-list__info">
                <div class="flex min-w-0 gap-2 items-center">
                    <ProjectCardTitle :title="title" />
                    <span
                        v-for="label in providerLabels"
                        :key="label"
                        class="rounded-full bg-surface-5 px-2 py-0.5 text-xs font-semibold text-[var(--color-text-tertiary)]"
                    >
                        {{ label }}
                    </span>
                    <ProjectStatusBadge v-if="status" :status="status" />
                </div>
                <div class="project-card-summary m-0 font-normal line-clamp-2">
                    {{ summary }}
                </div>
            </div>

            <div
                v-if="!!$slots.actions"
                class="flex gap-1 shrink-0 ml-auto empty:hidden smart-clickable:allow-pointer-events grid-project-card-list__actions"
            >
                <slot name="actions" />
            </div>
            <div
                class="flex flex-col gap-3 items-end shrink-0 ml-auto empty:hidden grid-project-card-list__stats"
                :class="{ 'mt-3': !!$slots.actions }"
            >
                <div v-if="downloads !== undefined" class="flex items-center gap-3">
                    <ProjectCardStats :downloads="downloads" />
                </div>
                <ProjectCardDate
                    v-if="date && autoDisplayDate"
                    :type="autoDisplayDate"
                    :date="date"
                />
            </div>
            <div class="mt-auto flex items-center gap-3 grid-project-card-list__tags">
                <div class="flex items-center gap-2 w-full">
                    <template v-if="isServerProject">
                        <ServerOnlinePlayers
                            v-if="serverOnlinePlayers !== undefined"
                            :online="serverOnlinePlayers"
                            :status-online="serverStatusOnline"
                            :hide-label="true"
                        />
                        <ServerRecentPlays
                            v-if="serverRecentPlays !== undefined"
                            :recent-plays="serverRecentPlays"
                            :hide-label="true"
                        />
                    </template>
                    <div class="flex items-center gap-1">
                        <template v-if="isServerProject">
                            <ServerPing
                                v-if="serverPing && serverStatusOnline"
                                :ping="serverPing"
                            />
                            <ServerRegion
                                v-if="serverRegion"
                                :region="serverRegion"
                                class="smart-clickable:allow-pointer-events"
                            />
                        </template>
                        <ProjectCardEnvironment
                            v-if="environment"
                            :client-side="environment.clientSide"
                            :server-side="environment.serverSide"
                        />
                        <ProjectCardTags
                            v-if="tags"
                            :tags="tags"
                            :extra-tags="extraTags"
                            :exclude-loaders="excludeLoaders"
                            :deprioritized-tags="deprioritizedTags"
                            :max-tags="computedMaxTags"
                        />
                    </div>
                    <ServerModpackContent
                        v-if="serverModpackContent"
                        :name="serverModpackContent.name"
                        :icon="serverModpackContent.icon"
                        :onclick="serverModpackContent.onclick"
                        :show-custom-modpack-tooltip="serverModpackContent.showCustomModpackTooltip"
                        class="text-[var(--color-text-default)]"
                    />
                </div>
            </div>
        </div>
    </SmartClickable>
</template>

<script setup lang="ts">
import type { ProjectStatus } from '@modrinth/utils'
import { useElementSize } from '@vueuse/core'
import dayjs from 'dayjs'
import { computed, ref } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

import { AutoLink, Avatar } from '../../base'
import { SmartClickable } from '../../base/index.ts'
import ProjectStatusBadge from '../ProjectStatusBadge.vue'
import ServerModpackContent from '../server/ServerModpackContent.vue'
import ServerOnlinePlayers from '../server/ServerOnlinePlayers.vue'
import ServerPing from '../server/ServerPing.vue'
import ServerRecentPlays from '../server/ServerRecentPlays.vue'
import ServerRegion from '../server/ServerRegion.vue'
import ProjectCardDate from './ProjectCardDate.vue'
import ProjectCardEnvironment, {
    type ProjectCardEnvironmentProps,
} from './ProjectCardEnvironment.vue'
import ProjectCardStats from './ProjectCardStats.vue'
import ProjectCardTags from './ProjectCardTags.vue'
import ProjectCardTitle from './ProjectCardTitle.vue'

defineEmits<{
    mouseenter: []
    mouseleave: []
}>()

const cardRef = ref<InstanceType<typeof SmartClickable> | null>(null)
const cardElement = computed(() => cardRef.value?.$el as HTMLElement | undefined)
const { width: cardWidth } = useElementSize(cardElement)

const props = defineProps<{
    layout: 'list' | 'compact' | 'grid'
    link?: string | RouteLocationRaw | (() => void)
    iconUrl?: string
    title: string
    author?: {
        name: string
        link?: string
    }
    summary?: string
    tags?: string[]
    allTags?: string[]
    deprioritizedTags?: string[]
    excludeLoaders?: boolean
    downloads?: number
    dateUpdated?: string
    datePublished?: string
    displayedDate?:
        | 'updated'
        | 'published'
        | 'created'
        | 'projectModified'
        | 'releaseUpdated'
        | 'projectPublished'
        | 'firstRelease'
    serverRegion?: string
    serverOnlinePlayers?: number
    serverStatusOnline?: boolean
    serverRecentPlays?: number
    serverPing?: number
    serverModpackContent?: {
        name: string
        icon?: string
        onclick?: () => void
        showCustomModpackTooltip?: boolean
    }
    isServerProject?: boolean
    banner?: string
    color?: string | number
    environment?: ProjectCardEnvironmentProps
    status?: ProjectStatus
    maxTags?: number
    providers?: ('modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft')[]
    provider?: 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'
}>()

const providerLabels = computed(() => {
    const names = {
        modrinth: 'Modrinth',
        curseforge: 'CurseForge',
        mcarchive: 'MCArchive',
        planet_minecraft: 'Planet Minecraft',
    }
    return [
        ...new Set(
            props.providers?.length ? props.providers : props.provider ? [props.provider] : [],
        ),
    ].map((source) => names[source])
})

const baseCardStyle =
    'w-full h-full border-[1px] border-solid border-surface-4 overflow-hidden bg-surface-3 rounded-2xl transition-all smart-clickable:outline-on-focus smart-clickable:highlight-on-hover'

const compactCardStyle =
    'w-full h-16 border-[1px] border-solid border-surface-4 overflow-hidden bg-surface-2 rounded-lg px-2 py-[7px] grid grid-cols-[48px_minmax(0,1fr)_5rem_10rem_4.5rem_4.5rem_5.5rem] items-center gap-3 transition-all smart-clickable:outline-on-focus smart-clickable:highlight-on-hover'

const updatedDate = computed(() =>
    props.dateUpdated ? dayjs(props.dateUpdated).toDate() : undefined,
)
const publishedDate = computed(() =>
    props.datePublished ? dayjs(props.datePublished).toDate() : undefined,
)

const autoDisplayDate = computed(() => {
    if (props.displayedDate) {
        return props.displayedDate
    } else if (props.dateUpdated) {
        return 'updated'
    } else if (props.datePublished) {
        return 'published'
    } else {
        return undefined
    }
})

const date = computed(() => {
    if (['updated', 'projectModified', 'releaseUpdated'].includes(autoDisplayDate.value ?? '')) {
        return updatedDate.value
    } else if (
        ['published', 'created', 'projectPublished', 'firstRelease'].includes(
            autoDisplayDate.value ?? '',
        )
    ) {
        return publishedDate.value
    }
    return undefined
})

const extraTags = computed(() => props.allTags?.filter((tag) => !props.tags?.includes(tag)))

const computedMaxTags = computed(() => {
    // 如果外部传入了 maxTags，优先使用
    if (props.maxTags !== undefined) {
        return props.maxTags + (props.environment ? 0 : 1)
    }

    const environmentOffset = props.environment ? 0 : 1

    // 根据卡片宽度动态计算
    if (props.layout === 'grid') {
        // Grid 布局：长方形，底部空间窄
        if (cardWidth.value >= 600) return 5 + environmentOffset
        if (cardWidth.value >= 400) return 4 + environmentOffset
        return 3 + environmentOffset
    } else {
        // List 布局：长条形，横向空间充足
        if (cardWidth.value >= 800) return 8 + environmentOffset
        if (cardWidth.value >= 600) return 6 + environmentOffset
        if (cardWidth.value >= 400) return 5 + environmentOffset
        return 4 + environmentOffset
    }
})

const cssColor = computed(() => {
    if (props.color === undefined || typeof props.color === 'string') {
        return props.color
    }

    const color = props.color >>> 0
    const b = color & 0xff
    const g = (color & 0xff00) >>> 8
    const r = (color & 0xff0000) >>> 16
    return 'rgba(' + [r, g, b, 1].join(',') + ')'
})
</script>
<style scoped>
.no-outline {
    outline: none;
}

:deep(.project-card-container) {
    container-type: inline-size;
}

.grid-project-card-list {
    grid-template:
        'icon info stats stats'
        'icon info stats stats'
        'icon tags tags tags';
    grid-template-columns: auto 1fr auto auto;
}

.grid-project-card-list.has-actions {
    grid-template:
        'icon info actions actions'
        'icon info dummy stats'
        'icon tags tags stats';
    grid-template-columns: auto 1fr auto auto;
}

.grid-project-card-list__icon {
    grid-area: icon;
}

.grid-project-card-list__info {
    grid-area: info;
}

.grid-project-card-list__actions {
    grid-area: actions;
}

.grid-project-card-list__stats {
    grid-area: stats;
}

.grid-project-card-list__tags {
    grid-area: tags;
}

.project-card--compact-server {
    grid-template-columns: 48px minmax(0, 1fr) minmax(0, auto) auto;
}

@container (width < 720px) {
    .project-card--compact {
        grid-template-columns: 48px minmax(0, 1fr) 5rem 4.5rem 4.5rem 5.5rem;
    }

    .project-card--compact__tags {
        display: none;
    }
}

@container (width < 600px) {
    .project-card--compact {
        grid-template-columns: 48px minmax(0, 1fr) 5rem 4.5rem 5.5rem;
    }

    .project-card--compact__date {
        display: none;
    }
}

@container (width < 520px) {
    .project-card--compact {
        grid-template-columns: 48px minmax(0, 1fr) 5rem 5.5rem;
    }

    .project-card--compact__downloads {
        display: none;
    }
}

@container (width < 440px) {
    .project-card--compact {
        grid-template-columns: 48px minmax(0, 1fr) 5.5rem;
    }

    .project-card--compact__provider {
        display: none;
    }
}

@container (width < 720px) {
    .project-card--compact-server {
        grid-template-columns: 48px minmax(0, 1fr) auto;
    }
}

@container (width < 600px) {
    .project-card--grid__footer-server {
        align-items: stretch;
        flex-direction: column;
    }

    .project-card--grid__footer-server .project-card--grid__actions {
        align-self: flex-end;
    }
}

@container (width < 850px) {
    .project-card__icon {
        --_override-size: 64px;
    }

    .grid-project-card-list {
        grid-template:
            'icon info stats'
            'icon info stats'
            'tags tags tags';
        grid-template-columns: auto 1fr auto;
    }

    .grid-project-card-list.has-actions {
        grid-template:
            'icon info actions'
            'icon info stats'
            'tags tags stats';
        grid-template-columns: auto 1fr auto;
    }
}

@container (width < 550px) {
    .project-card__icon {
        --_override-size: 64px;
    }

    .grid-project-card-list {
        grid-template:
            'icon info'
            'icon info'
            'tags tags'
            'stats stats';
        grid-template-columns: auto 1fr;
    }

    .grid-project-card-list.has-actions {
        grid-template:
            'icon info'
            'icon info'
            'tags tags'
            'stats stats'
            'actions actions';
        grid-template-columns: auto 1fr;
    }

    .grid-project-card-list__stats,
    .grid-project-card-list__actions {
        @apply items-start w-full;
    }

    .grid-project-card-list__info {
        @apply gap-0.5;
    }

    .project-card-summary {
        @apply text-sm;
    }
}

/*noinspection CssUnresolvedCustomProperty*/
.bg-project-gradient {
    --_gradient-start: var(--_project-color, #000);
    --_gradient-end: var(--_project-color, #000);
    @supports (background-color: oklch(from var(--_project-color, #000) l c h)) {
        --_gradient-start: oklch(
            from var(--_project-color, #000) calc(l * 0.8) calc(c * 0.8) calc(h + 15)
        );
        --_gradient-end: oklch(from var(--_project-color, #000) calc(l * 0.5) calc(c * 0.9) h);
    }
    background-color: var(--_gradient-start);
    background-image: linear-gradient(
        to bottom right,
        var(--_gradient-start),
        var(--_gradient-end)
    );
}
</style>
