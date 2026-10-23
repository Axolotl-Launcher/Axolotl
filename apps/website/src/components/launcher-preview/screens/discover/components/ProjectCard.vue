<script setup lang="ts">
import AutoLink from '@modrinth/ui/src/components/base/AutoLink.vue'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import SmartClickable from '@modrinth/ui/src/components/base/SmartClickable.vue'
import ProjectCardDate from '@modrinth/ui/src/components/project/card/ProjectCardDate.vue'
import ProjectCardEnvironment from '@modrinth/ui/src/components/project/card/ProjectCardEnvironment.vue'
import ProjectCardStats from '@modrinth/ui/src/components/project/card/ProjectCardStats.vue'
import ProjectCardTags from '@modrinth/ui/src/components/project/card/ProjectCardTags.vue'
import ProjectCardTitle from '@modrinth/ui/src/components/project/card/ProjectCardTitle.vue'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

/**
 * Port of the shared `ProjectCard` (packages/ui/src/components/project/card/ProjectCard.vue)
 * in its list, compact and grid layouts. Grid cards without a gallery keep the project
 * gradient; the app's remote landing artwork behind it is not loaded into the preview.
 */
type Environment = 'required' | 'optional' | 'unsupported' | 'unknown'
type DisplayedDate =
    | 'updated'
    | 'published'
    | 'created'
    | 'projectModified'
    | 'releaseUpdated'
    | 'projectPublished'
    | 'firstRelease'

const props = defineProps<{
    layout: 'list' | 'compact' | 'grid'
    link?: string | RouteLocationRaw
    iconUrl?: string
    title: string
    summary?: string
    tags?: string[]
    allTags?: string[]
    deprioritizedTags?: string[]
    excludeLoaders?: boolean
    downloads?: number
    dateUpdated?: string
    datePublished?: string
    displayedDate?: DisplayedDate
    banner?: string
    color?: string | number
    environment?: { clientSide: Environment; serverSide: Environment }
    maxTags?: number
    providers?: string[]
    provider?: string
}>()

defineEmits<{ mouseenter: []; mouseleave: [] }>()

const cardRef = ref<InstanceType<typeof SmartClickable> | null>(null)
const cardWidth = ref(0)
let observer: ResizeObserver | null = null

onMounted(() => {
    const element = (cardRef.value as unknown as { $el?: HTMLElement } | null)?.$el
    if (!element) return
    cardWidth.value = element.getBoundingClientRect().width
    observer = new ResizeObserver(([entry]) => {
        cardWidth.value = entry.contentRect.width
    })
    observer.observe(element)
})
onBeforeUnmount(() => observer?.disconnect())

const providerNames: Record<string, string> = {
    modrinth: 'Modrinth',
    curseforge: 'CurseForge',
    mcarchive: 'MCArchive',
    planet_minecraft: 'Planet Minecraft',
}

const providerLabels = computed(() =>
    [
        ...new Set(
            props.providers?.length ? props.providers : props.provider ? [props.provider] : [],
        ),
    ].map((source) => providerNames[source] ?? source),
)

const baseCardStyle =
    'w-full h-full border-[1px] border-solid border-surface-4 overflow-hidden bg-surface-3 rounded-2xl transition-all smart-clickable:outline-on-focus smart-clickable:highlight-on-hover'

const compactCardStyle =
    'w-full h-16 border-[1px] border-solid border-surface-4 overflow-hidden bg-surface-2 rounded-lg px-2 py-[7px] grid grid-cols-[48px_minmax(0,1fr)_5rem_10rem_4.5rem_4.5rem_5.5rem] items-center gap-3 transition-all smart-clickable:outline-on-focus smart-clickable:highlight-on-hover'

const updatedDate = computed(() => (props.dateUpdated ? new Date(props.dateUpdated) : undefined))
const publishedDate = computed(() =>
    props.datePublished ? new Date(props.datePublished) : undefined,
)

const autoDisplayDate = computed<DisplayedDate | undefined>(() => {
    if (props.displayedDate) return props.displayedDate
    if (props.dateUpdated) return 'updated'
    if (props.datePublished) return 'published'
    return undefined
})

const date = computed(() => {
    const type = autoDisplayDate.value ?? ''
    if (['updated', 'projectModified', 'releaseUpdated'].includes(type)) return updatedDate.value
    if (['published', 'created', 'projectPublished', 'firstRelease'].includes(type))
        return publishedDate.value
    return undefined
})

const extraTags = computed(() => props.allTags?.filter((tag) => !props.tags?.includes(tag)))

const computedMaxTags = computed(() => {
    const environmentOffset = props.environment ? 0 : 1
    if (props.maxTags !== undefined) return props.maxTags + environmentOffset
    if (props.layout === 'grid') {
        if (cardWidth.value >= 600) return 5 + environmentOffset
        if (cardWidth.value >= 400) return 4 + environmentOffset
        return 3 + environmentOffset
    }
    if (cardWidth.value >= 800) return 8 + environmentOffset
    if (cardWidth.value >= 600) return 6 + environmentOffset
    if (cardWidth.value >= 400) return 5 + environmentOffset
    return 4 + environmentOffset
})

