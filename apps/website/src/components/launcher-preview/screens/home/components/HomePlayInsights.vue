<script setup lang="ts">
import ChartIcon from '@modrinth/assets/icons/chart.svg?component'
import ClockIcon from '@modrinth/assets/icons/clock.svg?component'
import GameIcon from '@modrinth/assets/icons/game.svg?component'
import TrendingUpIcon from '@modrinth/assets/icons/trending-up.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'

import { toDateKey } from '../home-utils'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import type { DailyPlaytime } from '../state'

const { formatMessage } = useVIntl()
const runtime = useHomeRuntime()
const messages = homeMessages

const HISTORY_DAYS = 90

// The app's query reads the playtime table once and refetches whenever a game exits.
runtime.loadInsights()
const isPending = computed(() => !runtime.insightsLoaded.value)

function shiftedDate(base: Date, days: number): Date {
    const result = new Date(base)
    result.setDate(result.getDate() + days)
    return result
}

function startOfWeek(date: Date): Date {
    return shiftedDate(date, -((date.getDay() + 6) % 7))
}

const dailyPlaytime = computed<DailyPlaytime[]>(() => {
    const today = new Date()
    return runtime.getDailyPlaytime(toDateKey(shiftedDate(today, -HISTORY_DAYS)), toDateKey(today))
})

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
    if (hours > 0) return formatMessage(messages.playtimeHoursMinutes, { hours, minutes })
    if (minutes > 0) return formatMessage(messages.playtimeMinutes, { minutes })
    return formatMessage(messages.playtimeSeconds, { seconds: roundedSeconds })
}

const thisWeekLine = computed(() => {
    const duration = formatDuration(thisWeekSeconds.value)
    if (lastWeekSeconds.value === 0) {
        return formatMessage(messages.insightsThisWeek, { duration })
    }
    const percent = Math.round(
        (Math.abs(thisWeekSeconds.value - lastWeekSeconds.value) / lastWeekSeconds.value) * 100,
    )
    if (percent === 0) return formatMessage(messages.insightsThisWeekSame, { duration })
    return formatMessage(
        thisWeekSeconds.value > lastWeekSeconds.value
            ? messages.insightsThisWeekMore
            : messages.insightsThisWeekLess,
        { duration, percent },
    )
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
                {{ formatMessage(messages.insightsTitle) }}
            </h2>
        </div>
        <div v-if="isPending" class="flex flex-col gap-2" aria-hidden="true">
            <div class="h-4 w-3/4 animate-pulse rounded bg-surface-4" />
            <div class="h-4 w-1/2 animate-pulse rounded bg-surface-4" />
        </div>
        <p v-else-if="!hasAnyPlaytime" class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.insightsEmpty) }}
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
                    formatMessage(messages.insightsStreak, { days: streakDays })
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
                    {{ formatMessage(messages.insightsWeekTop, { name: weekTopInstance }) }}
                </span>
            </li>
        </ul>
    </section>
</template>
