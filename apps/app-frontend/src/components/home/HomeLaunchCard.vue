<script setup lang="ts">
import './home-widget.css'

import {
    ArrowLeftRightIcon,
    ClockIcon,
    CompassIcon,
    DownloadIcon,
    GameIcon,
    ListIcon,
    PackagePlusIcon,
    PlayIcon,
    PlusIcon,
    RightArrowIcon,
    ServerIcon,
    SpinnerIcon,
    StopCircleIcon,
    TimerIcon,
} from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    formatLoaderLabel,
    useRelativeTime,
    useVIntl,
} from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import dayjs from 'dayjs'
import { computed, inject } from 'vue'

import GameDirectoryBadge from '@/components/ui/instance/GameDirectoryBadge.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { hasMultipleGameDirectories, resolveGameDirectory } from '@/helpers/game-directory'
import { getDisplayInstanceIcon } from '@/helpers/instance-icons'
import type { GameInstance } from '@/helpers/types'
import { get_recent_worlds } from '@/helpers/worlds'

import type { HomeWidgetPlacement, HomeWidgetSize } from './home-dashboard'
import { useOptionalHomeDashboardRuntime } from './home-dashboard-runtime'
import { useIconTint } from './home-icon-tint'
import { HOME_LAUNCH_KEY, useHomeLaunchState } from './home-launch'
import { HOME_MINIMAL_STANDARD, type HomeMinimalLayout } from './home-minimal'
import { getQuickSwitchInstances, resolveHomeLaunchInstance } from './home-utils'
import HomeGreeting from './HomeGreeting.vue'

const props = withDefaults(
    defineProps<{
        instances: GameInstance[]
        /** `stage` is the minimal Home; `widget` sits in the dashboard grid. */
        variant?: 'stage' | 'widget'
        /** The dashboard placement, whose options carry the greeting settings. */
        placement?: HomeWidgetPlacement | null
        dashboardSize?: HomeWidgetSize | null
        playerName?: string | null
        /** The minimal Home layout; only the stage reads it. */
        stage?: HomeMinimalLayout | null
    }>(),
    {
        variant: 'widget',
        placement: null,
        dashboardSize: null,
        playerName: null,
        stage: null,
    },
)

const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const { offline } = useNetworkStatus()
const launch = inject(HOME_LAUNCH_KEY, null)
const runtime = useOptionalHomeDashboardRuntime()

const messages = defineMessages({
    homeInstance: { id: 'app.home.launch.home-instance', defaultMessage: 'Home instance' },
    continuePlaying: {
        id: 'app.home.launch.continue',
        defaultMessage: 'Continue playing',
    },
    running: { id: 'app.home.launch.running', defaultMessage: 'Running' },
    notInstalled: { id: 'app.home.launch.not-installed', defaultMessage: 'Not installed' },
    switchInstance: { id: 'app.home.launch.switch', defaultMessage: 'Switch instance' },
    switchTo: { id: 'app.home.launch.switch-to', defaultMessage: 'Switch to {name}' },
    recent: { id: 'app.home.launch.recent', defaultMessage: 'Recently played' },
    recentShort: { id: 'app.home.launch.recent-short', defaultMessage: 'Recent' },
    allInstances: { id: 'app.home.launch.all-instances', defaultMessage: 'All instances' },
    totalPlayed: {
        id: 'app.home.launch.total-played',
        defaultMessage: '{duration} in total',
    },
    lastWorld: { id: 'app.home.launch.last-world', defaultMessage: 'Last in {name}' },
    hoursMinutes: { id: 'app.home.playtime.hours-minutes', defaultMessage: '{hours}h {minutes}m' },
    minutes: { id: 'app.home.playtime.minutes', defaultMessage: '{minutes}m' },
    chooseInstance: {
        id: 'app.home.minimal.choose-instance',
        defaultMessage: 'Choose instance',
    },
    chooseHomeInstance: {
        id: 'app.home.minimal.picker.title',
        defaultMessage: 'Choose a Home instance',
    },
    createInstance: {
        id: 'app.home.instances.create',
        defaultMessage: 'Create instance',
    },
    noInstances: {
        id: 'app.home.instances.empty',
        defaultMessage: 'No instances yet',
    },
    loading: {
        id: 'app.instance.loading',
        defaultMessage: 'Instance is loading...',
    },
    played: {
        id: 'app.instance.played',
        defaultMessage: 'Played {time}',
    },
    neverPlayed: {
        id: 'app.instance.never-played',
        defaultMessage: 'Never played',
    },
    offlineInstalledOnly: {
        id: 'app.instance.offline-installed-only',
        defaultMessage: 'Offline mode can only launch fully downloaded instances.',
    },
    freshHint: {
        id: 'app.home.launch.fresh-hint',
        defaultMessage:
            'Create one from scratch, import one from another launcher, or start from a modpack.',
    },
    createOrImport: {
        id: 'app.home.launch.create-or-import',
        defaultMessage: 'Create or import',
    },
    browseModpacks: { id: 'app.home.launch.browse-modpacks', defaultMessage: 'Browse modpacks' },
    chooseHint: {
        id: 'app.home.launch.choose-hint',
        defaultMessage: 'Pick the instance Home starts. You can change it any time.',
    },
    quickPick: { id: 'app.home.launch.quick-pick', defaultMessage: 'Quick pick' },
})

