<script setup lang="ts">
import './home-widget.css'

import { BoxIcon, GameIcon, PlayIcon, ServerIcon } from '@modrinth/assets'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

import type { HomeWidgetKind, HomeWidgetSize, HomeWidgetTarget } from './home-dashboard'
import {
    createHomeWidget,
    homeWidgetNeedsTarget,
    homeWidgetPreviewSize,
} from './home-widget-catalog'
import HomeCalendar from './HomeCalendar.vue'
import HomeGreeting from './HomeGreeting.vue'
import HomeLaunchCard from './HomeLaunchCard.vue'
import HomePinnedInstances from './HomePinnedInstances.vue'
import HomePinnedServers from './HomePinnedServers.vue'
import HomePinnedWorlds from './HomePinnedWorlds.vue'
import HomeRecentWorlds from './HomeRecentWorlds.vue'
import HomeShortcutWidget from './HomeShortcutWidget.vue'

/**
 * A widget as it will look on the Home: the real component, fed the same data
 * the dashboard has, drawn at its size on the narrowest dashboard column and
 * scaled, keeping its proportions, to fill the box it is put in (the parent
 * sets the box and its margin). It cannot be clicked or focused.
 * A shortcut without anything to point at yet is drawn as a sketch.
 */
const props = withDefaults(
    defineProps<{
        kind: HomeWidgetKind
        size: HomeWidgetSize
        instances: GameInstance[]
        playerName?: string | null
        target?: HomeWidgetTarget | null
    }>(),
    { playerName: null, target: null },
)

const natural = computed(() => homeWidgetPreviewSize(props.size))
const box = ref<HTMLElement | null>(null)
const boxSize = ref({ width: 0, height: 0 })
const scale = computed(() =>
    Math.min(
        boxSize.value.width / natural.value.width,
        boxSize.value.height / natural.value.height,
    ),
)

let observer: ResizeObserver | null = null
onMounted(() => {
    observer = new ResizeObserver(([entry]) => {
        boxSize.value = { width: entry.contentRect.width, height: entry.contentRect.height }
    })
    if (box.value) observer.observe(box.value)
})
onBeforeUnmount(() => observer?.disconnect())
const placement = computed(() => createPreviewPlacement())
const sketch = computed(() => homeWidgetNeedsTarget(props.kind) && !props.target)
const sketchIcon = computed(() =>
    props.kind === 'server' ? ServerIcon : props.kind === 'world' ? GameIcon : BoxIcon,
)

function createPreviewPlacement() {
    return {
        ...createHomeWidget(props.kind, props.size, props.target),
        id: `preview-${props.kind}`,
    }
}
</script>

<template>
    <div
        ref="box"
        class="home-widget-preview"
        :data-kind="kind"
        :data-size="size"
        inert
        aria-hidden="true"
    >
        <div
            class="home-widget-preview-scaled"
            :style="{
                width: `${natural.width * scale}px`,
                height: `${natural.height * scale}px`,
                visibility: scale > 0 ? undefined : 'hidden',
            }"
        >
            <article
                class="home-widget-preview-tile home-surface"
                :class="{ 'home-surface-hero': kind === 'launch' }"
                :data-widget-kind="kind"
                :style="{
                    width: `${natural.width}px`,
                    height: `${natural.height}px`,
                    transform: `scale(${scale})`,
                }"
            >
                <div v-if="sketch" class="home-widget-preview-sketch" :data-size="size">
                    <span class="home-widget-preview-sketch-icon">
                        <component :is="sketchIcon" />
                    </span>
                    <span class="home-widget-preview-sketch-lines">
                        <span />
                        <span />
                    </span>
                    <span class="home-widget-preview-sketch-play"><PlayIcon /></span>
                </div>
                <div v-else class="home-widget-preview-content">
                    <HomeLaunchCard
                        v-if="kind === 'launch'"
                        :instances="instances"
                        :placement="placement"
                        :dashboard-size="size"
                        :player-name="playerName"
                        variant="widget"
                    />
                    <HomeGreeting
                        v-else-if="kind === 'greeting'"
                        :player-name="playerName"
                        :dashboard-size="size"
                    />
                    <HomeRecentWorlds
                        v-else-if="kind === 'recent'"
                        :instances="instances"
                        :dashboard-size="size"
                        dashboard
                    />
                    <HomeCalendar
                        v-else-if="kind === 'calendar'"
                        :instances="instances"
                        :dashboard-size="size"
                    />
                    <HomePinnedInstances
                        v-else-if="kind === 'pinned-instances'"
                        :instances="instances"
                        :dashboard-size="size"
                        dashboard
                    />
                    <HomePinnedWorlds
                        v-else-if="kind === 'pinned-worlds'"
                        :instances="instances"
                        :dashboard-size="size"
                        dashboard
                    />
                    <HomePinnedServers
                        v-else-if="kind === 'pinned-servers'"
                        :instances="instances"
                        :dashboard-size="size"
                        dashboard
                    />
                    <HomeShortcutWidget
                        v-else
                        :placement="placement"
                        :instances="instances"
                        :dashboard-size="size"
                    />
                </div>
            </article>
        </div>
    </div>
