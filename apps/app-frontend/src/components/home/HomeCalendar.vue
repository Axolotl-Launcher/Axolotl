<script setup lang="ts">
import {
    CalendarIcon,
    ChevronLeftIcon,
    ChevronRightIcon,
    PlayIcon,
    RotateCounterClockwiseIcon,
    SpinnerIcon,
    StopCircleIcon,
} from '@modrinth/assets'
import {
    Button,
    defineMessages,
    injectNotificationManager,
    useFormatDateTime,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import HomeWidgetFrame from '@/components/home/HomeWidgetFrame.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { trackEvent } from '@/helpers/analytics'
import {
    type DailyPlaytime,
    type DailyPlaytimeEntry,
    get_daily_playtime,
    get_daily_playtime_details,
    kill,
    run,
} from '@/helpers/instance'
import { instanceLaunchStartedAt } from '@/helpers/instance-launch-state'
import { createRequestGeneration } from '@/helpers/request-generation'
import type { GameInstance } from '@/helpers/types'
import { handleSevereError } from '@/store/error'

import type { HomeWidgetSize } from './home-dashboard'
import {
    buildHeatmapDays,
    dateFromKey,
    endOfPeriod,
    getPlaytimeLevel,
    shiftPeriod,
    startOfPeriod,
    toDateKey,
} from './home-utils'

const props = defineProps<{
    instances: GameInstance[]
    dashboardSize?: HomeWidgetSize | null
}>()

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const { instanceRevision, runningInstanceIds } = useHomeDashboardRuntime()
const formatPeriod = useFormatDateTime({ month: 'long', year: 'numeric' })
const formatWeekday = useFormatDateTime({ weekday: 'narrow' })
const formatDetailDate = useFormatDateTime({ month: 'long', day: 'numeric' })
const formatFullDate = useFormatDateTime({ dateStyle: 'full' })

const messages = defineMessages({
    calendar: { id: 'app.home.calendar.title', defaultMessage: 'Calendar' },
    thisMonth: { id: 'app.home.calendar.this-month', defaultMessage: 'This month' },
    previousMonth: { id: 'app.home.calendar.previous', defaultMessage: 'Previous month' },
    nextMonth: { id: 'app.home.calendar.next', defaultMessage: 'Next month' },
    playedOn: { id: 'app.home.calendar.played-on', defaultMessage: 'On {date} you played:' },
    noActivity: {
        id: 'app.home.calendar.no-activity',
        defaultMessage: 'No playtime recorded on this day.',
    },
    playInstance: { id: 'app.home.calendar.play', defaultMessage: 'Play' },
    stopInstance: { id: 'app.home.calendar.stop', defaultMessage: 'Stop' },
    minutes: { id: 'app.home.playtime.minutes', defaultMessage: '{minutes}m' },
    hoursMinutes: { id: 'app.home.playtime.hours-minutes', defaultMessage: '{hours}h {minutes}m' },
    seconds: { id: 'app.home.playtime.seconds', defaultMessage: '{seconds}s' },
    sessions: {
        id: 'app.home.playtime.sessions',
        defaultMessage: '{count, plural, one {# successful launch} other {# successful launches}}',
    },
    mostPlayed: { id: 'app.home.playtime.most-played', defaultMessage: 'Most played: {name}' },
    monthTotal: {
        id: 'app.home.calendar.month-total',
        defaultMessage: '{duration} this month',
    },
    hoursShort: { id: 'app.home.playtime.hours-short', defaultMessage: '{hours}h' },
})

const todayKey = toDateKey(new Date())
const anchor = ref(new Date())
const selectedKey = ref(todayKey)
const dailyPlaytime = ref<DailyPlaytime[]>([])
const dayDetails = ref<DailyPlaytimeEntry[]>([])
const playtimeRequests = createRequestGeneration()
const detailsRequests = createRequestGeneration()

const periodStart = computed(() => startOfPeriod(anchor.value, 'month'))
const periodEnd = computed(() => endOfPeriod(anchor.value, 'month'))
const periodLabel = computed(() => formatPeriod(periodStart.value))
const days = computed(() => buildHeatmapDays(anchor.value, 'month'))
const weekdayLabels = computed(() =>
    Array.from({ length: 7 }, (_, index) => formatWeekday(new Date(2024, 0, index + 1, 12))),
)
const dailyByDate = computed(() => new Map(dailyPlaytime.value.map((entry) => [entry.date, entry])))

function heatmapLevel(dateKey: string): number {
    return getPlaytimeLevel(dailyByDate.value.get(dateKey)?.played_seconds ?? 0)
}
const monthSeconds = computed(() =>
    dailyPlaytime.value.reduce((total, entry) => total + entry.played_seconds, 0),
)

/** Short enough to sit beside a name: hours to one decimal, or minutes. */
function formatShortDuration(seconds: number): string {
    if (seconds >= 3600) {
        return formatMessage(messages.hoursShort, { hours: Math.round(seconds / 360) / 10 })
    }
    return formatMessage(messages.minutes, { minutes: Math.max(1, Math.round(seconds / 60)) })
}
const isCurrentMonth = computed(
    () => toDateKey(periodStart.value) === toDateKey(startOfPeriod(new Date(), 'month')),
)
const dayTotalSeconds = computed(() =>
    dayDetails.value.reduce((total, entry) => total + entry.played_seconds, 0),
)
const canGoForward = computed(() => toDateKey(periodEnd.value) < todayKey)
const instanceById = computed(
    () => new Map(props.instances.map((instance) => [instance.id, instance])),
)
const selectedDateLabel = computed(() => formatDetailDate(dateFromKey(selectedKey.value)))
const detailRows = computed(() =>
    dayDetails.value.map((entry) => ({
        entry,
        instance: instanceById.value.get(entry.instance_id),
    })),
)

function formatDuration(seconds: number): string {
    const roundedSeconds = Math.max(0, Math.round(seconds))
    const hours = Math.floor(roundedSeconds / 3600)
    const minutes = Math.floor((roundedSeconds % 3600) / 60)
    if (hours > 0) return formatMessage(messages.hoursMinutes, { hours, minutes })
    if (minutes > 0) return formatMessage(messages.minutes, { minutes })
    return formatMessage(messages.seconds, { seconds: roundedSeconds })
}

function tooltipLinesFor(dateKey: string): string[] {
    const entry = dailyByDate.value.get(dateKey)
    const lines = [formatFullDate(dateFromKey(dateKey))]
    if (entry && entry.played_seconds > 0) {
        lines.push(
            formatDuration(entry.played_seconds),
            formatMessage(messages.sessions, { count: entry.session_count }),
        )
        if (entry.top_instance_name) {
            lines.push(formatMessage(messages.mostPlayed, { name: entry.top_instance_name }))
        }
    } else {
        lines.push(formatMessage(messages.noActivity))
    }
    return lines
}

async function refreshPlaytime() {
    const start = toDateKey(periodStart.value)
    const end = toDateKey(periodEnd.value)
    const request = playtimeRequests.begin(`${start}\0${end}`)
    try {
        const result = await get_daily_playtime(start, end)
        if (
            playtimeRequests.isCurrent(
                request,
                `${toDateKey(periodStart.value)}\0${toDateKey(periodEnd.value)}`,
            ) &&
            start === toDateKey(periodStart.value) &&
            end === toDateKey(periodEnd.value)
        ) {
            dailyPlaytime.value = result
        }
    } catch (error) {
        if (
            playtimeRequests.isCurrent(
                request,
                `${toDateKey(periodStart.value)}\0${toDateKey(periodEnd.value)}`,
            )
        ) {
            handleError(error)
            dailyPlaytime.value = []
        }
    }
}

async function refreshDayDetails() {
    const dateKey = selectedKey.value
    const request = detailsRequests.begin(dateKey)
    try {
        const result = await get_daily_playtime_details(dateKey)
        if (detailsRequests.isCurrent(request, selectedKey.value) && dateKey === selectedKey.value)
            dayDetails.value = result
    } catch (error) {
        if (detailsRequests.isCurrent(request, selectedKey.value)) {
            handleError(error)
            dayDetails.value = []
        }
    }
}

function movePeriod(amount: number) {
    anchor.value = shiftPeriod(anchor.value, 'month', amount)
}

function goToThisMonth() {
    anchor.value = new Date()
    selectedKey.value = todayKey
}

function selectDay(dateKey: string) {
    if (dateKey > todayKey) return
    selectedKey.value = dateKey
}

async function playInstance(instance: GameInstance) {
    try {
        await run(instance.id)
        trackEvent('InstanceStart', {
            loader: instance.loader,
            game_version: instance.game_version,
            source: 'HomeCalendar',
        })
    } catch (error) {
        const handled = await handleMinecraftLaunchError(error, {
            instance_id: instance.id,
            instance_name: instance.name,
        })
        if (!handled) handleSevereError(error, { instanceId: instance.id })
    }
}

async function stopInstance(instance: GameInstance) {
    await kill(instance.id).catch(handleError)
    trackEvent('InstanceStop', {
        loader: instance.loader,
        game_version: instance.game_version,
        source: 'HomeCalendar',
    })
}

watch(() => anchor.value.getTime(), refreshPlaytime, { immediate: true })
watch(selectedKey, refreshDayDetails, { immediate: true })
watch(instanceRevision, async () => {
    await refreshPlaytime()
    await refreshDayDetails()
})
</script>

<template>
    <HomeWidgetFrame class="home-calendar" :title="periodLabel" :icon="CalendarIcon">
        <template #actions>
            <Button
                v-if="!isCurrentMonth"
                v-tooltip="formatMessage(messages.thisMonth)"
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.thisMonth)"
                @click="goToThisMonth"
                ><RotateCounterClockwiseIcon />
            </Button>
            <Button
                v-tooltip="formatMessage(messages.previousMonth)"
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.previousMonth)"
                @click="movePeriod(-1)"
                ><ChevronLeftIcon />
            </Button>
            <Button
                v-tooltip="formatMessage(messages.nextMonth)"
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.nextMonth)"
                :disabled="!canGoForward"
                @click="movePeriod(1)"
                ><ChevronRightIcon />
            </Button>
        </template>

        <div class="flex-none">
            <div
                class="mb-1 grid grid-cols-7 gap-[0.1875rem] text-center text-[0.6875rem] font-semibold text-[var(--color-text-tertiary)]"
                aria-hidden="true"
            >
                <span v-for="(weekday, index) in weekdayLabels" :key="index">{{ weekday }}</span>
            </div>
            <div
                class="grid grid-cols-7 gap-[0.1875rem]"
                role="grid"
                :aria-label="formatMessage(messages.calendar)"
            >
                <button
                    v-for="day in days"
                    :key="day.dateKey"
                    v-tooltip="
                        day.inPeriod && day.dateKey <= todayKey
                            ? {
                                  content: tooltipLinesFor(day.dateKey).join('\n'),
                                  popperClass: 'whitespace-pre-line !leading-snug',
                              }
                            : undefined
                    "
                    type="button"
                    class="home-calendar-cell"
                    :class="{
                        'is-outside': !day.inPeriod,
                        'is-future': day.inPeriod && day.dateKey > todayKey,
                        'is-today': day.inPeriod && day.dateKey === todayKey,
                        'is-selected': day.inPeriod && day.dateKey === selectedKey,
                    }"
                    :data-level="
                        day.inPeriod && day.dateKey <= todayKey
                            ? heatmapLevel(day.dateKey)
                            : undefined
                    "
                    :tabindex="day.inPeriod && day.dateKey <= todayKey ? 0 : -1"
                    :disabled="!day.inPeriod || day.dateKey > todayKey"
                    :data-date-key="
                        day.inPeriod && day.dateKey <= todayKey ? day.dateKey : undefined
                    "
                    :aria-label="day.inPeriod ? day.dateKey : undefined"
                    :aria-pressed="day.inPeriod ? day.dateKey === selectedKey : undefined"
                    role="gridcell"
                    @click="selectDay(day.dateKey)"
                >
                    <span v-if="day.inPeriod" aria-hidden="true">{{ day.date.getDate() }}</span>
                </button>
            </div>
        </div>

        <div class="home-calendar-day flex min-h-0 min-w-0 flex-1 flex-col gap-0.5 pt-2">
            <div class="flex min-w-0 items-baseline gap-1.5 px-1 text-xs">
                <h3
                    class="m-0 min-w-0 truncate text-[0.8125rem] font-bold text-[var(--color-text-primary)]"
                >
                    {{ selectedDateLabel }}
                </h3>
                <span
                    v-if="dayTotalSeconds > 0"
                    class="shrink-0 font-semibold tabular-nums text-[var(--color-text-default)]"
                    >{{ formatShortDuration(dayTotalSeconds) }}</span
                >
                <span
                    v-tooltip="
                        formatMessage(messages.monthTotal, {
                            duration: formatDuration(monthSeconds),
                        })
                    "
                    class="ml-auto shrink-0 tabular-nums text-[var(--color-text-tertiary)]"
                    >{{
                        formatMessage(messages.monthTotal, {
                            duration: formatShortDuration(monthSeconds),
                        })
                    }}</span
                >
            </div>
            <p
                v-if="dayDetails.length === 0"
                class="m-0 px-1 text-[0.8125rem] text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.noActivity) }}
            </p>
            <ul
                v-else
                class="m-0 flex min-h-0 list-none flex-col overflow-y-auto overflow-x-hidden p-0"
                :aria-label="formatMessage(messages.playedOn, { date: selectedDateLabel })"
            >
                <li
                    v-for="row in detailRows"
                    :key="row.entry.instance_id"
                    class="home-calendar-row home-row group"
                >
                    <InstanceIcon
                        :icon-path="row.instance?.icon_path"
                        :instance-id="row.entry.instance_id"
                        :loader="row.instance?.loader"
                        size="28px"
                        class="col-start-1 row-span-2 row-start-1 shrink-0"
                    />
                    <span
                        class="col-start-2 row-start-1 min-w-0 truncate text-[0.8125rem] font-semibold leading-tight text-[var(--color-text-primary)]"
                    >
                        {{ row.instance?.name ?? row.entry.instance_name }}
                    </span>
                    <span class="col-start-2 row-start-2 flex min-w-0 items-center gap-1.5">
                        <span class="home-calendar-share" aria-hidden="true">
                            <span
                                :style="{
                                    width: `${Math.max(6, (row.entry.played_seconds / Math.max(1, dayTotalSeconds)) * 100)}%`,
                                }"
                            />
                        </span>
                        <span
                            class="shrink-0 text-[0.6875rem] font-semibold tabular-nums text-[var(--color-text-tertiary)]"
                        >
                            {{ formatShortDuration(row.entry.played_seconds) }}
                        </span>
                    </span>
                    <span
                        v-if="row.instance"
                        class="col-start-3 row-span-2 row-start-1 flex shrink-0 items-center"
                    >
                        <button
                            v-if="runningInstanceIds.includes(row.instance.id)"
                            v-tooltip="formatMessage(messages.stopInstance)"
                            type="button"
                            class="home-calendar-play is-stop"
                            :aria-label="formatMessage(messages.stopInstance)"
                            @click="stopInstance(row.instance)"
                        >
                            <StopCircleIcon aria-hidden="true" />
                        </button>
                        <button
                            v-else
                            v-tooltip="formatMessage(messages.playInstance)"
                            type="button"
                            class="home-calendar-play"
                            :aria-label="formatMessage(messages.playInstance)"
                            :disabled="instanceLaunchStartedAt(row.instance.id) !== undefined"
                            @click="playInstance(row.instance)"
                        >
                            <SpinnerIcon
                                v-if="instanceLaunchStartedAt(row.instance.id) !== undefined"
                                class="animate-spin"
                                aria-hidden="true"
                            />
                            <PlayIcon v-else aria-hidden="true" />
                        </button>
                    </span>
                </li>
            </ul>
        </div>
    </HomeWidgetFrame>
