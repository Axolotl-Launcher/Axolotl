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
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { useHomeDashboardRuntime } from '@/components/home/home-dashboard-runtime'
import HomeFitList from '@/components/home/HomeFitList.vue'
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
    buildHeatWeeks,
    dateFromKey,
    endOfPeriod,
    getBandColumns,
    getCalendarLayout,
    getHeatLevel,
    getLongestStreak,
    getWeekSpan,
    HEAT_LEVELS,
    shiftPeriod,
    startOfPeriod,
    toDateKey,
} from './home-utils'

/*
 * Playtime as a heatmap: each day is a square whose depth of accent is how
 * long was played, scaled to the busiest day in view. One column shows the
 * month as a grid of weeks; a wide card shows a band of weeks like a
 * contribution graph; a single tile shows this week. The chosen day's
 * instances open under the map when there is room for them.
 */
const props = defineProps<{
    instances: GameInstance[]
    dashboardSize?: HomeWidgetSize | null
}>()

const { handleError } = injectNotificationManager()
const { formatMessage, locale } = useVIntl()
const handleMinecraftLaunchError = useMinecraftLaunchError()
const { instanceRevision, runningInstanceIds } = useHomeDashboardRuntime()
const formatPeriod = useFormatDateTime({ month: 'long', year: 'numeric' })
const formatMonthShort = useFormatDateTime({ month: 'short' })
const formatWeekday = useFormatDateTime({ weekday: 'narrow' })
const formatDetailDate = useFormatDateTime({ month: 'long', day: 'numeric' })

const messages = defineMessages({
    calendar: { id: 'app.home.calendar.title', defaultMessage: 'Calendar' },
    thisMonth: { id: 'app.home.calendar.this-month', defaultMessage: 'This month' },
    thisWeek: { id: 'app.home.calendar.this-week', defaultMessage: 'This week' },
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
    monthEmpty: {
        id: 'app.home.calendar.month-empty',
        defaultMessage: 'No playtime this month',
    },
    monthEmptyHint: {
        id: 'app.home.calendar.month-empty-hint',
        defaultMessage: 'Each day you play gets marked on the calendar.',
    },
    statHours: {
        id: 'app.home.calendar.stat-hours',
        defaultMessage: '{count, plural, one {hour} other {hours}}',
    },
    statDays: {
        id: 'app.home.calendar.stat-days',
        defaultMessage: '{count, plural, one {day played} other {days played}}',
    },
    monthHoursShort: {
        id: 'app.home.calendar.month-hours-short',
        defaultMessage: '{hours}h this month',
    },
    dayInstances: {
        id: 'app.home.calendar.day-instances',
        defaultMessage: '{name} · {total} instances',
    },
    insightLongest: { id: 'app.home.calendar.insight-longest', defaultMessage: 'Longest day' },
    insightTop: { id: 'app.home.calendar.insight-top', defaultMessage: 'Most played' },
    insightStreak: { id: 'app.home.calendar.insight-streak', defaultMessage: 'Longest run' },
    insightDays: {
        id: 'app.home.calendar.insight-days',
        defaultMessage: '{count, plural, one {# day} other {# days}}',
    },
    legend: {
        id: 'app.home.calendar.legend',
        defaultMessage: 'Daily playtime, from less to more',
    },
    legendLess: { id: 'app.home.calendar.legend-less', defaultMessage: 'Less' },
    legendMore: { id: 'app.home.calendar.legend-more', defaultMessage: 'More' },
    more: { id: 'app.home.widgets.more', defaultMessage: '{count} more' },
})

/** Weeks of history the band loads; it shows as many recent weeks as fit. */
const BAND_WEEKS = 53

const todayKey = toDateKey(new Date())
const anchor = ref(new Date())
const selectedKey = ref(todayKey)
/** The day keyboard focus enters the map on, and arrow keys move from. */
const focusKey = ref(todayKey)
const dailyPlaytime = ref<DailyPlaytime[]>([])
/** Set once the shown range has loaded, so an empty month is not shown while it loads. */
/** The range whose response `dailyPlaytime` holds; null while unknown or failed. */
const loadedRangeKey = ref<string | null>(null)
const playtimeLoaded = computed(
    () => loadedRangeKey.value === `${fetchRange.value.start}\0${fetchRange.value.end}`,
)
const dayDetails = ref<DailyPlaytimeEntry[]>([])
/** Instances of the chosen day that did not fit under the map. */
const hiddenDetails = ref(0)
const playtimeRequests = createRequestGeneration()
const detailsRequests = createRequestGeneration()

const layout = computed(() => getCalendarLayout(props.dashboardSize))
const periodStart = computed(() => startOfPeriod(anchor.value, 'month'))
const periodEnd = computed(() => endOfPeriod(anchor.value, 'month'))
const periodStartKey = computed(() => toDateKey(periodStart.value))
const periodEndKey = computed(() => toDateKey(periodEnd.value))
/** The last day the band draws: the end of the month shown, or today. */
const rangeEndKey = computed(() => (periodEndKey.value < todayKey ? periodEndKey.value : todayKey))
const title = computed(() =>
    layout.value === 'week' ? formatMessage(messages.thisWeek) : formatPeriod(periodStart.value),
)

const monthDays = computed(() => buildHeatmapDays(anchor.value, 'month'))
const weekDays = computed(() => buildHeatWeeks(new Date(), 1)[0])
const weekdayLabels = computed(() =>
    Array.from({ length: 7 }, (_, index) => formatWeekday(new Date(2024, 0, index + 1, 12))),
)

