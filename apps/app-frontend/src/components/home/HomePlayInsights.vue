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
    unitHours: {
        id: 'app.home.insights.unit-hours',
        defaultMessage: '{count, plural, one {hour} other {hours}}',
    },
    unitMinutes: {
        id: 'app.home.insights.unit-minutes',
        defaultMessage: '{count, plural, one {minute} other {minutes}}',
    },
    dailyAverage: { id: 'app.home.insights.daily-average', defaultMessage: 'Avg {duration}' },
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

/**
 * A bar's height: strictly its share of the longest day, so one minute and an
 * hour never stand the same. A played day keeps two pixels, only so it shows.
 */
function barHeight(share: number): string {
    return `max(2px, ${(Math.max(0, Math.min(1, share)) * 100).toFixed(2)}%)`
}

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

/**
 * The week's total as one big figure: hours to one decimal from an hour up,
 * minutes below that. The unit is set apart so the number carries the card.
 */
const heroFigure = computed(() => {
    const seconds = thisWeekSeconds.value
    const number = new Intl.NumberFormat(locale.value, { maximumFractionDigits: 1 })
    if (seconds >= 3600) {
        const hours = Math.round(seconds / 360) / 10
        return {
            value: number.format(hours),
            unit: formatMessage(messages.unitHours, { count: hours }),
        }
    }
    const minutes = Math.max(seconds > 0 ? 1 : 0, Math.round(seconds / 60))
    return {
        value: number.format(minutes),
        unit: formatMessage(messages.unitMinutes, { count: minutes }),
    }
})