</template>

<style scoped>
.home-calendar-cell {
    @apply flex h-[1.1875rem] cursor-pointer items-center justify-center rounded-[var(--radius-sm)] border-0 p-0 text-[0.6875rem] font-semibold tabular-nums text-[var(--color-text-primary)] outline-none;
    background: var(--home-widget-tint);
    transition:
        box-shadow 100ms ease,
        background-color 100ms ease;
}

.home-calendar-cell.is-outside {
    @apply cursor-default bg-transparent;
}

.home-calendar-cell.is-future {
    @apply cursor-default bg-transparent text-[var(--color-text-tertiary)] opacity-50;
}

/* Playtime levels are shares of the accent, so they follow the theme and stay
   readable over the widget fill at any opacity. */
.home-calendar-cell[data-level='1'] {
    background: color-mix(in srgb, var(--color-brand) 22%, transparent);
}

.home-calendar-cell[data-level='2'] {
    background: color-mix(in srgb, var(--color-brand) 42%, transparent);
}

.home-calendar-cell[data-level='3'] {
    background: color-mix(in srgb, var(--color-brand) 66%, transparent);
}

.home-calendar-cell[data-level='4'] {
    @apply bg-brand text-[var(--color-accent-contrast)];
}

.home-calendar-cell.is-today {
    box-shadow: inset 0 0 0 1.5px var(--color-brand);
}

