<script setup lang="ts">
import {
    CalendarIcon,
    ChevronLeftIcon,
    ChevronRightIcon,
    PlayIcon,
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

function heatmapLevelClass(dateKey: string): string {
    const level = getPlaytimeLevel(dailyByDate.value.get(dateKey)?.played_seconds ?? 0)
    return level === 0 ? 'bg-surface-4' : `home-calendar-level-${level}`
}
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
    <section class="flex min-w-0 min-h-0 h-full flex-col gap-2.5 overflow-hidden p-2">
        <header class="flex min-w-0 h-8 flex-none items-center gap-2">
            <div class="home-calendar-title flex min-w-0 items-center gap-2">
                <CalendarIcon class="size-5 shrink-0 text-brand" aria-hidden="true" />
                <h2>{{ formatMessage(messages.calendar) }}</h2>
            </div>
            <div class="ml-auto flex min-w-0 items-center gap-0.5">
                <Button
                    v-tooltip="formatMessage(messages.previousMonth)"
                    type="quiet"
                    size="2xs"
                    circular
                    icon-only
                    @click="movePeriod(-1)"
                    ><ChevronLeftIcon />
                </Button>
                <Button
                    v-tooltip="formatMessage(messages.thisMonth)"
                    class="home-calendar-period min-w-0"
                    type="quiet"
                    size="2xs"
                    @click="goToThisMonth"
                    >{{ periodLabel }}
                </Button>
                <Button
                    v-tooltip="formatMessage(messages.nextMonth)"
                    type="quiet"
                    size="2xs"
                    circular
                    icon-only
                    :disabled="!canGoForward"
                    @click="movePeriod(1)"
                    ><ChevronRightIcon />
                </Button>
            </div>
        </header>
        <div class="flex-none">
            <div
                class="mb-1 grid grid-cols-7 gap-[0.1875rem] text-center text-xs font-semibold text-[var(--color-text-tertiary)]"
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
                    class="home-calendar-cell h-[1.4rem] border border-solid rounded-[var(--radius-sm)] text-[0.6875rem] font-semibold outline-none p-0"
                    :class="{
                        'text-[var(--color-text-primary)]': !(
                            day.inPeriod && day.dateKey > todayKey
                        ),
                        'text-[var(--color-text-tertiary)] opacity-50 cursor-default':
                            day.inPeriod && day.dateKey > todayKey,
                        'cursor-default': !day.inPeriod,
                        'cursor-pointer': day.inPeriod && day.dateKey <= todayKey,
                        'border-transparent': !(day.inPeriod && day.dateKey === todayKey),
                        'border-brand': day.inPeriod && day.dateKey === todayKey,
                        'bg-transparent': !(day.inPeriod && day.dateKey <= todayKey),
                        'home-calendar-cell-selected': day.inPeriod && day.dateKey === selectedKey,
                        [heatmapLevelClass(day.dateKey)]: day.inPeriod && day.dateKey <= todayKey,
                    }"
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
        <div
            class="flex min-w-0 min-h-[3.75rem] flex-1 flex-col gap-1.5 overflow-y-auto border-t border-divider pt-2.5"
        >
            <h3 class="m-0 text-sm font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.playedOn, { date: selectedDateLabel }) }}
            </h3>
            <p v-if="dayDetails.length === 0" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.noActivity) }}
            </p>
            <ul v-else class="m-0 flex list-none flex-col p-0">
                <li
                    v-for="row in detailRows"
                    :key="row.entry.instance_id"
                    class="group flex min-w-0 items-center gap-2.5 rounded-lg px-1.5 py-1.5 transition-colors hover:bg-surface-4"
                >
                    <InstanceIcon
                        :icon-path="row.instance?.icon_path"
                        :instance-id="row.entry.instance_id"
                        :loader="row.instance?.loader"
                        size="36px"
                        class="shrink-0"
                    />
                    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                        <span
                            class="truncate text-sm font-semibold text-[var(--color-text-primary)]"
                        >
                            {{ row.instance?.name ?? row.entry.instance_name }}
                        </span>
                        <span class="truncate text-xs text-[var(--color-text-tertiary)]">
                            {{ formatDuration(row.entry.played_seconds) }}
                        </span>
                    </div>
                    <div v-if="row.instance" class="ml-auto shrink-0">
                        <Button
                            v-if="runningInstanceIds.includes(row.instance.id)"
                            v-tooltip="formatMessage(messages.stopInstance)"
                            type="quiet"
                            size="2xs"
                            circular
                            icon-only
                            class="!text-red"
                            @click="stopInstance(row.instance)"
                            ><StopCircleIcon />
                        </Button>
                        <Button
                            v-else
                            v-tooltip="formatMessage(messages.playInstance)"
                            type="quiet"
                            size="2xs"
                            circular
                            icon-only
                            class="!text-brand opacity-60 transition-opacity group-hover:opacity-100"
                            @click="playInstance(row.instance)"
                            ><PlayIcon />
                        </Button>
                    </div>
                </li>
            </ul>
        </div>
    </section>
</template>

<style scoped>
.home-calendar-title h2 {
    overflow: hidden;
    margin: 0;
    color: var(--color-text-primary);
    font-size: 1rem;
    font-weight: 700;
    letter-spacing: 0;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.home-calendar-period :deep(button) {
    max-width: 7.5rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.home-calendar-cell {
    transition:
        box-shadow 100ms ease,
        background-color 100ms ease;
}

.home-calendar-cell:focus-visible {
    box-shadow: 0 0 0 2px var(--color-brand);
}

.home-calendar-cell-selected {
    box-shadow: 0 0 0 2px var(--color-brand);
}

.home-calendar-level-1 {
    background: color-mix(in oklab, var(--color-brand) 28%, var(--surface-4));
}
.home-calendar-level-2 {
    background: color-mix(in oklab, var(--color-brand) 48%, var(--surface-4));
}
.home-calendar-level-3 {
    background: color-mix(in oklab, var(--color-brand) 70%, var(--surface-4));
}
.home-calendar-level-4 {
    background: var(--color-brand);
    color: var(--color-accent-contrast);
}
</style>