/* The band: as many whole week columns as the card has room for. */
const bandBox = ref<HTMLElement | null>(null)
const bandSize = ref({ width: 0, height: 0 })
let bandObserver: ResizeObserver | null = null
watch(bandBox, (element) => {
    bandObserver?.disconnect()
    if (!element) return
    bandObserver = new ResizeObserver(([entry]) => {
        bandSize.value = { width: entry.contentRect.width, height: entry.contentRect.height }
    })
    bandObserver.observe(element)
})
onBeforeUnmount(() => bandObserver?.disconnect())

/** Weeks from the first week with any play up to the band's last week; 0 with none. */
const dataWeeks = computed(() => {
    let first: string | null = null
    for (const entry of dailyPlaytime.value) {
        if (entry.played_seconds <= 0 || entry.date > rangeEndKey.value) continue
        if (first === null || entry.date < first) first = entry.date
    }
    return first ? getWeekSpan(first, rangeEndKey.value) : 0
})

const bandGeometry = computed(() => {
    const strip = layout.value === 'strip'
    const gap = strip ? 2 : 3
    // Room for the Mon / Wed / Fri labels on the left and the month names on top.
    const labels = strip ? 0 : 22
    const monthRow = strip ? 13 : 16
    const width = Math.max(0, bandSize.value.width - labels)
    const target = strip
        ? Math.max(4, Math.min(14, Math.floor((bandSize.value.height - monthRow - 6 * gap) / 7)))
        : 15
    const fit = Math.max(1, Math.min(BAND_WEEKS, Math.floor((width + gap) / (target + gap))))
    // A tall band sizes its squares as if it filled the card; a strip keeps them to its height.
    const cell = strip
        ? target
        : Math.max(target, Math.min(19, Math.floor((width - (fit - 1) * gap) / fit)))
    const columns = Math.min(
        getBandColumns(dataWeeks.value),
        Math.floor((width + gap) / (cell + gap)) || 1,
    )
    const bandWidth = labels + columns * cell + (columns - 1) * gap
    // The width the weeks do not need goes to the figures about them.
    const showInsights = bandSize.value.width - bandWidth - 20 >= 120
    return { columns, cell, gap, labels, monthRow, bandWidth, showInsights }
})
const bandWeeks = computed(() =>
    buildHeatWeeks(dateFromKey(rangeEndKey.value), bandGeometry.value.columns),
)
/** A month's short name over the week column that holds its first day. */
const bandMonthLabels = computed(() =>
    bandWeeks.value.flatMap((week, column) => {
        const first = week.find((day) => day.date.getDate() === 1)
        return first && column < bandWeeks.value.length - 1
            ? [{ column, label: formatMonthShort(first.date) }]
            : []
    }),
)

/** What the request asks the backend for: everything the layout draws or counts. */
const fetchRange = computed(() => {
    if (layout.value === 'week') {
        const weekStart = weekDays.value[0].dateKey
        const monthStart = toDateKey(startOfPeriod(new Date(), 'month'))
        return { start: weekStart < monthStart ? weekStart : monthStart, end: todayKey }
    }
    if (layout.value === 'month') return { start: periodStartKey.value, end: periodEndKey.value }
    const first = buildHeatWeeks(dateFromKey(rangeEndKey.value), BAND_WEEKS)[0][0].dateKey
    const start = first < periodStartKey.value ? first : periodStartKey.value
    return { start, end: periodEndKey.value }
})

const dailyByDate = computed(() => new Map(dailyPlaytime.value.map((entry) => [entry.date, entry])))
function secondsOn(dateKey: string): number {
    return dailyByDate.value.get(dateKey)?.played_seconds ?? 0
}

/** The days on screen that have happened; the busiest of them tops the scale. */
const visibleDays = computed(() => {
    const days =
        layout.value === 'week'
            ? weekDays.value
            : layout.value === 'month'
              ? monthDays.value.filter((day) => day.inPeriod)
              : bandWeeks.value.flat()
    return days.filter((day) => day.dateKey <= todayKey)
})
const maxSeconds = computed(() =>
    visibleDays.value.reduce((max, day) => Math.max(max, secondsOn(day.dateKey)), 0),
)
function heatLevel(dateKey: string): number {
    return getHeatLevel(secondsOn(dateKey), maxSeconds.value)
}

/** About the days in the band: the longest day, the instance played most, the longest run. */
const bandInsights = computed(() => {
    let longest: { dateKey: string; seconds: number } | null = null
    const byInstance = new Map<string, number>()
    const played: string[] = []
    for (const day of visibleDays.value) {
        const entry = dailyByDate.value.get(day.dateKey)
        if (!entry || entry.played_seconds <= 0) continue
        played.push(day.dateKey)
        if (!longest || entry.played_seconds > longest.seconds)
            longest = { dateKey: day.dateKey, seconds: entry.played_seconds }
        if (entry.top_instance_name)
            byInstance.set(
                entry.top_instance_name,
                (byInstance.get(entry.top_instance_name) ?? 0) + entry.played_seconds,
            )
    }
    if (!longest) return null
    const top = [...byInstance].sort((a, b) => b[1] - a[1])[0]?.[0] ?? null
    return {
        longest: formatShortDuration(longest.seconds),
        longestDate: formatDetailDate(dateFromKey(longest.dateKey)),
        top,
        streak: formatMessage(messages.insightDays, { count: getLongestStreak(played) }),
    }
})