.home-calendar-cell.is-selected,
.home-calendar-cell:focus-visible {
    box-shadow:
        0 0 0 2px var(--color-brand),
        0 0 0 4px var(--color-brand-shadow);
}

.home-calendar-day {
    border-top: 1px solid var(--home-widget-edge);
}

/* Name on the first line, its share of the day and the time on the second. */
.home-calendar-row {
    @apply grid min-w-0 items-center gap-x-2 gap-y-1 px-1 py-1;
    grid-template-columns: auto minmax(0, 1fr) auto;
}

.home-calendar-play {
    @apply flex size-7 cursor-pointer items-center justify-center rounded-full border-0 p-0 text-brand;
    background: var(--home-widget-accent-tint);
}

.home-calendar-play svg {
    @apply size-3.5;
}

.home-calendar-play:hover:not(:disabled),
.home-calendar-play:focus-visible {
    @apply bg-brand text-[var(--color-accent-contrast)];
    outline: none;
}

.home-calendar-play:disabled {
    @apply cursor-not-allowed opacity-50;
}

.home-calendar-play.is-stop {
    @apply bg-highlight-red text-brand-red;
}

.home-calendar-share {
    @apply block h-1 min-w-0 flex-1 overflow-hidden rounded-full;
    background: var(--home-widget-tint);
}

.home-calendar-share > span {
    @apply block h-full rounded-full bg-brand;
}

@media (prefers-reduced-motion: reduce) {
    .home-calendar-cell {
        transition: none;
    }
}
</style>
