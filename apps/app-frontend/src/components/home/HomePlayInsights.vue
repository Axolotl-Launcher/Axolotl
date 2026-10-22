<script setup lang="ts">
import { ChartIcon, ClockIcon, GameIcon, TrendingUpIcon } from '@modrinth/assets'
import { Button, commonMessages, defineMessages, useVIntl } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, onUnmounted } from 'vue'

import { process_listener } from '@/helpers/events'
import { type DailyPlaytime, get_daily_playtime } from '@/helpers/instance'

import { toDateKey } from './home-utils'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    insights: { id: 'app.home.insights.title', defaultMessage: 'Play insights' },
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
    empty: {
        id: 'app.home.insights.empty',
        defaultMessage: 'Play something and your stats will show up here.',
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
    <section
        class="flex min-w-0 flex-col gap-3 border-0 border-b-[1px] border-solid border-[--brand-gradient-border] p-4"
    >
        <div class="flex items-center gap-2">
            <ChartIcon
                class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                aria-hidden="true"
            />
            <h2 class="m-0 truncate text-lg">
                {{ formatMessage(messages.insights) }}
            </h2>
        </div>
        <div v-if="playtimeQuery.isPending.value" class="flex flex-col gap-2" aria-hidden="true">
            <div class="h-4 w-3/4 animate-pulse rounded bg-surface-4" />
            <div class="h-4 w-1/2 animate-pulse rounded bg-surface-4" />
        </div>
        <Button
            v-else-if="playtimeQuery.isError.value && !playtimeQuery.data.value"
            @click="playtimeQuery.refetch()"
        >
            {{ formatMessage(commonMessages.retryButton) }}
        </Button>
        <p v-else-if="!hasAnyPlaytime" class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.empty) }}
        </p>
        <ul v-else class="m-0 flex list-none flex-col gap-2 p-0">
            <li class="flex min-w-0 items-center gap-2 text-sm text-[var(--color-text-default)]">
                <ClockIcon
                    class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                    aria-hidden="true"
                />
                <span class="min-w-0">{{ thisWeekLine }}</span>
            </li>
            <li
                v-if="streakDays > 0"
                class="flex min-w-0 items-center gap-2 text-sm text-[var(--color-text-default)]"
            >
                <TrendingUpIcon
                    class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                    aria-hidden="true"
                />
                <span class="min-w-0">{{
                    formatMessage(messages.streak, { days: streakDays })
                }}</span>
            </li>
            <li
                v-if="weekTopInstance"
                class="flex min-w-0 items-center gap-2 text-sm text-[var(--color-text-default)]"
            >
                <GameIcon
                    class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                    aria-hidden="true"
                />
                <span class="min-w-0 truncate">
                    {{ formatMessage(messages.weekTop, { name: weekTopInstance }) }}
                </span>
            </li>
        </ul>
    </section>
</template>