/** The month the figures are about: the one shown, or this one on the week tile. */
const statsMonth = computed(() =>
    layout.value === 'week'
        ? {
              start: toDateKey(startOfPeriod(new Date(), 'month')),
              end: toDateKey(endOfPeriod(new Date(), 'month')),
          }
        : { start: periodStartKey.value, end: periodEndKey.value },
)
const monthEntries = computed(() =>
    dailyPlaytime.value.filter(
        (entry) =>
            entry.played_seconds > 0 &&
            entry.date >= statsMonth.value.start &&
            entry.date <= statsMonth.value.end,
    ),
)
const monthSeconds = computed(() =>
    monthEntries.value.reduce((total, entry) => total + entry.played_seconds, 0),
)
/** Loaded and nothing played: the figures give way to the empty-month line. */
const monthIsEmpty = computed(() => playtimeLoaded.value && monthSeconds.value === 0)
/** The month in big figures (hours played, days with any play), and this week's hours. */
const monthStats = computed(() => {
    const number = new Intl.NumberFormat(locale.value, { maximumFractionDigits: 1 })
    const hours = Math.round(monthSeconds.value / 360) / 10
    const days = monthEntries.value.length
    const weekHours =
        Math.round(weekDays.value.reduce((total, day) => total + secondsOn(day.dateKey), 0) / 360) /
        10
    return {
        hours: number.format(hours),
        hoursUnit: formatMessage(messages.statHours, { count: hours }),
        monthHours: formatMessage(messages.monthHoursShort, { hours: number.format(hours) }),
        days: number.format(days),
        daysUnit: formatMessage(messages.statDays, { count: days }),
        weekHours: number.format(weekHours),
        weekHoursUnit: formatMessage(messages.statHours, { count: weekHours }),
    }
})

/** Short enough to sit beside a name: hours to one decimal, or minutes. */
function formatShortDuration(seconds: number): string {
    if (seconds >= 3600) {
        return formatMessage(messages.hoursShort, { hours: Math.round(seconds / 360) / 10 })
    }
    return formatMessage(messages.minutes, { minutes: Math.max(1, Math.round(seconds / 60)) })
}
const isCurrentMonth = computed(
    () => periodStartKey.value === toDateKey(startOfPeriod(new Date(), 'month')),
)
const dayTotalSeconds = computed(() =>
    dayDetails.value.reduce((total, entry) => total + entry.played_seconds, 0),
)
const canGoForward = computed(() => periodEndKey.value < todayKey)
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
/** The one-column month sums the chosen day up in one line, so the grid keeps its room. */
const daySummary = computed(() => {
    const top = detailRows.value[0]
    if (!top) return null
    const name = top.instance?.name ?? top.entry.instance_name
    return {
        top,
        text:
            detailRows.value.length > 1
                ? formatMessage(messages.dayInstances, { name, total: detailRows.value.length })
                : name,
    }
})

/*
 * The month grid: seven columns that always span the card. A cell is square
 * when the card is tall enough and flattens when a six-week month would not fit.
 */
const monthBox = ref<HTMLElement | null>(null)
const monthBoxSize = ref({ width: 0, height: 0 })
let monthObserver: ResizeObserver | null = null
watch(monthBox, (element) => {
    monthObserver?.disconnect()
    if (!element) return
    monthObserver = new ResizeObserver(([entry]) => {
        monthBoxSize.value = { width: entry.contentRect.width, height: entry.contentRect.height }
    })
    monthObserver.observe(element)
})
onBeforeUnmount(() => monthObserver?.disconnect())
const MONTH_GAP = 3
const monthCell = computed(() => {
    const rows = monthDays.value.length / 7
    const { width, height } = monthBoxSize.value
    if (!width || !height) return 22
    const byWidth = (width - 6 * MONTH_GAP) / 7
    const byHeight = (height - (rows - 1) * MONTH_GAP) / rows
    return Math.max(12, Math.floor(Math.min(byWidth, byHeight)))
})

function formatDuration(seconds: number): string {
    const roundedSeconds = Math.max(0, Math.round(seconds))
    const hours = Math.floor(roundedSeconds / 3600)
    const minutes = Math.floor((roundedSeconds % 3600) / 60)
    if (hours > 0) return formatMessage(messages.hoursMinutes, { hours, minutes })
    if (minutes > 0) return formatMessage(messages.minutes, { minutes })
    return formatMessage(messages.seconds, { seconds: roundedSeconds })
}

/** "12 November · 2h 5m", then the instance played most and the launches. */
function dayLines(dateKey: string): string[] {
    const entry = dailyByDate.value.get(dateKey)
    const date = formatDetailDate(dateFromKey(dateKey))
    if (!entry || entry.played_seconds <= 0) {
        return [`${date} · ${formatMessage(messages.noActivity)}`]
    }
    const second = [
        entry.top_instance_name
            ? formatMessage(messages.mostPlayed, { name: entry.top_instance_name })
            : null,
        formatMessage(messages.sessions, { count: entry.session_count }),
    ].filter(Boolean)
    return [`${date} · ${formatDuration(entry.played_seconds)}`, second.join(' · ')]
}
function dayTooltip(dateKey: string) {
    return {
        content: dayLines(dateKey).join('\n'),
        popperClass: 'whitespace-pre-line !leading-snug',
    }
}

/** The one square in the map that takes Tab; the arrow keys move it. */
const tabStopKey = computed(() => {
    const keys = new Set(visibleDays.value.map((day) => day.dateKey))
    if (keys.has(focusKey.value)) return focusKey.value
    if (keys.has(selectedKey.value)) return selectedKey.value
    return visibleDays.value.at(-1)?.dateKey ?? null
})

