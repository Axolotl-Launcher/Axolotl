<script setup lang="ts">
import { ChartIcon, GameIcon, TrendingUpIcon, UpdatedIcon } from '@modrinth/assets'
import { Button, commonMessages, defineMessages, useFormatDateTime, useVIntl } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, onUnmounted } from 'vue'

import { process_listener } from '@/helpers/events'
import { type DailyPlaytime, get_daily_playtime } from '@/helpers/instance'

import { toDateKey } from './home-utils'
import HomeWidgetFrame from './HomeWidgetFrame.vue'

const { formatMessage, locale } = useVIntl()
const formatWeekday = useFormatDateTime({ weekday: 'narrow' })
const formatDayTitle = useFormatDateTime({ weekday: 'long', month: 'long', day: 'numeric' })

const messages = defineMessages({
    insights: { id: 'app.home.insights.title', defaultMessage: 'Play insights' },
    thisWeekLabel: { id: 'app.home.insights.this-week-label', defaultMessage: 'This week' },
    vsLastWeek: { id: 'app.home.insights.vs-last-week', defaultMessage: 'vs last week' },
    hoursShort: { id: 'app.home.playtime.hours-short', defaultMessage: '{hours}h' },
    thisWeek: { id: 'app.home.insights.this-week', defaultMessage: 'This week: {duration}' },
    thisWeekMore: {
        id: 'app.home.insights.this-week-more',
        defaultMessage: 'This week: {duration} ({percent}% more than last week)',
    },
    thisWeekLess: {
        id: 'app.home.insights.this-week-less',
        defaultMessage: 'This week: {duration} ({percent}% less than last week)',
    },
    thisWeekSame: {
        id: 'app.home.insights.this-week-same',
        defaultMessage: 'This week: {duration} (same as last week)',
    },
    streak: {
        id: 'app.home.insights.streak',
        defaultMessage:
            '{days, plural, one {# day played in a row} other {# days played in a row}}',
    },
    weekTop: { id: 'app.home.insights.week-top', defaultMessage: 'Most played: {name}' },
    emptyTitle: { id: 'app.home.insights.empty-title', defaultMessage: 'No playtime yet' },
    empty: {
        id: 'app.home.insights.empty',
        defaultMessage: 'Play something and your stats will show up here.',
    },
    errorTitle: {
        id: 'app.home.insights.error-title',
        defaultMessage: "Couldn't load your playtime",
    },
    weekEmpty: { id: 'app.home.insights.week-empty', defaultMessage: 'Nothing played yet' },
    lastWeek: { id: 'app.home.insights.last-week', defaultMessage: 'Last week: {duration}' },
    ghostLegend: {
        id: 'app.home.insights.ghost-legend',
        defaultMessage: 'Faded bars are last week',
    },
    minutes: { id: 'app.home.playtime.minutes', defaultMessage: '{minutes}m' },
    hoursMinutes: { id: 'app.home.playtime.hours-minutes', defaultMessage: '{hours}h {minutes}m' },
    seconds: { id: 'app.home.playtime.seconds', defaultMessage: '{seconds}s' },
})

const HISTORY_DAYS = 90

const playtimeQuery = useQuery({
    queryKey: ['home', 'daily-playtime', HISTORY_DAYS],
    queryFn: () => {
        const today = new Date()
        return get_daily_playtime(toDateKey(shiftedDate(today, -HISTORY_DAYS)), toDateKey(today))
    },
    staleTime: 60_000,
})
const dailyPlaytime = computed<DailyPlaytime[]>(() => playtimeQuery.data.value ?? [])

function shiftedDate(base: Date, days: number): Date {
    const result = new Date(base)
    result.setDate(result.getDate() + days)
    return result
}

function startOfWeek(date: Date): Date {
    return shiftedDate(date, -((date.getDay() + 6) % 7))
}

const dailyByDate = computed(() => new Map(dailyPlaytime.value.map((entry) => [entry.date, entry])))

function rangeSeconds(start: Date, days: number): number {
    let total = 0
    for (let offset = 0; offset < days; offset++) {
        total += dailyByDate.value.get(toDateKey(shiftedDate(start, offset)))?.played_seconds ?? 0
    }
    return total
}

const weekStart = computed(() => startOfWeek(new Date()))
const thisWeekSeconds = computed(() => rangeSeconds(weekStart.value, 7))
const lastWeekSeconds = computed(() => rangeSeconds(shiftedDate(weekStart.value, -7), 7))