const resolved = computed(() =>
    resolveHomeLaunchInstance(props.instances, launch?.selectedInstanceId.value),
)
const target = computed(() => resolved.value?.instance ?? null)
const state = useHomeLaunchState(target, props.variant === 'stage' ? 'HomeMinimal' : 'HomeLaunch')
const phase = state.phase

/**
 * A one-row tile has room for the instance and its actions only; the hero
 * (two rows, or the minimal stage) adds the greeting and the details.
 */
const shape = computed(() => {
    if (props.variant === 'stage') return 'hero'
    const rows = Number(props.dashboardSize?.split('x')[1] ?? 2)
    return rows >= 2 ? 'hero' : 'strip'
})
const quickSwitch = computed(() => getQuickSwitchInstances(props.instances, target.value?.id, 3))
const instanceRoute = computed(() =>
    target.value ? `/instance/${encodeURIComponent(target.value.id)}` : '/library',
)
const showGameDirectory = computed(() => hasMultipleGameDirectories(props.instances))
const gameDirectory = computed(() =>
    target.value && showGameDirectory.value ? resolveGameDirectory(target.value) : null,
)
const artUrl = computed(() =>
    target.value ? getDisplayInstanceIcon(target.value.icon_path, target.value.loader).url : null,
)
/** The icon's own colour for the glow; null (accent colour) for grey or unreadable icons. */
const tint = useIconTint(artUrl)

function playedLabel(instance: GameInstance): string {
    if (!instance.last_played) return formatMessage(messages.neverPlayed)
    return formatMessage(messages.played, {
        time: formatRelativeTime(dayjs(instance.last_played).toISOString()),
    })
}
const versionLabel = computed(() => {
    if (!target.value) return ''
    const loader =
        target.value.loader === 'vanilla' ? 'Minecraft' : formatLoaderLabel(target.value.loader)
    return `${loader} ${target.value.game_version}`
})
const lastPlayed = computed(() => (target.value ? playedLabel(target.value) : ''))
/** The playtime the instance page shows, from the same two counters. */
const totalPlayed = computed(() => {
    const seconds =
        (target.value?.recent_time_played ?? 0) + (target.value?.submitted_time_played ?? 0)
    if (seconds < 60) return null
    const hours = Math.floor(seconds / 3600)
    const minutes = Math.floor((seconds % 3600) / 60)
    return formatMessage(messages.totalPlayed, {
        duration:
            hours > 0
                ? formatMessage(messages.hoursMinutes, { hours, minutes })
                : formatMessage(messages.minutes, { minutes }),
    })
})

/** The dashboard already keeps recent worlds; the minimal Home asks for them itself. */
const recentWorldsQuery = useQuery({
    queryKey: ['home', 'launch-recent-worlds'],
    queryFn: () => get_recent_worlds(8, ['normal', 'favorite']),
    enabled: computed(() => !runtime && shape.value === 'hero'),
    staleTime: 60_000,
})
const lastWorld = computed(() => {
    if (!target.value) return null
    const worlds = runtime ? runtime.recentWorlds.value : (recentWorldsQuery.data.value ?? [])
    return worlds.find((world) => world.instance_id === target.value?.id) ?? null
})
const statusLabel = computed(() => {
    if (phase.value === 'running') return formatMessage(messages.running)
    if (phase.value === 'installing') return formatMessage(commonMessages.installingLabel)
    if (phase.value === 'not-installed') return formatMessage(messages.notInstalled)
    return formatMessage(resolved.value?.chosen ? messages.homeInstance : messages.continuePlaying)
})
/** The minimal Home's layout; the dashboard card always shows everything. */
const stageLayout = computed(() =>
    props.variant === 'stage' ? (props.stage ?? HOME_MINIMAL_STANDARD) : null,
)
const show = computed(() => stageLayout.value?.show ?? HOME_MINIMAL_STANDARD.show)
/** Without a target the card explains what to do next, so it keeps its surface. */
const plain = computed(() => stageLayout.value?.surface === 'plain' && target.value !== null)
const fresh = computed(() => props.instances.length === 0)
/**
 * Without a card, or with only the name left beside the icon, a full-width
 * row would leave a lopsided gap; the content then takes its own width and
 * sits in the middle of the page.
 */