function moveFocus(event: KeyboardEvent) {
    const current = (event.target as HTMLElement | null)?.dataset?.dateKey
    if (!current) return
    // Month and week grids run left to right by day; the band runs top to bottom.
    const steps: Record<string, number> =
        layout.value === 'month' || layout.value === 'week'
            ? { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 }
            : { ArrowUp: -1, ArrowDown: 1, ArrowLeft: -7, ArrowRight: 7 }
    const step = steps[event.key]
    if (step === undefined) return
    event.preventDefault()
    const date = dateFromKey(current)
    date.setDate(date.getDate() + step)
    const next = toDateKey(date)
    const cell = (event.currentTarget as HTMLElement).querySelector<HTMLElement>(
        `[data-date-key="${next}"]`,
    )
    if (!cell) return
    focusKey.value = next
    cell.focus()
}

async function refreshPlaytime() {
    const { start, end } = fetchRange.value
    const request = playtimeRequests.begin(`${start}\0${end}`)
    try {
        const result = await get_daily_playtime(start, end)
        const now = fetchRange.value
        if (playtimeRequests.isCurrent(request, `${now.start}\0${now.end}`)) {
            dailyPlaytime.value = result
            loadedRangeKey.value = `${now.start}\0${now.end}`
        }
    } catch (error) {
        const now = fetchRange.value
        if (playtimeRequests.isCurrent(request, `${now.start}\0${now.end}`)) {
            handleError(error)
            dailyPlaytime.value = []
            loadedRangeKey.value = null
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
    focusKey.value = dateKey
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

/** What every square says and does: its step, whether it can be picked, its label. */
function cellBindings(dateKey: string, drawn = true) {
    const pickable = drawn && dateKey <= todayKey
    return {
        class: {
            'home-heat-cell': true,
            'is-outside': !drawn,
            'is-future': drawn && dateKey > todayKey,
            'is-today': drawn && dateKey === todayKey,
            'is-selected': pickable && dateKey === selectedKey.value,
        },
        'data-level': drawn ? (pickable ? heatLevel(dateKey) : 0) : undefined,
        'data-date-key': pickable ? dateKey : undefined,
        tabindex: pickable ? (dateKey === tabStopKey.value ? 0 : -1) : -1,
        'aria-label': pickable ? dayLines(dateKey).join(', ') : undefined,
        'aria-pressed': pickable ? dateKey === selectedKey.value : undefined,
        'aria-hidden': drawn ? undefined : true,
        disabled: !pickable,
    }
}

watch(() => `${fetchRange.value.start}\0${fetchRange.value.end}`, refreshPlaytime, {
    immediate: true,
})
watch(selectedKey, refreshDayDetails, { immediate: true })
watch(instanceRevision, async () => {
    await refreshPlaytime()
    await refreshDayDetails()
})
</script>

<template>
    <HomeWidgetFrame
        class="home-calendar"
        :data-layout="layout"
        :title="title"
        :icon="CalendarIcon"
        :dense="layout === 'week' || layout === 'strip'"
    >
        <template v-if="layout !== 'week'" #actions>
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

        <!-- One tile: this week's hours and its seven squares; the month in a small note. -->
        <div v-if="layout === 'week'" class="home-calendar-week">
            <p class="home-calendar-stats">
                <span class="home-calendar-stat">
                    <strong>{{ monthStats.weekHours }}</strong>
                    <span>{{ monthStats.weekHoursUnit }}</span>
                </span>
                <span
                    v-tooltip="
                        formatMessage(messages.monthTotal, {
                            duration: formatDuration(monthSeconds),
                        })
                    "
                    class="home-calendar-stat-note"
                    >{{
                        monthIsEmpty ? formatMessage(messages.monthEmpty) : monthStats.monthHours
                    }}</span
                >
            </p>
            <div
                class="home-heat-week"
                role="grid"
                :aria-label="formatMessage(messages.calendar)"
                @keydown="moveFocus"
            >
                <span v-for="(day, index) in weekDays" :key="day.dateKey" class="home-heat-day">
                    <button
                        v-tooltip="day.dateKey <= todayKey ? dayTooltip(day.dateKey) : undefined"
                        type="button"
                        role="gridcell"
                        v-bind="cellBindings(day.dateKey)"
                        @click="selectDay(day.dateKey)"
                    />
                    <span class="home-heat-weekday" aria-hidden="true">{{
                        weekdayLabels[index]
                    }}</span>
                </span>
            </div>
        </div>

        <!-- One column: the month as a grid of weeks. -->
        <div v-else-if="layout === 'month'" class="home-calendar-month">
            <p v-if="!monthIsEmpty" class="home-calendar-stats">
                <span
                    v-tooltip="
                        formatMessage(messages.monthTotal, {
                            duration: formatDuration(monthSeconds),
                        })
                    "
                    class="home-calendar-stat"
                >
                    <strong>{{ monthStats.hours }}</strong>
                    <span>{{ monthStats.hoursUnit }}</span>
                </span>
                <span class="home-calendar-stat">
                    <strong>{{ monthStats.days }}</strong>
                    <span>{{ monthStats.daysUnit }}</span>
                </span>
            </p>
            <div
                class="home-heat-month home-heat-weekdays"
                :style="{ '--home-heat-cell': `${monthCell}px` }"
                aria-hidden="true"
            >
                <span v-for="(weekday, index) in weekdayLabels" :key="index">{{ weekday }}</span>
            </div>
            <div ref="monthBox" class="home-heat-month-box">
                <div
                    :key="periodStartKey"
                    class="home-heat-month home-heat-fade"
                    :style="{ '--home-heat-cell': `${monthCell}px` }"
                    role="grid"
                    :aria-label="formatMessage(messages.calendar)"
                    @keydown="moveFocus"
                >
                    <button
                        v-for="day in monthDays"
                        :key="day.dateKey"
                        v-tooltip="
                            day.inPeriod && day.dateKey <= todayKey
                                ? dayTooltip(day.dateKey)
                                : undefined
                        "
                        type="button"
                        role="gridcell"
                        v-bind="cellBindings(day.dateKey, day.inPeriod)"
                        @click="selectDay(day.dateKey)"
                    >
                        <span v-if="day.inPeriod" class="home-heat-number" aria-hidden="true">{{
                            day.date.getDate()
                        }}</span>
                    </button>
                </div>
            </div>
            <div
                class="home-calendar-day home-calendar-summary"
                role="group"
                :aria-label="formatMessage(messages.playedOn, { date: selectedDateLabel })"
            >
                <div class="home-calendar-summary-head">
                    <h3>{{ selectedDateLabel }}</h3>
                    <span v-if="dayTotalSeconds > 0">{{
                        formatShortDuration(dayTotalSeconds)
                    }}</span>
                    <div
                        class="home-heat-legend ml-auto"
                        role="img"
                        :aria-label="formatMessage(messages.legend)"
                    >
                        <span>{{ formatMessage(messages.legendLess) }}</span>
                        <span
                            v-for="level in HEAT_LEVELS + 1"
                            :key="level"
                            class="home-heat-swatch"
                            :data-level="level - 1"
                        />
                        <span>{{ formatMessage(messages.legendMore) }}</span>
                    </div>
                </div>
                <div v-if="daySummary" class="home-calendar-summary-row">
                    <InstanceIcon
                        :icon-path="daySummary.top.instance?.icon_path"
                        :instance-id="daySummary.top.entry.instance_id"
                        :loader="daySummary.top.instance?.loader"
                        size="20px"
                        class="shrink-0"
                    />
                    <span class="min-w-0 flex-1 truncate">{{ daySummary.text }}</span>
                    <template v-if="daySummary.top.instance">
                        <button
                            v-if="runningInstanceIds.includes(daySummary.top.instance.id)"
                            v-tooltip="formatMessage(messages.stopInstance)"
                            type="button"
                            class="home-calendar-play is-stop"
                            :aria-label="formatMessage(messages.stopInstance)"
                            @click="stopInstance(daySummary.top.instance)"
                        >
                            <StopCircleIcon aria-hidden="true" />
                        </button>
                        <button
                            v-else
                            v-tooltip="formatMessage(messages.playInstance)"
                            type="button"
                            class="home-calendar-play"
                            :aria-label="formatMessage(messages.playInstance)"
                            :disabled="
                                instanceLaunchStartedAt(daySummary.top.instance.id) !== undefined
                            "
                            @click="playInstance(daySummary.top.instance)"
                        >
                            <SpinnerIcon
                                v-if="
                                    instanceLaunchStartedAt(daySummary.top.instance.id) !==
                                    undefined
                                "
                                class="animate-spin"
                                aria-hidden="true"
                            />
                            <PlayIcon v-else aria-hidden="true" />
                        </button>
                    </template>
                </div>
                <p v-else class="home-calendar-summary-row is-quiet">
                    {{
                        monthIsEmpty
                            ? formatMessage(messages.monthEmpty)
                            : formatMessage(messages.noActivity)
                    }}
                </p>
            </div>
        </div>

        <!-- Wide: a band of weeks, newest on the right, like a contribution graph. -->
        <div v-else class="home-calendar-band" :class="{ 'is-strip': layout === 'strip' }">
            <div class="home-calendar-band-side">
                <p v-if="monthIsEmpty && layout === 'strip'" class="home-calendar-empty-note">
                    {{ formatMessage(messages.monthEmpty) }}
                </p>
                <p v-else-if="!monthIsEmpty" class="home-calendar-stats">
                    <span
                        v-tooltip="
                            formatMessage(messages.monthTotal, {
                                duration: formatDuration(monthSeconds),
                            })
                        "
                        class="home-calendar-stat"
                    >
                        <strong>{{ monthStats.hours }}</strong>
                        <span>{{ monthStats.hoursUnit }}</span>
                    </span>
                    <span class="home-calendar-stat">
                        <strong>{{ monthStats.days }}</strong>
                        <span>{{ monthStats.daysUnit }}</span>
                    </span>
                </p>
                <div
                    v-if="layout === 'strip'"
                    class="home-heat-legend"
                    role="img"
                    :aria-label="formatMessage(messages.legend)"
                >
                    <span>{{ formatMessage(messages.legendLess) }}</span>
                    <span
                        v-for="level in HEAT_LEVELS + 1"
                        :key="level"
                        class="home-heat-swatch"
                        :data-level="level - 1"
                    />
                    <span>{{ formatMessage(messages.legendMore) }}</span>
                </div>
            </div>
            <div ref="bandBox" class="home-heat-band-row">
                <dl v-if="bandGeometry.showInsights && bandInsights" class="home-calendar-insights">
                    <div>
                        <dt>{{ formatMessage(messages.insightLongest) }}</dt>
                        <dd>
                            {{ bandInsights.longest }}
                            <small>{{ bandInsights.longestDate }}</small>
                        </dd>
                    </div>
                    <div v-if="bandInsights.top">
                        <dt>{{ formatMessage(messages.insightTop) }}</dt>
                        <dd class="truncate">{{ bandInsights.top }}</dd>
                    </div>
                    <div>
                        <dt>{{ formatMessage(messages.insightStreak) }}</dt>
                        <dd>{{ bandInsights.streak }}</dd>
                    </div>
                </dl>
                <div
                    class="home-heat-band-box"
                    :style="{
                        width: `${bandGeometry.bandWidth}px`,
                        '--home-heat-cell': `${bandGeometry.cell}px`,
                        '--home-heat-gap': `${bandGeometry.gap}px`,
                        '--home-heat-labels': `${bandGeometry.labels}px`,
                        '--home-heat-month-row': `${bandGeometry.monthRow}px`,
                    }"
                >
                    <div class="home-heat-band-months" aria-hidden="true">
                        <span
                            v-for="month in bandMonthLabels"
                            :key="month.column"
                            :style="{
                                left: `calc(var(--home-heat-labels) + ${month.column} * (var(--home-heat-cell) + var(--home-heat-gap)))`,
                            }"
                            >{{ month.label }}</span
                        >
                    </div>
                    <div class="home-heat-band-body">
                        <div
                            v-if="layout === 'band'"
                            class="home-heat-band-days"
                            aria-hidden="true"
                        >
                            <span v-for="row in 7" :key="row">{{
                                row % 2 === 1 ? '' : weekdayLabels[row - 1]
                            }}</span>
                        </div>
                        <div
                            :key="rangeEndKey"
                            class="home-heat-band home-heat-fade"
                            role="grid"
                            :aria-label="formatMessage(messages.calendar)"
                            :style="{
                                gridTemplateColumns: `repeat(${bandWeeks.length}, var(--home-heat-cell))`,
                            }"
                            @keydown="moveFocus"
                        >
                            <template v-for="week in bandWeeks" :key="week[0].dateKey">
                                <button
                                    v-for="day in week"
                                    :key="day.dateKey"
                                    v-tooltip="
                                        day.dateKey <= todayKey
                                            ? dayTooltip(day.dateKey)
                                            : undefined
                                    "
                                    type="button"
                                    role="gridcell"
                                    v-bind="cellBindings(day.dateKey, day.dateKey <= rangeEndKey)"
                                    @click="selectDay(day.dateKey)"
                                />
                            </template>
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <template v-if="layout === 'band'">
            <div
                v-if="monthIsEmpty"
                class="home-calendar-day home-calendar-empty flex min-h-0 min-w-0 flex-1 items-center gap-3 pt-2"
            >
                <span class="home-calendar-empty-art" aria-hidden="true"><CalendarIcon /></span>
                <span class="flex min-w-0 flex-col gap-0.5">
                    <strong
                        class="text-[0.8125rem] font-medium leading-snug text-[var(--color-text-default)]"
                        >{{ formatMessage(messages.monthEmpty) }}</strong
                    >
                    <span class="text-xs leading-snug text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.monthEmptyHint)
                    }}</span>
                </span>
            </div>
            <div
                v-else
                class="home-calendar-day flex min-h-0 min-w-0 flex-1 flex-col gap-0.5 pt-2"
                role="group"
                :aria-label="formatMessage(messages.playedOn, { date: selectedDateLabel })"
            >
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
                        v-if="hiddenDetails > 0"
                        class="ml-auto shrink-0 font-semibold text-[var(--color-text-tertiary)]"
                        >{{ formatMessage(messages.more, { count: hiddenDetails }) }}</span
                    >
                </div>
                <p
                    v-if="dayDetails.length === 0"
                    class="m-0 px-1 text-[0.8125rem] text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noActivity) }}
                </p>
                <HomeFitList
                    v-else
                    v-slot="{ item: row }"
                    :items="detailRows"
                    :item-key="(row) => row.entry.instance_id"
                    variant="row"
                    :header-more="false"
                    @hidden="hiddenDetails = $event"
                >
                    <div class="home-calendar-row home-row group">
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
                    </div>
                </HomeFitList>
            </div>
            <div
                class="home-heat-legend is-footer"
                role="img"
                :aria-label="formatMessage(messages.legend)"
            >
                <span>{{ formatMessage(messages.legendLess) }}</span>
                <span
                    v-for="level in HEAT_LEVELS + 1"
                    :key="level"
                    class="home-heat-swatch"
                    :data-level="level - 1"
                />
                <span>{{ formatMessage(messages.legendMore) }}</span>
            </div>
        </template>
    </HomeWidgetFrame>