const streakDays = computed(() => {
    const today = new Date()
    let start = 0
    if (!(dailyByDate.value.get(toDateKey(today))?.played_seconds ?? 0)) {
        start = 1
    }
    let days = 0
    for (let offset = start; offset <= HISTORY_DAYS; offset++) {
        if (dailyByDate.value.get(toDateKey(shiftedDate(today, -offset)))?.played_seconds ?? 0) {
            days += 1
        } else {
            break
        }
    }
    return days
})

const weekTopInstance = computed(() => {
    const totals = new Map<string, number>()
    for (let offset = 0; offset < 7; offset++) {
        const entry = dailyByDate.value.get(toDateKey(shiftedDate(weekStart.value, offset)))
        if (entry?.top_instance_name && entry.played_seconds > 0) {
            totals.set(
                entry.top_instance_name,
                (totals.get(entry.top_instance_name) ?? 0) + entry.played_seconds,
            )
        }
    }
    let topName: string | null = null
    let topSeconds = 0
    for (const [name, seconds] of totals) {
        if (seconds > topSeconds) {
            topName = name
            topSeconds = seconds
        }
    }
    return topName
})

const hasAnyPlaytime = computed(() => dailyPlaytime.value.some((entry) => entry.played_seconds > 0))
/** Nothing loaded at all, or nothing ever played: the card shows its empty state. */
const emptyState = computed(() => {
    if (playtimeQuery.isError.value && !playtimeQuery.data.value) return 'error'
    if (!playtimeQuery.isError.value && !hasAnyPlaytime.value) return 'empty'
    return null
})
/**
 * A week with no playtime yet still has a chart: last week's days in faded
 * bars, so the card keeps its shape and says what to measure against.
 */
const weekIsEmpty = computed(() => thisWeekSeconds.value === 0)
const showGhostWeek = computed(() => weekIsEmpty.value && lastWeekSeconds.value > 0)

function formatDuration(seconds: number): string {
    const roundedSeconds = Math.max(0, Math.round(seconds))
    const hours = Math.floor(roundedSeconds / 3600)
    const minutes = Math.floor((roundedSeconds % 3600) / 60)
    if (hours > 0) return formatMessage(messages.hoursMinutes, { hours, minutes })
    if (minutes > 0) return formatMessage(messages.minutes, { minutes })
    return formatMessage(messages.seconds, { seconds: roundedSeconds })
}

const thisWeekLine = computed(() => {
    const duration = formatDuration(thisWeekSeconds.value)
    if (lastWeekSeconds.value === 0) {
        return formatMessage(messages.thisWeek, { duration })
    }
    const percent = Math.round(
        (Math.abs(thisWeekSeconds.value - lastWeekSeconds.value) / lastWeekSeconds.value) * 100,
    )
    if (percent === 0) return formatMessage(messages.thisWeekSame, { duration })
    return formatMessage(
        thisWeekSeconds.value > lastWeekSeconds.value
            ? messages.thisWeekMore
            : messages.thisWeekLess,
        { duration, percent },
    )
})

const weekDays = computed(() => {
    const todayKey = toDateKey(new Date())
    const days = Array.from({ length: 7 }, (_, offset) => {
        const date = shiftedDate(weekStart.value, offset)
        const key = toDateKey(date)
        return {
            key,
            label: formatWeekday(date),
            title: formatDayTitle(date),
            seconds: dailyByDate.value.get(key)?.played_seconds ?? 0,
            today: key === todayKey,
            future: key > todayKey,
        }
    })
    const peak = Math.max(1, ...days.map((day) => day.seconds))
    const peakKey = days.reduce((best, day) => (day.seconds > best.seconds ? day : best)).key
    const ghosts = days.map(
        (_, offset) =>
            dailyByDate.value.get(toDateKey(shiftedDate(weekStart.value, offset - 7)))
                ?.played_seconds ?? 0,
    )
    const ghostPeak = Math.max(1, ...ghosts)
    return days.map((day, index) => ({
        ...day,
        share: day.seconds / peak,
        peak: day.seconds > 0 && day.key === peakKey,
        ghostSeconds: ghosts[index],
        ghostShare: ghosts[index] / ghostPeak,
    }))
})

/** Short enough to sit over a bar: hours to one decimal, or minutes. */
function formatShortDuration(seconds: number): string {
    if (seconds >= 3600) {
        return formatMessage(messages.hoursShort, { hours: Math.round(seconds / 360) / 10 })
    }
    return formatMessage(messages.minutes, { minutes: Math.max(1, Math.round(seconds / 60)) })
}