const fitContent = computed(
    () =>
        !!stageLayout.value &&
        target.value !== null &&
        stageLayout.value.arrangement === 'row' &&
        (plain.value || (!show.value.meta && !show.value.recent)),
)
const iconSize = computed(() => {
    const layout = stageLayout.value
    if (layout) {
        if (layout.arrangement === 'stacked') return layout.density === 'compact' ? '88px' : '112px'
        return layout.density === 'compact' ? '80px' : '104px'
    }
    return shape.value === 'hero' ? '88px' : '64px'
})
</script>

<template>
    <div
        class="home-launch"
        :data-variant="variant"
        :data-shape="shape"
        :data-arrangement="stageLayout?.arrangement"
        :data-density="stageLayout?.density"
        :data-surface="stageLayout ? (plain ? 'plain' : 'card') : undefined"
        :data-fit="fitContent ? '' : undefined"
    >
        <section
            class="home-launch-card"
            :class="[
                variant !== 'stage'
                    ? 'home-surface-bare'
                    : plain
                      ? 'home-legible'
                      : 'home-surface home-surface-hero',
                { 'is-empty': !target, 'is-fresh': !target && fresh },
            ]"
            :data-phase="target ? phase : undefined"
            :aria-label="target?.name"
        >
            <span
                v-if="target && !plain"
                class="home-launch-glow"
                :class="{ 'is-tinted': tint }"
                :style="tint ? { '--home-launch-tint': tint } : undefined"
                aria-hidden="true"
            />

            <HomeGreeting
                v-if="variant === 'widget' && shape === 'hero'"
                class="home-launch-greeting"
                variant="hero"
                :player-name="playerName"
                :greeting-mode="placement?.options?.greetingMode"
                :greeting-text="placement?.options?.greetingText"
                :greeting-font="placement?.options?.greetingFont"
                :greeting-font-size="placement?.options?.greetingFontSize"
            />

            <template v-if="target">
                <router-link
                    :to="instanceRoute"
                    class="home-launch-icon"
                    tabindex="-1"
                    aria-hidden="true"
                >
                    <InstanceIcon
                        :icon-path="target.icon_path"
                        :instance-id="target.id"
                        :loader="target.loader"
                        :size="iconSize"
                    />
                </router-link>

                <div class="home-launch-copy">
                    <span v-if="show.status" class="home-launch-status">
                        <span class="home-launch-status-dot" aria-hidden="true" />
                        <span class="truncate">{{ statusLabel }}</span>
                    </span>
                    <router-link :to="instanceRoute" class="home-launch-name">
                        {{ target.name }}
                    </router-link>
                    <div v-if="show.meta" class="home-launch-meta">
                        <span class="home-chip">
                            <GameIcon aria-hidden="true" />
                            <span class="truncate">{{ versionLabel }}</span>
                        </span>
                        <span class="home-chip">
                            <TimerIcon aria-hidden="true" />
                            <span class="truncate">{{ lastPlayed }}</span>
                        </span>
                        <span v-if="totalPlayed && shape === 'hero'" class="home-chip">
                            <ClockIcon aria-hidden="true" />
                            <span class="truncate">{{ totalPlayed }}</span>
                        </span>
                        <GameDirectoryBadge v-if="gameDirectory" :directory="gameDirectory" />
                    </div>
                    <span
                        v-if="show.meta && lastWorld && shape === 'hero'"
                        class="home-launch-detail"
                    >
                        <ServerIcon v-if="lastWorld.type === 'server'" aria-hidden="true" />
                        <GameIcon v-else aria-hidden="true" />
                        <span class="truncate">{{
                            formatMessage(messages.lastWorld, { name: lastWorld.name })
                        }}</span>
                    </span>
                </div>

                <div class="home-launch-footer">
                    <div class="home-launch-actions">
                        <Button
                            v-if="phase === 'error'"
                            size="xl"
                            class="home-launch-primary"
                            @click="state.retry()"
                        >
                            <span class="truncate">{{
                                formatMessage(commonMessages.retryButton)
                            }}</span>
                        </Button>
                        <Button
                            v-else-if="phase === 'running'"
                            type="colored"
                            color="red"
                            size="xl"
                            class="home-launch-primary"
                            @click="state.stop()"
                        >
                            <StopCircleIcon aria-hidden="true" />
                            <span class="truncate">{{
                                formatMessage(commonMessages.stopButton)
                            }}</span>
                        </Button>
                        <Button
                            v-else-if="phase === 'busy' || phase === 'installing'"
                            size="xl"
                            class="home-launch-primary"
                            disabled
                        >
                            <SpinnerIcon class="animate-spin" aria-hidden="true" />
                            <span class="truncate">{{
                                formatMessage(
                                    phase === 'installing'
                                        ? commonMessages.installingLabel
                                        : messages.loading,
                                )
                            }}</span>
                        </Button>
                        <Button
                            v-else-if="phase === 'ready'"
                            type="colored"
                            color="brand"
                            size="xl"
                            class="home-launch-primary"
                            @click="state.play()"
                        >
                            <PlayIcon aria-hidden="true" />
                            <span class="truncate">{{
                                formatMessage(commonMessages.playButton)
                            }}</span>
                        </Button>
                        <Button
                            v-else
                            v-tooltip="
                                offline ? formatMessage(messages.offlineInstalledOnly) : undefined
                            "
                            type="colored"
                            color="brand"
                            size="xl"
                            class="home-launch-primary"
                            :disabled="offline"
                            @click="state.install()"
                        >
                            <DownloadIcon aria-hidden="true" />
                            <span class="truncate">{{
                                formatMessage(commonMessages.installButton)
                            }}</span>
                        </Button>
                        <button
                            v-if="launch && show.switch"
                            v-tooltip="
                                shape === 'strip'
                                    ? formatMessage(messages.switchInstance)
                                    : undefined
                            "
                            type="button"
                            class="home-launch-switch"
                            :aria-label="formatMessage(messages.switchInstance)"
                            @click="launch.choose()"
                        >
                            <ArrowLeftRightIcon aria-hidden="true" />
                            <span class="home-launch-switch-label">{{
                                formatMessage(messages.switchInstance)
                            }}</span>
                        </button>
                    </div>

                    <div
                        v-if="variant === 'widget' && launch && quickSwitch.length > 0"
                        class="home-launch-switcher"
                        role="group"
                        :aria-label="formatMessage(messages.recent)"
                    >
                        <span class="home-launch-switcher-label" aria-hidden="true">{{
                            formatMessage(messages.recentShort)
                        }}</span>
                        <button
                            v-for="instance in quickSwitch"
                            :key="instance.id"
                            v-tooltip="formatMessage(messages.switchTo, { name: instance.name })"
                            type="button"
                            class="home-launch-switch-avatar"
                            :aria-label="formatMessage(messages.switchTo, { name: instance.name })"
                            @click="launch.select(instance)"
                        >
                            <InstanceIcon
                                :icon-path="instance.icon_path"
                                :instance-id="instance.id"
                                :loader="instance.loader"
                                size="36px"
                            />
                        </button>
                    </div>
                </div>
            </template>

            <template v-else>
                <div class="home-launch-empty-icon" aria-hidden="true">
                    <PackagePlusIcon v-if="fresh" />
                    <ListIcon v-else />
                </div>
                <div class="home-launch-copy">
                    <h2 class="home-launch-name">
                        {{
                            formatMessage(
                                fresh ? messages.noInstances : messages.chooseHomeInstance,
                            )
                        }}
                    </h2>
                    <p v-if="shape === 'hero'" class="home-launch-hint">
                        {{ formatMessage(fresh ? messages.freshHint : messages.chooseHint) }}
                    </p>
                </div>
                <div class="home-launch-footer">
                    <div class="home-launch-actions">
                        <Button
                            type="colored"
                            color="brand"
                            size="xl"
                            class="home-launch-primary"
                            @click="fresh ? launch?.create() : launch?.choose()"
                        >
                            <PlusIcon v-if="fresh" aria-hidden="true" />
                            <ListIcon v-else aria-hidden="true" />
                            <span class="truncate">{{
                                formatMessage(
                                    fresh ? messages.createOrImport : messages.chooseInstance,
                                )
                            }}</span>
                        </Button>
                        <router-link
                            v-if="fresh"
                            v-tooltip="
                                shape === 'strip'
                                    ? formatMessage(messages.browseModpacks)
                                    : undefined
                            "
                            to="/browse/modpack"
                            class="home-launch-switch"
                            :aria-label="formatMessage(messages.browseModpacks)"
                        >
                            <CompassIcon aria-hidden="true" />
                            <span class="home-launch-switch-label">{{
                                formatMessage(messages.browseModpacks)
                            }}</span>
                        </router-link>
                    </div>
                    <div
                        v-if="!fresh && launch && shape === 'hero' && quickSwitch.length > 0"
                        class="home-launch-switcher"
                        role="group"
                        :aria-label="formatMessage(messages.quickPick)"
                    >
                        <span class="home-launch-switcher-label" aria-hidden="true">{{
                            formatMessage(messages.quickPick)
                        }}</span>
                        <button
                            v-for="instance in quickSwitch"
                            :key="instance.id"
                            v-tooltip="instance.name"
                            type="button"
                            class="home-launch-switch-avatar"
                            :aria-label="formatMessage(messages.switchTo, { name: instance.name })"
                            @click="launch.select(instance)"
                        >
                            <InstanceIcon
                                :icon-path="instance.icon_path"
                                :instance-id="instance.id"
                                :loader="instance.loader"
                                size="36px"
                            />
                        </button>
                    </div>
                </div>
            </template>
        </section>

        <nav
            v-if="variant === 'stage' && show.recent && launch && target && quickSwitch.length > 0"
            class="home-launch-recent"
            :aria-label="formatMessage(messages.recent)"
        >
            <div class="home-launch-recent-header">
                <span class="home-launch-recent-label home-legible">{{
                    formatMessage(messages.recent)
                }}</span>
                <button
                    type="button"
                    class="home-launch-recent-all home-legible"
                    @click="launch.choose()"
                >
                    <span>{{ formatMessage(messages.allInstances) }}</span>
                    <RightArrowIcon aria-hidden="true" />
                </button>
            </div>
            <ul class="home-launch-recent-list">
                <li v-for="instance in quickSwitch" :key="instance.id" class="min-w-0">
                    <button
                        v-tooltip="formatMessage(messages.switchTo, { name: instance.name })"
                        type="button"
                        class="home-launch-recent-card home-surface"
                        @click="launch.select(instance)"
                    >
                        <InstanceIcon
                            :icon-path="instance.icon_path"
                            :instance-id="instance.id"
                            :loader="instance.loader"
                            size="40px"
                        />
                        <span class="flex min-w-0 flex-col gap-0.5 text-left">
                            <span class="home-launch-recent-name">{{ instance.name }}</span>
                            <span class="home-launch-recent-meta">{{ playedLabel(instance) }}</span>
                        </span>
                    </button>
                </li>
            </ul>
        </nav>
    </div>