</template>

<style scoped>
/* The figures over the map: big numbers, small units. */
.home-calendar-stats {
    @apply m-0 mb-2 flex min-w-0 flex-wrap items-baseline gap-x-3.5 gap-y-1;
}

.home-calendar-stat {
    @apply inline-flex items-baseline gap-1;
}

/* Where the figures go when the month is empty, on the tiles without room for more. */
.home-calendar-empty-note {
    @apply m-0 text-[0.8125rem] font-medium leading-snug text-[var(--color-text-tertiary)];
}

.home-calendar-stat > strong {
    @apply text-[1.625rem] font-black leading-none tabular-nums tracking-tight text-[var(--color-text-primary)];
}

.home-calendar-stat > span {
    @apply text-xs font-bold text-[var(--color-text-tertiary)];
}

/*
 * The heatmap squares. Their colour is the information. The empty step is a
 * faint tint of the text colour, so it fades with the widget opacity like the
 * card does. Each step above mixes more of the accent into the card's own
 * opaque colour, up to the plain accent, and carries a hairline of the accent,
 * so played days stay apart from the empty ones and from a background picture
 * at any widget opacity.
 */
.home-calendar {
    --home-heat-base: var(--surface-3-opaque, var(--surface-3));
    --home-heat-0: var(
        --home-widget-tint-strong,
        color-mix(in srgb, var(--color-text-primary) 9%, transparent)
    );
    --home-heat-1: color-mix(in srgb, var(--color-brand) 26%, var(--home-heat-base));
    --home-heat-2: color-mix(in srgb, var(--color-brand) 48%, var(--home-heat-base));
    --home-heat-3: color-mix(in srgb, var(--color-brand) 72%, var(--home-heat-base));
    --home-heat-4: var(--color-brand);
    --home-heat-ring: var(--color-text-primary);
}