/** The change against last week, as a signed percentage; null without last week. */
const weekChange = computed(() => {
    if (lastWeekSeconds.value === 0) return null
    const ratio = (thisWeekSeconds.value - lastWeekSeconds.value) / lastWeekSeconds.value
    return {
        up: ratio > 0,
        same: Math.round(ratio * 100) === 0,
        label: new Intl.NumberFormat(locale.value, {
            style: 'percent',
            maximumFractionDigits: 0,
            signDisplay: 'exceptZero',
        }).format(ratio),
    }
})

let disposed = false
let unlistenProcesses: (() => void) | undefined
void process_listener((event: { event: string }) => {
    if (!disposed && event.event === 'finished') void playtimeQuery.refetch()
})
    .then((unlisten) => {
        if (disposed) unlisten()
        else unlistenProcesses = unlisten
    })
    .catch(console.error)
onUnmounted(() => {
    disposed = true
    unlistenProcesses?.()
})
</script>

<template>
    <HomeWidgetFrame
        class="home-sidebar-section home-surface"
        :title="formatMessage(messages.insights)"
        :icon="ChartIcon"
        :loading="playtimeQuery.isPending.value"
        :skeleton-rows="2"
        :empty-text="
            emptyState === 'error'
                ? formatMessage(messages.errorTitle)
                : emptyState === 'empty'
                  ? formatMessage(messages.emptyTitle)
                  : null
        "
        :empty-hint="emptyState === 'empty' ? formatMessage(messages.empty) : null"
        :empty-tone="emptyState === 'error' ? 'warning' : 'neutral'"
        :empty-icon="ChartIcon"
    >
        <template v-if="emptyState === 'error'" #empty-action>
            <Button size="sm" @click="playtimeQuery.refetch()">
                <UpdatedIcon aria-hidden="true" />
                {{ formatMessage(commonMessages.retryButton) }}
            </Button>
        </template>
        <div class="flex min-w-0 flex-col gap-3">
            <div class="flex min-w-0 flex-col gap-0.5">
                <span class="text-xs font-bold text-[var(--color-text-tertiary)]">{{
                    formatMessage(messages.thisWeekLabel)
                }}</span>
                <div
                    v-if="weekIsEmpty"
                    class="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-1"
                >
                    <strong
                        class="text-xl font-extrabold leading-tight text-[var(--color-text-default)]"
                        >{{ formatMessage(messages.weekEmpty) }}</strong
                    >
                    <span
                        v-if="lastWeekSeconds > 0"
                        class="text-xs font-semibold text-[var(--color-text-tertiary)]"
                        >{{
                            formatMessage(messages.lastWeek, {
                                duration: formatDuration(lastWeekSeconds),
                            })
                        }}</span
                    >
                </div>
                <div v-else class="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-1">
                    <strong
                        v-tooltip="thisWeekLine"
                        class="text-2xl font-extrabold leading-tight tabular-nums text-[var(--color-text-primary)]"
                        >{{ formatDuration(thisWeekSeconds) }}</strong
                    >
                    <span
                        v-if="weekChange"
                        class="home-insights-change"
                        :data-trend="weekChange.same ? 'same' : weekChange.up ? 'up' : 'down'"
                    >
                        <TrendingUpIcon v-if="!weekChange.same" aria-hidden="true" />
                        {{ weekChange.label }}
                        <span class="font-medium opacity-80">{{
                            formatMessage(messages.vsLastWeek)
                        }}</span>
                    </span>
                </div>
            </div>

            <ol
                v-if="!weekIsEmpty || showGhostWeek"
                class="home-insights-week"
                :class="{ 'is-ghost': showGhostWeek }"
                :aria-label="formatMessage(messages.thisWeekLabel)"
            >
                <li
                    v-for="day in weekDays"
                    :key="day.key"
                    v-tooltip="`${day.title}\n${formatDuration(day.seconds)}`"
                    :class="{
                        'is-today': day.today,
                        'is-future': day.future,
                        'has-time': day.seconds > 0,
                    }"
                    :aria-label="`${day.title}: ${formatDuration(day.seconds)}`"
                    tabindex="0"
                >
                    <span class="home-insights-plot">
                        <span
                            v-if="showGhostWeek && day.ghostSeconds > 0"
                            class="home-insights-bar is-ghost"
                            :style="{ height: `${Math.max(8, day.ghostShare * 100)}%` }"
                        />
                        <span
                            v-else-if="day.seconds > 0"
                            class="home-insights-bar"
                            :style="{ height: `${Math.max(8, day.share * 100)}%` }"
                        >
                            <span class="home-insights-value" :class="{ 'is-peak': day.peak }">{{
                                formatShortDuration(day.seconds)
                            }}</span>
                        </span>
                    </span>
                    <span class="home-insights-day" aria-hidden="true">{{ day.label }}</span>
                </li>
            </ol>
            <p v-else class="m-0 text-xs leading-snug text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.empty) }}
            </p>
            <p v-if="showGhostWeek" class="home-insights-legend">
                <span aria-hidden="true" />
                {{ formatMessage(messages.ghostLegend) }}
            </p>

            <div v-if="streakDays > 0 || weekTopInstance" class="flex min-w-0 flex-wrap gap-1.5">
                <span v-if="streakDays > 0" class="home-chip">
                    <TrendingUpIcon aria-hidden="true" />
                    <span class="truncate">{{
                        formatMessage(messages.streak, { days: streakDays })
                    }}</span>
                </span>
                <span v-if="weekTopInstance" class="home-chip">
                    <GameIcon aria-hidden="true" />
                    <span class="truncate">{{
                        formatMessage(messages.weekTop, { name: weekTopInstance })
                    }}</span>
                </span>
            </div>
        </div>
    </HomeWidgetFrame>