</template>

<style scoped>
.home-launch {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 1rem;
    container-type: inline-size;
}

.home-launch[data-variant='widget'],
.home-launch[data-variant='widget'] .home-launch-card {
    height: 100%;
}

.home-launch-card {
    position: relative;
    isolation: isolate;
    display: grid;
    min-width: 0;
    align-items: center;
    overflow: hidden;
}

.home-launch-card > :not(.home-launch-glow) {
    position: relative;
    z-index: 1;
}

/*
 * A soft glow from the card's right edge in the colour of the instance's icon
 * (the accent colour when the icon is grey or cannot be read). It is a plain
 * gradient, not the blurred icon, so it never smears a muddy shape behind the
 * text, and it stays on the side the copy does not use. Its strength follows
 * the widget fill: at 0% only a hint remains over a background image.
 */
.home-launch-glow {
    --home-launch-tint: var(--color-brand);
    --home-launch-glow-mix: calc(var(--home-widget-bg-alpha, 1) * 12% + 8%);
    position: absolute;
    z-index: 0;
    inset: 0;
    background: radial-gradient(
        55% 90% at 100% 35%,
        color-mix(in srgb, var(--home-launch-tint) var(--home-launch-glow-mix), transparent),
        transparent 72%
    );
    pointer-events: none;
}