/*
 * Over a background picture the empty step also takes a share of the page
 * colour, the same share the text halo takes: none at full widget opacity, a
 * readable faint square when the widget is see-through.
 */
html.has-custom-background .home-calendar {
    --home-heat-0: color-mix(
        in srgb,
        var(--home-widget-halo, transparent) 55%,
        var(--home-widget-tint-strong, transparent)
    );
}

.home-heat-cell,
.home-heat-swatch {
    --home-heat-fill: var(--home-heat-0);
    border-radius: 0.25rem;
    background: var(--home-heat-fill);
}

:is(.home-heat-cell, .home-heat-swatch)[data-level='1'] {
    --home-heat-fill: var(--home-heat-1);
}

:is(.home-heat-cell, .home-heat-swatch)[data-level='2'] {
    --home-heat-fill: var(--home-heat-2);
}

:is(.home-heat-cell, .home-heat-swatch)[data-level='3'] {
    --home-heat-fill: var(--home-heat-3);
}

:is(.home-heat-cell, .home-heat-swatch)[data-level='4'] {
    --home-heat-fill: var(--home-heat-4);
}

:is(.home-heat-cell, .home-heat-swatch):is(
    [data-level='1'],
    [data-level='2'],
    [data-level='3'],
    [data-level='4']
) {
    outline: 1px solid color-mix(in srgb, var(--color-brand) 50%, transparent);
    outline-offset: -1px;
}