/** The average of this week's played days, drawn as a reference line once there are two. */
const dailyAverage = computed(() => {
    const played = weekDays.value.filter((day) => !day.future && day.seconds > 0)
    if (played.length < 2) return null
    const seconds = played.reduce((total, day) => total + day.seconds, 0) / played.length
    const peak = Math.max(...weekDays.value.map((day) => day.seconds))
    const share = seconds / Math.max(1, peak)
    // Right under the tallest bar it would only cover its value.
    return share > 0.85 ? null : { seconds, share }
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
        variant="sidebar"
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
            <Button size="sm" type="quiet" color="brand" @click="playtimeQuery.refetch()">
                <UpdatedIcon aria-hidden="true" />
                {{ formatMessage(commonMessages.retryButton) }}
            </Button>
        </template>
        <div class="home-insights flex min-w-0 flex-col">
            <div class="home-insights-hero">
                <strong v-if="weekIsEmpty" class="home-insights-empty-week">{{
                    formatMessage(messages.weekEmpty)
                }}</strong>
                <span v-else v-tooltip="thisWeekLine" class="home-insights-figure">
                    <strong>{{ heroFigure.value }}</strong>
                    <span>{{ heroFigure.unit }}</span>
                </span>
                <span
                    v-if="weekChange && !weekIsEmpty"
                    v-tooltip="formatMessage(messages.vsLastWeek)"
                    class="home-insights-change"
                    :data-trend="weekChange.same ? 'same' : weekChange.up ? 'up' : 'down'"
                >
                    <TrendingUpIcon v-if="!weekChange.same" aria-hidden="true" />
                    {{ weekChange.label }}
                </span>
            </div>
            <p class="home-insights-caption">
                <span>{{ formatMessage(messages.thisWeekLabel) }}</span>
                <template v-if="weekIsEmpty && lastWeekSeconds > 0">
                    <span aria-hidden="true">·</span>
                    <span>{{
                        formatMessage(messages.lastWeek, {
                            duration: formatDuration(lastWeekSeconds),
                        })
                    }}</span>
                </template>
                <template v-if="streakDays > 0">
                    <span aria-hidden="true">·</span>
                    <span class="home-insights-fact">
                        <TrendingUpIcon aria-hidden="true" />
                        {{ formatMessage(messages.streak, { days: streakDays }) }}
                    </span>
                </template>
            </p>
            <p v-if="weekTopInstance" class="home-insights-caption">
                <span class="home-insights-fact min-w-0">
                    <GameIcon aria-hidden="true" />
                    <span class="truncate">{{
                        formatMessage(messages.weekTop, { name: weekTopInstance })
                    }}</span>
                </span>
            </p>

            <p v-if="showGhostWeek" class="home-insights-legend">
                <span aria-hidden="true" />
                {{ formatMessage(messages.ghostLegend) }}
            </p>
            <ol
                v-if="!weekIsEmpty || showGhostWeek"
                class="home-insights-week"
                :class="{ 'is-ghost': showGhostWeek }"
                :aria-label="formatMessage(messages.thisWeekLabel)"
            >
                <li
                    v-if="dailyAverage && !showGhostWeek"
                    class="home-insights-average"
                    :style="{ '--home-insights-average': dailyAverage.share }"
                    aria-hidden="true"
                >
                    <span>{{
                        formatMessage(messages.dailyAverage, {
                            duration: formatShortDuration(dailyAverage.seconds),
                        })
                    }}</span>
                </li>
                <li
                    v-for="day in weekDays"
                    :key="day.key"
                    v-tooltip="`${day.title}\n${formatDuration(day.seconds)}`"
                    class="home-insights-day"
                    :class="{
                        'is-today': day.today,
                        'is-future': day.future,
                        'has-time': showGhostWeek ? day.ghostSeconds > 0 : day.seconds > 0,
                    }"
                    :aria-label="`${day.title}: ${formatDuration(day.seconds)}`"
                    tabindex="0"
                >
                    <span class="home-insights-plot">
                        <span
                            v-if="showGhostWeek && day.ghostSeconds > 0"
                            class="home-insights-bar is-ghost"
                            :style="{ height: barHeight(day.ghostShare) }"
                        />
                        <span
                            v-else-if="!showGhostWeek && day.seconds > 0"
                            class="home-insights-bar"
                            :style="{ height: barHeight(day.share) }"
                        >
                            <span class="home-insights-value" :class="{ 'is-peak': day.peak }">{{
                                formatShortDuration(day.seconds)
                            }}</span>
                        </span>
                    </span>
                    <span class="home-insights-label" aria-hidden="true">{{ day.label }}</span>
                </li>
            </ol>
            <p v-else class="m-0 mt-3 text-xs leading-snug text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.empty) }}
            </p>
        </div>
    </HomeWidgetFrame>
</template>

<style scoped>
/* The week's total is the headline: a big figure, the unit beside it. */
.home-insights-hero {
    @apply flex min-w-0 flex-wrap items-center justify-between gap-x-3 gap-y-1;
}

.home-insights-figure {
    @apply flex min-w-0 items-baseline gap-1.5 text-[var(--color-text-primary)];
}

.home-insights-figure > strong {
    @apply text-[2.75rem] font-black leading-none tabular-nums tracking-tight;
}

.home-insights-figure > span {
    @apply text-sm font-bold text-[var(--color-text-tertiary)];
}

.home-insights-empty-week {
    @apply text-xl font-extrabold leading-tight text-[var(--color-text-default)];
}

/* The change against last week: a solid badge, so it reads at a glance. */
.home-insights-change {
    @apply inline-flex shrink-0 items-center gap-1 rounded-full px-2.5 text-[0.8125rem] font-extrabold leading-7 tabular-nums;
    background: var(--home-widget-tint-strong);
    color: var(--color-text-default);
}

.home-insights-change svg {
    @apply size-4;
}

/* The change is a status word too: its darker shade, on a tint of its colour. */
.home-insights-change[data-trend='up'] {
    background: color-mix(in srgb, var(--color-green) 12%, transparent);
    color: var(--home-status-green-text);
}

.home-insights-change[data-trend='down'] {
    background: color-mix(in srgb, var(--color-orange) 12%, transparent);
    color: var(--home-status-orange-text);
}

.home-insights-change[data-trend='down'] svg {
    transform: scaleY(-1);
}

.home-insights-caption {
    @apply m-0 mt-1.5 flex min-w-0 flex-wrap items-center gap-x-1.5 text-xs font-semibold text-[var(--color-text-tertiary)];
}

.home-insights-caption + .home-insights-caption {
    @apply mt-0.5;
}

.home-insights-fact {
    @apply inline-flex items-center gap-1;
}

.home-insights-fact svg {
    @apply size-3.5 shrink-0 text-brand;
}

/*
 * The week as a graphic across the foot of the card: the bars run edge to
 * edge, their heights strictly in proportion, with the day letters on one
 * line under them and each value over its bar.
 */
.home-insights-week {
    @apply relative m-0 -mx-4 -mb-2 mt-4 grid h-28 list-none grid-cols-7 gap-1 px-2 pb-0 pt-5;
}

.home-insights-day {
    @apply relative flex h-full min-w-0 flex-col gap-1 outline-none;
}

.home-insights-day:focus-visible {
    border-radius: var(--radius-sm);
    box-shadow: inset 0 0 0 2px var(--color-brand);
}

.home-insights-plot {
    @apply flex min-h-0 flex-1 items-end;
}

.home-insights-bar {
    @apply relative block w-full;
    border-radius: var(--radius-md) var(--radius-md) 0 0;
    background: color-mix(in srgb, var(--color-brand) 30%, transparent);
    transition:
        height 300ms ease,
        background-color 120ms ease;
}

.home-insights-day:hover .home-insights-bar:not(.is-ghost) {
    background: color-mix(in srgb, var(--color-brand) 46%, transparent);
}

.home-insights-day.is-today .home-insights-bar {
    @apply bg-brand;
}

/* Last week's bars behind an empty week: dashed outlines, clearly not this week. */
.home-insights-bar.is-ghost,
.home-insights-day.is-today .home-insights-bar.is-ghost {
    border: 1.5px dashed color-mix(in srgb, var(--color-brand) 55%, transparent);
    border-bottom: 0;
    background: color-mix(in srgb, var(--color-brand) 8%, transparent);
}

/* Day letters share one line under the bars. */
.home-insights-label {
    @apply pointer-events-none h-3 flex-none text-center text-[0.6875rem] font-bold leading-3 text-[var(--color-text-tertiary)];
}

.home-insights-day.has-time .home-insights-label {
    @apply text-[var(--color-text-primary)];
}

.home-insights-day.is-today .home-insights-label {
    @apply text-brand;
}

.home-insights-day.is-future .home-insights-label {
    @apply opacity-50;
}

/* The value shows over the week's longest day, and over any day on hover or focus. */
.home-insights-value {
    @apply pointer-events-none absolute bottom-full left-1/2 mb-1 -translate-x-1/2 whitespace-nowrap text-[0.625rem] font-bold leading-none tabular-nums text-[var(--color-text-default)] opacity-0;
    transition: opacity 120ms ease;
}

.home-insights-value.is-peak,
.home-insights-day:hover .home-insights-value,
.home-insights-day:focus-visible .home-insights-value {
    @apply opacity-100;
}

/* A dashed line at the daily average that fades in from the left. */
.home-insights-average {
    /* The average's share of the tallest bar, set on the element. */
    --home-insights-average: 0.5;
    @apply pointer-events-none absolute left-0 right-0 z-[1] flex h-0 items-end justify-start px-2;
    /* Above the letters (0.75rem and a 0.25rem gap), over the 1.25rem top padding. */
    bottom: calc(1rem + var(--home-insights-average) * (100% - 2.25rem));
    border-top: 1px dashed color-mix(in srgb, var(--color-text-primary) 40%, transparent);
    -webkit-mask-image: linear-gradient(to left, transparent, #000 40%);
    mask-image: linear-gradient(to left, transparent, #000 40%);
}

.home-insights-average > span {
    @apply mb-0.5 text-[0.625rem] font-bold leading-none tabular-nums text-[var(--color-text-tertiary)];
}

.home-insights-legend {
    @apply m-0 mt-2 flex items-center gap-1.5 text-[0.6875rem] font-semibold text-[var(--color-text-tertiary)];
}

.home-insights-legend > span {
    @apply inline-block size-2.5 rounded-[0.125rem];
    border: 1.5px dashed color-mix(in srgb, var(--color-brand) 55%, transparent);
    background: color-mix(in srgb, var(--color-brand) 8%, transparent);
}

@media (prefers-reduced-motion: reduce) {
    .home-insights-bar,
    .home-insights-value {
        transition: none;
    }
}
</style>