/* An icon's own colour is stronger than the accent, so it gets a little more room. */
.home-launch-glow.is-tinted {
    --home-launch-glow-mix: calc(var(--home-widget-bg-alpha, 1) * 16% + 10%);
}

/* Hero: greeting on top, instance in the middle, actions and switcher below. */
.home-launch[data-shape='hero'] .home-launch-card {
    grid-template-columns: auto minmax(0, 1fr);
    grid-template-rows: auto 1fr auto;
    grid-template-areas:
        'greeting greeting'
        'icon copy'
        'footer footer';
    column-gap: 1.25rem;
    row-gap: 1rem;
    padding: 1.25rem 1.5rem 1.25rem;
}

.home-launch[data-variant='stage'] .home-launch-card {
    grid-template-rows: 1fr auto;
    grid-template-areas:
        'icon copy'
        'footer footer';
    row-gap: 1.5rem;
    column-gap: 1.75rem;
    padding: 2rem 2.25rem 1.75rem;
}

/* Strip: one row for icon, name and actions. */
.home-launch[data-shape='strip'] .home-launch-card {
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    grid-template-areas: 'icon copy actions switcher';
    column-gap: 1rem;
    padding: 1rem 1.25rem;
}

.home-launch-greeting {
    grid-area: greeting;
    align-self: start;
}

.home-launch-greeting :deep(h1) {
    line-height: 1.25;
}

.home-launch-icon {
    grid-area: icon;
    display: flex;
    border-radius: var(--radius-lg);
    transition: transform 160ms ease;
}

.home-launch-icon :deep(img),
.home-launch-icon :deep(.avatar) {
    border-radius: var(--radius-lg);
    box-shadow:
        0 6px 18px color-mix(in srgb, var(--surface-1-opaque) 45%, transparent),
        var(--home-widget-shadow);
}