.home-heat-cell {
    @apply relative flex aspect-square min-w-0 cursor-pointer items-center justify-center border-0 p-0 outline-none;
    transition:
        box-shadow 100ms ease,
        transform 100ms ease;
}

.home-heat-cell:hover:not(:disabled) {
    transform: scale(1.12);
}

/* Days to come are drawn faint; days outside the month are not drawn. */
.home-heat-cell.is-future {
    @apply cursor-default;
    opacity: 0.4;
}

.home-heat-cell.is-outside {
    @apply cursor-default;
    visibility: hidden;
}

/* Today is a ring, never a fill, so it is not mistaken for a long day. */
.home-heat-cell.is-today {
    outline: none;
    box-shadow: inset 0 0 0 1.5px var(--home-heat-ring);
}

.home-heat-cell.is-selected,
.home-heat-cell:focus-visible {
    box-shadow:
        0 0 0 1.5px var(--home-heat-base),
        0 0 0 3px var(--color-brand);
    z-index: 1;
}

/*
 * One mark per cell: the selection ring replaces the level edge and today's
 * ring instead of stacking on them. Today then reads from its bold number.
 */
.home-heat-cell.is-selected,
.home-heat-cell:focus-visible {
    outline: none;
}

/* The date sits small and quiet in the square; the colour leads. */
.home-heat-number {
    @apply pointer-events-none font-bold leading-none tabular-nums;
    font-size: clamp(0.5625rem, calc(var(--home-heat-cell, 1rem) * 0.42), 0.75rem);
    color: color-mix(in srgb, var(--color-text-primary) 62%, transparent);
}

.home-heat-cell:is([data-level='1'], [data-level='2']) .home-heat-number {
    color: color-mix(in srgb, var(--color-text-primary) 88%, transparent);
}

.home-heat-cell:is([data-level='3'], [data-level='4']) .home-heat-number {
    color: color-mix(in srgb, var(--color-accent-contrast) 85%, transparent);
}

.home-heat-cell.is-today .home-heat-number {
    @apply font-black;
    color: var(--color-text-primary);
}

.home-heat-cell.is-today:is([data-level='3'], [data-level='4']) .home-heat-number {
    color: var(--color-accent-contrast);
}

/* One column: seven square columns, centred, with the weekday letters over them. */
/*
 * One column: the stats, then seven square columns as wide as the card (as
 * tall as it allows), left-aligned with the title, then the chosen day in one
 * line with the legend.
 */
.home-calendar-month {
    @apply flex min-h-0 min-w-0 flex-1 flex-col;
}

.home-calendar-month .home-calendar-stats {
    @apply mb-1.5;
}

.home-heat-month-box {
    @apply min-h-0 min-w-0 flex-1;
}

.home-heat-month {
    --home-heat-cell: 1.375rem;
    @apply grid gap-[3px];
    grid-template-columns: repeat(7, minmax(0, 1fr));
}

.home-heat-month .home-heat-cell {
    aspect-ratio: auto;
    height: var(--home-heat-cell);
}

.home-calendar-summary {
    @apply flex flex-none flex-col gap-0.5 pt-1;
}

.home-calendar-summary-head {
    @apply flex min-w-0 items-center gap-1.5 text-xs;
}

.home-calendar-summary-head > h3 {
    @apply m-0 min-w-0 truncate text-[0.8125rem] font-bold text-[var(--color-text-primary)];
}

.home-calendar-summary-head > span {
    @apply shrink-0 font-semibold tabular-nums text-[var(--color-text-default)];
}

.home-calendar-summary-row {
    @apply m-0 flex h-6 min-w-0 items-center gap-2 text-[0.8125rem] font-semibold text-[var(--color-text-primary)];
}

.home-calendar-summary-row.is-quiet {
    @apply font-medium text-[var(--color-text-tertiary)];
}

.home-calendar-summary-row .home-calendar-play {
    @apply size-6;
}