const cssColor = computed(() => {
    if (props.color === undefined || typeof props.color === 'string') return props.color
    const color = props.color >>> 0
    const b = color & 0xff
    const g = (color & 0xff00) >>> 8
    const r = (color & 0xff0000) >>> 16
    return `rgba(${r},${g},${b},1)`
})
</script>

<template>
    <SmartClickable ref="cardRef" class="project-card-container w-full">
        <template v-if="link" #clickable>
            <AutoLink
                :to="link"
                class="no-outline no-click-animation custom-focus-indicator rounded-xl"
                @mouseenter="$emit('mouseenter')"
                @mouseleave="$emit('mouseleave')"
            ></AutoLink>
        </template>
        <div v-if="layout === 'compact'" :class="[compactCardStyle, 'project-card--compact']">
            <Avatar :src="iconUrl" size="48px" class="ease-brightness" no-shadow />
            <div class="project-card--compact__identity flex min-w-0 flex-col justify-center gap-1">
                <div class="flex min-w-0 items-center gap-2">
                    <ProjectCardTitle :title="title" dense />
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
                class="smart-clickable:allow-pointer-events flex justify-end gap-1 empty:hidden"
            >
                <slot name="actions" />
            </div>
        </div>
        <div v-else-if="layout === 'grid'" :class="[baseCardStyle, 'flex flex-col']">
            <div
                :style="{ '--_project-color': cssColor }"
                class="bg-project-gradient relative aspect-[2/1] w-full overflow-clip border-0 border-b-[1px] border-solid border-surface-4"
            >
                <img
                    v-if="banner"
                    :src="banner"
                    alt=""
                    loading="lazy"
                    decoding="async"
                    class="absolute inset-0 h-full w-full object-cover object-center"
                />
            </div>
            <div class="flex grow flex-col gap-3 p-4">
                <div class="flex gap-3">
                    <Avatar
                        :src="iconUrl"
                        size="96px"
                        class="project-card__icon ease-brightness"
                        no-shadow
                    />
                    <div class="flex w-full flex-col gap-2">
                        <div class="grid grid-cols-[1fr_auto] gap-4">
                            <div class="flex min-w-0 flex-col gap-1">
                                <div class="flex min-w-0 items-center gap-2">
                                    <ProjectCardTitle :title="title" compact />
                                    <span
                                        v-for="label in providerLabels"
                                        :key="label"
                                        class="rounded-full bg-surface-5 px-2 py-0.5 text-xs font-semibold text-[var(--color-text-tertiary)]"
                                    >
                                        {{ label }}
                                    </span>
                                </div>
                                <div class="m-0 line-clamp-2 font-normal">
                                    {{ summary }}
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                <div
                    class="project-card--grid__footer mt-auto flex items-end justify-between gap-3"
                >
                    <div class="flex min-w-0 grow flex-col gap-3">
                        <div class="flex min-w-0 flex-wrap items-center gap-1">
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
                        <div v-if="downloads !== undefined" class="flex w-fit flex-col gap-1">
                            <ProjectCardStats :downloads="downloads" />
                            <ProjectCardDate
                                v-if="date && autoDisplayDate"
                                :type="autoDisplayDate"
                                :date="date"
                            />
                        </div>
                    </div>
                    <div
                        class="project-card--grid__actions smart-clickable:allow-pointer-events flex shrink-0 gap-2 empty:hidden"
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
                'grid-project-card-list grid gap-x-3 gap-y-2 p-4',
                { 'has-actions': !!$slots.actions },
            ]"
        >
            <Avatar
                :src="iconUrl"
                size="100px"
                class="project-card__icon grid-project-card-list__icon ease-brightness"
                no-shadow
            />
            <div class="grid-project-card-list__info flex min-w-0 flex-col gap-2">
                <div class="flex min-w-0 items-center gap-2">
                    <ProjectCardTitle :title="title" />
                    <span
                        v-for="label in providerLabels"
                        :key="label"
                        class="rounded-full bg-surface-5 px-2 py-0.5 text-xs font-semibold text-[var(--color-text-tertiary)]"
                    >
                        {{ label }}
                    </span>
                </div>
                <div class="project-card-summary m-0 line-clamp-2 font-normal">
                    {{ summary }}
                </div>
            </div>
            <div
                v-if="!!$slots.actions"
                class="smart-clickable:allow-pointer-events grid-project-card-list__actions ml-auto flex shrink-0 gap-1 empty:hidden"
            >
                <slot name="actions" />
            </div>
            <div
                class="grid-project-card-list__stats ml-auto flex shrink-0 flex-col items-end gap-3 empty:hidden"
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
            <div class="grid-project-card-list__tags mt-auto flex items-center gap-3">
                <div class="flex w-full items-center gap-2">
                    <div class="flex items-center gap-1">
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
                </div>
            </div>
        </div>
    </SmartClickable>
</template>

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
        align-items: flex-start;
        width: 100%;
    }

    .grid-project-card-list__info {
        gap: 0.125rem;
    }

    .project-card-summary {
        font-size: 0.875rem;
        line-height: 1.25rem;
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