.home-launch-icon:hover {
    transform: translateY(-1px) scale(1.02);
}

/*
 * The empty card's picture: the next step's icon on an accent tile, with two
 * faint tiles fanned out behind it, drawn from the widget tokens only.
 */
.home-launch-empty-icon {
    @apply relative flex size-16 items-center justify-center rounded-[var(--radius-lg)] text-brand;
    grid-area: icon;
    background: color-mix(in srgb, var(--color-brand) 16%, var(--home-widget-fill, transparent));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-brand) 30%, transparent);
}

.home-launch-empty-icon::before,
.home-launch-empty-icon::after {
    position: absolute;
    z-index: -1;
    inset: 0;
    border-radius: inherit;
    background: var(--home-widget-tint-strong);
    content: '';
}

.home-launch-empty-icon::before {
    transform: translate(-0.5rem, 0.25rem) rotate(-10deg);
}

.home-launch-empty-icon::after {
    transform: translate(0.5rem, 0.25rem) rotate(9deg);
}

.home-launch-empty-icon svg {
    @apply size-7;
}

.home-launch[data-shape='strip'] .home-launch-empty-icon {
    @apply size-12;
}

.home-launch[data-shape='strip'] .home-launch-empty-icon svg {
    @apply size-5;
}

.home-launch-hint {
    @apply m-0 max-w-[34rem] text-sm leading-snug text-[var(--color-text-tertiary)];
}

.home-launch-copy {
    @apply flex min-w-0 flex-col gap-1.5;
    grid-area: copy;
}

.home-launch-status {
    @apply flex min-w-0 items-center gap-1.5 text-xs font-bold text-[var(--color-text-tertiary)];
}

.home-launch-status-dot {
    @apply size-2 shrink-0 rounded-full bg-brand;
}

.home-launch-card[data-phase='running'] .home-launch-status {
    @apply text-brand-green;
}

.home-launch-card[data-phase='running'] .home-launch-status-dot {
    @apply bg-brand-green;
    animation: home-launch-pulse 1.6s ease-in-out infinite;
}

.home-launch-card:is([data-phase='not-installed'], [data-phase='installing'])
    .home-launch-status-dot {
    @apply bg-brand-orange;
}

.home-launch-name {
    @apply m-0 min-w-0 truncate text-xl font-extrabold leading-tight text-[var(--color-text-primary)] no-underline;
}

/* The hero has the height for a second line before it cuts a long name. */
.home-launch[data-shape='hero'] .home-launch-name {
    @apply text-2xl;
    display: -webkit-box;
    white-space: normal;
    overflow-wrap: anywhere;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
}

.home-launch[data-variant='stage'] .home-launch-name {
    @apply text-[1.75rem];
}

a.home-launch-name:hover {
    @apply underline;
}

a.home-launch-name:focus-visible,
.home-launch-switch:focus-visible,
.home-launch-switch-avatar:focus-visible,
.home-launch-recent-all:focus-visible,
.home-launch-recent-card:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px var(--color-brand-shadow);
}

.home-launch-meta {
    @apply flex min-w-0 flex-wrap items-center gap-1.5;
}

.home-launch-detail {
    @apply flex min-w-0 items-center gap-1.5 text-xs font-semibold text-[var(--color-text-tertiary)];
}

.home-launch-detail svg {
    @apply size-3.5 shrink-0;
}

.home-launch-footer {
    @apply flex min-w-0 items-center justify-between gap-3;
    grid-area: footer;
}

/* The strip lays the actions and the switcher out on its own grid row. */
.home-launch[data-shape='strip'] .home-launch-footer {
    display: contents;
}

.home-launch-actions {
    @apply flex min-w-0 items-center gap-2;
    grid-area: actions;
}

.home-launch-primary {
    min-width: 9.5rem;
    max-width: 14rem;
    justify-content: center;
}

.home-launch[data-variant='stage'] .home-launch-primary {
    min-width: 12rem;
}

/* The switch action names itself; in the narrow strip the label folds into a tooltip. */
.home-launch-switch {
    @apply flex h-12 shrink-0 cursor-pointer items-center gap-2 rounded-full border-0 px-4 text-sm font-semibold text-[var(--color-text-primary)];
    /* A control, not a surface: it keeps a solid fill at any widget opacity. */
    background: color-mix(in srgb, var(--surface-4-opaque) 85%, transparent);
    text-shadow: none;
    transition:
        background-color 120ms ease,
        transform 120ms ease;
}

.home-launch-switch svg {
    @apply size-4 shrink-0;
}

a.home-launch-switch {
    @apply no-underline;
}

.home-launch-switch:hover {
    background: var(--surface-5-opaque);
}