</template>

<style scoped>
.home-insights-change {
    @apply inline-flex items-center gap-1 rounded-full px-2 text-xs font-bold leading-5;
    background: var(--home-widget-tint);
    color: var(--color-text-default);
}

.home-insights-change svg {
    @apply size-3.5;
}

.home-insights-change[data-trend='up'] {
    @apply bg-highlight-green text-brand-green;
}

.home-insights-change[data-trend='down'] {
    @apply bg-highlight-orange text-brand-orange;
}

.home-insights-change[data-trend='down'] svg {
    transform: scaleY(-1);
}

.home-insights-week {
    @apply relative m-0 grid list-none grid-cols-7 gap-1.5 p-0 pt-4;
}

.home-insights-week > li {
    @apply flex min-w-0 flex-col items-center gap-1.5 rounded-[var(--radius-sm)] outline-none;
}

.home-insights-week > li:focus-visible {
    box-shadow: 0 0 0 3px var(--color-brand-shadow);
}

/* Days share one baseline; only days with playtime grow a bar from it. */
.home-insights-plot {
    @apply relative flex h-14 w-full items-end justify-center;
    border-bottom: 2px solid var(--home-widget-tint-strong);
}

.home-insights-bar {
    @apply relative block w-3/5 max-w-4 rounded-t-[0.25rem];
    background: color-mix(in srgb, var(--color-brand) 55%, transparent);
    transition: height 300ms ease;
}

.home-insights-week > li.is-today .home-insights-bar {
    @apply bg-brand;
}

/* Last week's bars behind an empty week: dashed outlines, clearly not this week. */
.home-insights-bar.is-ghost,
.home-insights-week > li.is-today .home-insights-bar.is-ghost {
    border: 1.5px dashed color-mix(in srgb, var(--color-brand) 55%, transparent);
    border-bottom: 0;
    background: color-mix(in srgb, var(--color-brand) 10%, transparent);
}

.home-insights-legend {
    @apply m-0 flex items-center gap-1.5 text-[0.6875rem] font-semibold text-[var(--color-text-tertiary)];
}

.home-insights-legend > span {
    @apply inline-block size-2.5 rounded-[0.125rem];
    border: 1.5px dashed color-mix(in srgb, var(--color-brand) 55%, transparent);
    background: color-mix(in srgb, var(--color-brand) 10%, transparent);
}

/* The value shows over the week's longest day, and over any day on hover or focus. */
.home-insights-value {
    @apply pointer-events-none absolute bottom-full left-1/2 mb-1 -translate-x-1/2 whitespace-nowrap text-[0.625rem] font-bold leading-none tabular-nums text-[var(--color-text-default)] opacity-0;
    transition: opacity 120ms ease;
}

.home-insights-value.is-peak,
.home-insights-week > li:hover .home-insights-value,
.home-insights-week > li:focus-visible .home-insights-value {
    @apply opacity-100;
}

.home-insights-day {
    @apply relative text-[0.6875rem] font-semibold leading-none text-[var(--color-text-tertiary)];
}

.home-insights-week > li.is-future .home-insights-day {
    @apply opacity-50;
}

/* Today: accent label with a dot under it. */
.home-insights-week > li.is-today .home-insights-day {
    @apply text-brand;
}

.home-insights-week > li.is-today .home-insights-day::after {
    @apply absolute -bottom-1.5 left-1/2 size-1 -translate-x-1/2 rounded-full bg-brand;
    content: '';
}

@media (prefers-reduced-motion: reduce) {
    .home-insights-bar,
    .home-insights-value {
        transition: none;
    }
}
</style>
