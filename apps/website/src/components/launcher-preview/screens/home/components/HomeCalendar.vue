<script setup lang="ts">
import CalendarIcon from '@modrinth/assets/icons/calendar.svg?component'
import ChevronLeftIcon from '@modrinth/assets/icons/chevron-left.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import StopCircleIcon from '@modrinth/assets/icons/stop-circle.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import type { GameInstance } from '../../../lib/types'
import type { HomeWidgetSize } from '../home-dashboard'
import {
    buildHeatmapDays,
    dateFromKey,
    endOfPeriod,
    getPlaytimeLevel,
    shiftPeriod,
    startOfPeriod,
    toDateKey,
} from '../home-utils'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'
import InstanceIcon from './InstanceIcon.vue'

const props = defineProps<{
    instances: GameInstance[]
    dashboardSize?: HomeWidgetSize | null
}>()

const { formatMessage } = useVIntl()
const runtime = useHomeRuntime()
const { runningInstanceIds } = runtime
const formatPeriod = useFormatDateTime({ month: 'long', year: 'numeric' })
const formatWeekday = useFormatDateTime({ weekday: 'narrow' })
const formatDetailDate = useFormatDateTime({ month: 'long', day: 'numeric' })
const formatFullDate = useFormatDateTime({ dateStyle: 'full' })
const messages = homeMessages

const todayKey = toDateKey(new Date())
const anchor = ref(new Date())
const selectedKey = ref(todayKey)

const periodStart = computed(() => startOfPeriod(anchor.value, 'month'))
const periodEnd = computed(() => endOfPeriod(anchor.value, 'month'))
const periodLabel = computed(() => formatPeriod(periodStart.value))
const days = computed(() => buildHeatmapDays(anchor.value, 'month'))
const weekdayLabels = computed(() =>
    Array.from({ length: 7 }, (_, index) => formatWeekday(new Date(2024, 0, index + 1, 12))),
)
/** `get_daily_playtime(start, end)` for the month on screen. */
const dailyPlaytime = computed(() =>
    runtime.getDailyPlaytime(toDateKey(periodStart.value), toDateKey(periodEnd.value)),
)
/** `get_daily_playtime_details(date)` for the selected day. */
const dayDetails = computed(() => runtime.getDailyPlaytimeDetails(selectedKey.value))
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
    if (hours > 0) return formatMessage(messages.playtimeHoursMinutes, { hours, minutes })
    if (minutes > 0) return formatMessage(messages.playtimeMinutes, { minutes })
    return formatMessage(messages.playtimeSeconds, { seconds: roundedSeconds })
}

function tooltipLinesFor(dateKey: string): string[] {
    const entry = dailyByDate.value.get(dateKey)
    const lines = [formatFullDate(dateFromKey(dateKey))]
    if (entry && entry.played_seconds > 0) {
        lines.push(
            formatDuration(entry.played_seconds),
            formatMessage(messages.playtimeSessions, { count: entry.session_count }),
        )
        if (entry.top_instance_name) {
            lines.push(
                formatMessage(messages.playtimeMostPlayed, { name: entry.top_instance_name }),
            )
        }
    } else {
        lines.push(formatMessage(messages.calendarNoActivity))
    }
    return lines
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
        await runtime.run(instance)
    } catch (error) {
        runtime.handleError(error)
    }
}

async function stopInstance(instance: GameInstance) {
    await runtime.kill(instance.id).catch(runtime.handleError)
}
</script>

<template>
    <section class="flex h-full min-h-0 min-w-0 flex-col gap-2.5 overflow-hidden p-2">
        <header class="flex h-8 min-w-0 flex-none items-center gap-2">
            <div class="home-calendar-title flex min-w-0 items-center gap-2">
                <CalendarIcon class="size-5 shrink-0 text-brand" aria-hidden="true" />
                <h2>{{ formatMessage(messages.calendarTitle) }}</h2>
            </div>
            <div class="ml-auto flex min-w-0 items-center gap-0.5">
                <Button
                    v-tooltip="formatMessage(messages.calendarPrevious)"
                    type="quiet"
                    size="2xs"
                    circular
                    icon-only
                    @click="movePeriod(-1)"
                    ><ChevronLeftIcon />
                </Button>
                <Button
                    v-tooltip="formatMessage(messages.calendarThisMonth)"
                    class="home-calendar-period min-w-0"
                    type="quiet"
                    size="2xs"
                    @click="goToThisMonth"
                    >{{ periodLabel }}
                </Button>
                <Button
                    v-tooltip="formatMessage(messages.calendarNext)"
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
                :aria-label="formatMessage(messages.calendarTitle)"
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
                    class="home-calendar-cell h-[1.4rem] rounded-[var(--radius-sm)] border border-solid p-0 text-[0.6875rem] font-semibold outline-none"
                    :class="{
                        'text-[var(--color-text-primary)]': !(
                            day.inPeriod && day.dateKey > todayKey
                        ),
                        'cursor-default text-[var(--color-text-tertiary)] opacity-50':
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
            class="flex min-h-[3.75rem] min-w-0 flex-1 flex-col gap-1.5 overflow-y-auto border-t border-divider pt-2.5"
        >
            <h3 class="m-0 text-sm font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.calendarPlayedOn, { date: selectedDateLabel }) }}
            </h3>
            <p v-if="dayDetails.length === 0" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.calendarNoActivity) }}
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
                            v-tooltip="formatMessage(messages.calendarStop)"
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
                            v-tooltip="formatMessage(messages.calendarPlay)"
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