.home-launch-switch:active {
    transform: scale(0.97);
}

.home-launch[data-shape='strip'] .home-launch-switch {
    @apply w-12 justify-center px-0;
}

.home-launch[data-shape='strip'] .home-launch-switch-label {
    @apply sr-only;
}

.home-launch-switcher {
    @apply flex min-w-0 items-center justify-end gap-1.5;
    grid-area: switcher;
}

.home-launch[data-shape='strip'] .home-launch-switcher {
    @apply pl-3;
    border-left: 1px solid var(--home-widget-edge);
}

.home-launch-switcher-label {
    @apply mr-1 text-xs font-bold text-[var(--color-text-tertiary)];
}

.home-launch[data-shape='strip'] .home-launch-switcher-label {
    display: none;
}

.home-launch-switch-avatar {
    @apply flex cursor-pointer rounded-[var(--radius-md)] border-0 bg-transparent p-0.5 opacity-75;
    transition:
        opacity 120ms ease,
        transform 120ms ease;
}

.home-launch-switch-avatar:hover,
.home-launch-switch-avatar:focus-visible {
    @apply opacity-100;
    transform: translateY(-1px);
}

.home-launch-recent {
    @apply flex min-w-0 flex-col gap-2.5;
    container-type: inline-size;
}

.home-launch-recent-header {
    @apply flex min-w-0 items-center justify-between gap-3 px-1;
}

.home-launch-recent-label {
    @apply text-xs font-bold text-[var(--color-text-tertiary)];
}

.home-launch-recent-all {
    @apply flex cursor-pointer items-center gap-1 rounded-full border-0 bg-transparent px-2 py-1 text-xs font-semibold text-[var(--color-text-default)];
    transition: color 120ms ease;
}

.home-launch-recent-all svg {
    @apply size-3.5;
}

.home-launch-recent-all:hover {
    @apply text-[var(--color-text-primary)];
}

/* Up to three cards, in columns that line up with the card above them. */
.home-launch-recent-list {
    @apply m-0 grid list-none gap-3 p-0;
    grid-template-columns: repeat(3, minmax(0, 1fr));
}

.home-launch-recent-card {
    @apply flex w-full min-w-0 cursor-pointer items-center gap-3 px-3 py-2.5 text-[var(--color-text-default)];
    transition:
        border-color 120ms ease,
        transform 120ms ease;
}

.home-launch-recent-card :deep(img),
.home-launch-recent-card :deep(.avatar) {
    border-radius: var(--radius-md);
}

.home-launch-recent-card:hover {
    border-color: color-mix(in srgb, var(--color-brand) 55%, transparent);
    transform: translateY(-1px);
}

.home-launch-recent-name {
    @apply min-w-0 truncate text-sm font-bold text-[var(--color-text-primary)];
}

.home-launch-recent-meta {
    @apply min-w-0 truncate text-xs text-[var(--color-text-tertiary)];
}

@container (max-width: 34rem) {
    .home-launch-recent-list {
        grid-template-columns: minmax(0, 1fr);
    }
}

/*
 * Minimal Home layouts. `stacked` puts the icon, the name and the button in
 * one centred column; `plain` drops the card so only the content sits on the
 * page; `compact` tightens the spacing. Hidden parts are not rendered at all,
 * so no gap is left behind for them.
 */
.home-launch[data-variant='stage'][data-arrangement='stacked'] {
    align-items: center;
}

.home-launch[data-variant='stage'][data-arrangement='stacked'] .home-launch-card {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto;
    grid-template-areas:
        'icon'
        'copy'
        'footer';
    justify-items: center;
    row-gap: 1.25rem;
    min-width: min(100%, 24rem);
    text-align: center;
}

.home-launch[data-variant='stage'][data-arrangement='stacked'] .home-launch-recent {
    width: 100%;
    max-width: 40rem;
}

.home-launch[data-variant='stage'][data-arrangement='stacked'] .home-launch-copy {
    align-items: center;
}

.home-launch[data-variant='stage'][data-arrangement='stacked'] .home-launch-meta,
.home-launch[data-variant='stage'][data-arrangement='stacked'] .home-launch-footer,
.home-launch[data-variant='stage'][data-arrangement='stacked'] .home-launch-actions {
    justify-content: center;
}

.home-launch[data-variant='stage'][data-arrangement='stacked'] .home-launch-name {
    text-align: center;
}

.home-launch[data-variant='stage'][data-surface='plain'] .home-launch-card {
    padding: 0;
    overflow: visible;
}

.home-launch[data-variant='stage'][data-density='compact'] .home-launch-card {
    row-gap: 1rem;
    column-gap: 1.25rem;
}

.home-launch[data-variant='stage'][data-density='compact'][data-surface='card'] .home-launch-card {
    padding: 1.25rem 1.5rem;
}