</template>

<style scoped>
.home-widget-preview {
    display: flex;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    align-items: center;
    justify-content: center;
    pointer-events: none;
    user-select: none;
}

.home-widget-preview-scaled {
    position: relative;
    flex: none;
}

.home-widget-preview-tile {
    position: absolute;
    top: 0;
    left: 0;
    display: flex;
    overflow: hidden;
    flex-direction: column;
    transform-origin: 0 0;
}

.home-widget-preview-content {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    padding: 1rem;
}

.home-widget-preview-tile:is(
        [data-widget-kind='launch'],
        [data-widget-kind='instance'],
        [data-widget-kind='world'],
        [data-widget-kind='server']
    )
    .home-widget-preview-content {
    padding: 0;
}

.home-widget-preview-content > :deep(*) {
    height: 100%;
    min-height: 0;
}

/* A shortcut with no target yet: its shape, drawn in the widget's own tones. */
.home-widget-preview-sketch {
    @apply grid h-full gap-x-3 gap-y-2 p-4;
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-rows: minmax(0, 1fr) auto;
    grid-template-areas:
        'icon . .'
        'lines lines play';
    background: radial-gradient(
        120% 130% at 0% 0%,
        color-mix(in srgb, var(--color-brand) 16%, transparent),
        transparent 62%
    );
}

.home-widget-preview-sketch[data-size='2x1'] {
    @apply p-5;
    grid-template-rows: minmax(0, 1fr);
    grid-template-areas: 'icon lines play';
}

.home-widget-preview-sketch-icon {
    grid-area: icon;
    @apply flex size-16 items-center justify-center self-start text-[var(--color-text-tertiary)];
    border: 1.5px dashed var(--home-widget-edge);
    border-radius: var(--radius-lg);
}

.home-widget-preview-sketch[data-size='2x1'] .home-widget-preview-sketch-icon {
    @apply size-[6.5rem] self-center;
}

.home-widget-preview-sketch-icon :deep(svg) {
    @apply size-7;
}

.home-widget-preview-sketch-lines {
    grid-area: lines;
    @apply flex flex-col justify-end gap-2 self-end;
}

.home-widget-preview-sketch[data-size='2x1'] .home-widget-preview-sketch-lines {
    @apply self-center;
}

.home-widget-preview-sketch-lines > span {
    @apply block h-3 w-3/4 rounded-full;
    background: var(--home-widget-tint-strong);
}

.home-widget-preview-sketch-lines > span + span {
    @apply h-2.5 w-1/2;
    background: var(--home-widget-tint);
}

.home-widget-preview-sketch-play {
    grid-area: play;
    @apply flex size-12 items-center justify-center self-end rounded-full bg-brand text-[var(--color-accent-contrast)];
}

.home-widget-preview-sketch[data-size='2x1'] .home-widget-preview-sketch-play {
    @apply self-center;
}

.home-widget-preview-sketch-play :deep(svg) {
    @apply size-5;
}
</style>