.home-heat-weekdays {
    @apply mb-[3px] text-center text-[0.625rem] font-bold leading-[0.75rem] text-[var(--color-text-tertiary)];
}

/* One tile: this week's seven squares, as large as the tile allows. */
.home-calendar-week {
    @apply flex min-h-0 flex-1 flex-col justify-between;
}

.home-calendar-week .home-calendar-stats {
    @apply mb-0;
}

.home-heat-week {
    @apply grid gap-1;
    grid-template-columns: repeat(7, minmax(0, 1.75rem));
    justify-content: space-between;
}

.home-heat-day {
    @apply flex min-w-0 flex-col items-center gap-0.5;
}

.home-heat-day .home-heat-cell {
    @apply w-full;
    border-radius: 0.375rem;
}

.home-heat-weekday {
    @apply text-[0.625rem] font-bold leading-3 text-[var(--color-text-tertiary)];
}

/* Wide: figures over (or beside) a band of week columns. */
.home-calendar-band {
    @apply flex min-w-0 flex-none flex-col;
}

.home-calendar-band.is-strip {
    @apply min-h-0 flex-1 flex-row gap-4;
}

.home-calendar-band.is-strip .home-calendar-band-side {
    @apply flex w-[7.5rem] flex-none flex-col justify-between;
}

.home-calendar-band.is-strip .home-calendar-stats {
    @apply mb-0 flex-col items-start gap-y-1.5;
}

.home-calendar-band.is-strip .home-calendar-stat > strong {
    @apply text-xl;
}

/* The band sits on the right at its own width; the figures about it take the rest. */
.home-heat-band-row {
    @apply flex min-w-0 items-end justify-end gap-5;
}

.home-calendar-band.is-strip .home-heat-band-row {
    @apply min-h-0 flex-1 items-stretch;
}

.home-heat-band-box {
    /* Measured sizes replace these defaults inline. */
    --home-heat-cell: 15px;
    --home-heat-gap: 3px;
    --home-heat-labels: 22px;
    --home-heat-month-row: 16px;
    @apply relative flex-none;
}

.home-calendar-insights {
    @apply m-0 flex min-w-0 flex-1 flex-col justify-end gap-2.5 self-stretch;
}

.home-calendar-band.is-strip .home-calendar-insights {
    @apply justify-center gap-1;
}

.home-calendar-insights > div {
    @apply flex min-w-0 flex-col;
}

.home-calendar-band.is-strip .home-calendar-insights > div {
    @apply flex-row items-baseline gap-2;
}

.home-calendar-insights dt {
    @apply text-[0.625rem] font-extrabold uppercase tracking-[0.08em] text-[var(--color-text-tertiary)];
}

.home-calendar-insights dd {
    @apply m-0 min-w-0 text-[0.9375rem] font-extrabold leading-snug text-[var(--color-text-primary)];
}

.home-calendar-band.is-strip .home-calendar-insights dd {
    @apply truncate text-[0.8125rem];
}

.home-calendar-insights small {
    @apply ml-1 text-[0.6875rem] font-semibold text-[var(--color-text-tertiary)];
}

/* One tile: this week's hours big, the month's beside them small. */
.home-calendar-stat-note {
    @apply ml-auto truncate text-xs font-bold text-[var(--color-text-tertiary)];
}

.home-heat-band-months {
    @apply relative;
    height: var(--home-heat-month-row);
}

.home-heat-band-months > span {
    @apply absolute top-0 whitespace-nowrap text-[0.625rem] font-bold leading-3 text-[var(--color-text-tertiary)];
}

.home-heat-band-body {
    @apply flex justify-end;
}

.home-heat-band-days {
    @apply grid flex-none text-[0.5625rem] font-bold text-[var(--color-text-tertiary)];
    width: calc(var(--home-heat-labels) - 4px);
    margin-right: 4px;
    grid-template-rows: repeat(7, var(--home-heat-cell));
    row-gap: var(--home-heat-gap);
    line-height: var(--home-heat-cell);
}

.home-heat-band {
    @apply grid;
    grid-auto-flow: column;
    grid-template-rows: repeat(7, var(--home-heat-cell));
    gap: var(--home-heat-gap);
}

.home-heat-band .home-heat-cell {
    border-radius: 0.1875rem;
}

/* The scale, at the foot of the card: less, the five steps, more. */
.home-heat-legend {
    @apply flex flex-none items-center gap-[3px] text-[0.625rem] font-bold text-[var(--color-text-tertiary)];
}

.home-heat-legend.is-footer {
    @apply mt-1.5 justify-end;
}

.home-heat-legend > span:first-child {
    @apply mr-1;
}

.home-heat-legend > span:last-child {
    @apply ml-1;
}

.home-heat-swatch {
    @apply block size-2.5;
    border-radius: 0.1875rem;
}

/* A new month or range fades its squares in. */
.home-heat-fade {
    animation: home-heat-in 220ms ease both;
}

@keyframes home-heat-in {
    from {
        opacity: 0;
    }
}

.home-calendar-day {
    border-top: 1px solid var(--home-widget-edge);
}

/* An empty month: the same quiet icon in a hairline ring as the empty widgets. */
.home-calendar-empty-art {
    @apply ml-1 flex size-9 shrink-0 items-center justify-center rounded-full text-[var(--color-text-tertiary)];
    border: 1px solid var(--home-widget-edge);
}

.home-calendar-empty-art svg {
    @apply size-4;
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
    .home-heat-cell {
        transition: none;
    }

    .home-heat-cell:hover:not(:disabled) {
        transform: none;
    }

    .home-heat-fade {
        animation: none;
    }
}
</style>