.home-launch[data-variant='stage'][data-density='compact'] .home-launch-name {
    @apply text-[1.375rem];
}

.home-launch[data-variant='stage'][data-density='compact'] .home-launch-primary {
    min-width: 10rem;
}

.home-launch[data-variant='stage'][data-density='compact'] {
    gap: 0.75rem;
}

.home-launch[data-variant='stage'][data-fit] {
    align-items: center;
}

.home-launch[data-variant='stage'][data-fit] .home-launch-card {
    width: fit-content;
    min-width: min(100%, 22rem);
    max-width: 100%;
}

/* A narrower strip keeps two quick switches and drops the second chip. */
@container (max-width: 40rem) {
    .home-launch[data-shape='strip'] .home-launch-meta > :not(:first-child) {
        display: none;
    }

    .home-launch[data-shape='strip'] .home-launch-name {
        @apply text-lg;
    }

    .home-launch[data-shape='strip'] .home-launch-primary {
        min-width: 7.5rem;
    }

    .home-launch[data-shape='strip'] .home-launch-switch-avatar:nth-child(n + 4) {
        display: none;
    }
}

/* Too narrow for the strip's switcher: the button and the picker remain. */
@container (max-width: 31rem) {
    .home-launch[data-shape='strip'] .home-launch-switcher {
        display: none;
    }

    .home-launch[data-shape='strip'] .home-launch-card {
        grid-template-columns: auto minmax(0, 1fr) auto;
        grid-template-areas: 'icon copy actions';
    }
}

/*
 * A narrow hero keeps one footer row: the play button takes the room, the
 * switch button folds to its icon, and the quick switches follow it after a
 * divider, as in the strip, instead of dropping to a row of their own.
 */
@container (max-width: 34rem) {
    .home-launch[data-shape='hero'] .home-launch-card {
        padding: 1.125rem 1.25rem;
    }

    .home-launch[data-shape='hero'] .home-launch-actions {
        flex: 1 1 auto;
    }

    .home-launch[data-shape='hero'] .home-launch-primary {
        min-width: 0;
        max-width: none;
        flex: 1 1 auto;
    }

    .home-launch[data-variant='widget'][data-shape='hero'] .home-launch-switch {
        @apply w-12 justify-center px-0;
    }

    .home-launch[data-variant='widget'][data-shape='hero'] .home-launch-switch-label,
    .home-launch[data-shape='hero'] .home-launch-switcher-label {
        @apply sr-only;
    }

    .home-launch[data-shape='hero'] .home-launch-switcher {
        @apply pl-3;
        border-left: 1px solid var(--home-widget-edge);
    }

    .home-launch[data-shape='hero'] .home-launch-icon :deep(.avatar),
    .home-launch[data-shape='hero'] .home-launch-icon :deep(img) {
        width: 64px !important;
        height: 64px !important;
    }
}

/* Very narrow: the quick switches get a labelled row of their own under the button. */
@container (max-width: 22rem) {
    .home-launch[data-shape='hero'] .home-launch-footer {
        flex-direction: column;
        align-items: stretch;
    }

    .home-launch[data-shape='hero'] .home-launch-switcher {
        justify-content: flex-start;
        padding-left: 0;
        border-left: 0;
    }

    .home-launch[data-shape='hero'] .home-launch-switcher-label {
        position: static;
        width: auto;
        height: auto;
        margin: 0 0.25rem 0 0;
        overflow: visible;
        clip: auto;
        white-space: nowrap;
    }
}

/* One column: the strip stacks too. */
@container (max-width: 24rem) {
    .home-launch[data-shape='strip'] .home-launch-card {
        grid-template-columns: auto minmax(0, 1fr);
        grid-template-areas:
            'icon copy'
            'actions actions';
        row-gap: 0.75rem;
        padding: 0.875rem 1rem;
    }

    .home-launch[data-shape='strip'] .home-launch-icon :deep(.avatar),
    .home-launch[data-shape='strip'] .home-launch-icon :deep(img) {
        width: 44px !important;
        height: 44px !important;
    }

    .home-launch[data-shape='strip'] .home-launch-primary {
        max-width: none;
        flex: 1 1 auto;
    }
}

@keyframes home-launch-pulse {
    50% {
        opacity: 0.35;
    }
}

@media (prefers-reduced-motion: reduce) {
    .home-launch-card[data-phase='running'] .home-launch-status-dot {
        animation: none;
    }

    .home-launch-icon,
    .home-launch-icon:hover,
    .home-launch-switch,
    .home-launch-switch:active,
    .home-launch-switch-avatar,
    .home-launch-switch-avatar:hover,
    .home-launch-recent-card,
    .home-launch-recent-card:hover {
        transform: none;
        transition: none;
    }
}
</style>
