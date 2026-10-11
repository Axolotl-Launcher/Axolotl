<script setup lang="ts">
import { CalendarIcon, ChevronLeftIcon, ChevronRightIcon } from '@modrinth/assets'
import { Button, defineMessages, useFormatDateTime, useVIntl } from '@modrinth/ui'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import { dateKey, getAlmanac, isAuspicious } from './almanac'
import { useAlmanacTerms } from './almanac-terms'
import AlmanacDetailModal from './AlmanacDetailModal.vue'
import {
    HOME_ALMANAC_DEFAULT_SHOW_ACTIVITIES,
    HOME_ALMANAC_DEFAULT_TERM_STYLE,
    type HomeAlmanacTermStyle,
    type HomeWidgetSize,
} from './home-dashboard'

const props = defineProps<{
    dashboardSize?: HomeWidgetSize | null
    showHours?: boolean | null
    showActivities?: boolean | null
    tagLimit?: number | null
    termStyle?: HomeAlmanacTermStyle | null
}>()

const { formatMessage } = useVIntl()
const formatDayLabel = useFormatDateTime({ month: 'long', day: 'numeric' })
const formatWeekday = useFormatDateTime({ weekday: 'long' })

const messages = defineMessages({
    title: { id: 'app.home.almanac.title', defaultMessage: 'Almanac' },
    today: { id: 'app.home.almanac.today', defaultMessage: 'Today' },
    previousDay: { id: 'app.home.almanac.previous-day', defaultMessage: 'Previous day' },
    nextDay: { id: 'app.home.almanac.next-day', defaultMessage: 'Next day' },
    yi: { id: 'app.home.almanac.yi', defaultMessage: 'Do' },
    ji: { id: 'app.home.almanac.ji', defaultMessage: 'Avoid' },
    more: { id: 'app.home.almanac.more', defaultMessage: 'More' },
    zhishen: { id: 'app.home.almanac.zhishen', defaultMessage: 'Day officer' },
    zhixing: { id: 'app.home.almanac.zhixing', defaultMessage: 'Twelve officers' },
    nayin: { id: 'app.home.almanac.nayin', defaultMessage: 'Element' },
    chongsha: { id: 'app.home.almanac.chongsha', defaultMessage: 'Clash' },
    sha: { id: 'app.home.almanac.sha', defaultMessage: 'Sha' },
    xiu: { id: 'app.home.almanac.xiu', defaultMessage: 'Mansion' },
    taishen: { id: 'app.home.almanac.taishen', defaultMessage: 'Fetal god' },
    pengzu: { id: 'app.home.almanac.pengzu', defaultMessage: 'Peng Zu taboos' },
    lucky: { id: 'app.home.almanac.lucky', defaultMessage: 'Auspicious' },
    unlucky: { id: 'app.home.almanac.unlucky', defaultMessage: 'Inauspicious' },
    hours: { id: 'app.home.almanac.hours', defaultMessage: 'Hourly fortune' },
    viewDetail: { id: 'app.home.almanac.view-detail', defaultMessage: 'View full almanac' },
    sourceNote: {
        id: 'app.home.almanac.source-note',
        defaultMessage:
            'Auspicious and inauspicious activities follow a local almanac table and may differ from other almanac sources.',
    },
})

const size = computed<HomeWidgetSize>(() => props.dashboardSize ?? '2x2')
/** Two rows are available, so the Peng Zu line fits. */
const isTall = computed(() => size.value === '1x2' || size.value === '2x2')
/** Four columns of width: the fact grid and the hourly strip fit. */
const isWide = computed(() => size.value === '2x2')
/** Only one row of height: header, a single meta line and the two tag rows fit. */
const isShort = computed(() => size.value === '2x1')
/**
 * How many activity tags each list shows. The widget options can raise it, but
 * the 2x1 card keeps a single row per list so anything past three would clip.
 */
const tagLimit = computed(() => {
    const requested = props.tagLimit ?? (isWide.value ? 6 : 3)
    return isShort.value ? Math.min(requested, 3) : requested
})
/** The hourly strip only fits the wide card and can be disabled in the options. */
const showHours = computed(() => isWide.value && (props.showHours ?? true))
/**
 * The activity lists stay hidden by default: that column is built from a
 * different almanac table than the mainstream calendars, so it can contradict
 * them. Everything else on the card is shared by every source.
 */
const showActivities = computed(() => props.showActivities ?? HOME_ALMANAC_DEFAULT_SHOW_ACTIVITIES)
/** Without the activity rows the tall cards fall back to the fact grid. */
const showFactGrid = computed(() => isWide.value || (isTall.value && !showActivities.value))
/** The 2x1 card has one spare row, so it shows a single line of facts instead. */
const showFactLine = computed(() => isShort.value && !showActivities.value)

/** How the activity terms are spelled out; see `useAlmanacTerms`. */
const termStyle = computed(() => props.termStyle ?? HOME_ALMANAC_DEFAULT_TERM_STYLE)
const { termLabel } = useAlmanacTerms(() => termStyle.value)

const detailModal = ref<InstanceType<typeof AlmanacDetailModal>>()

function openDetail() {
    detailModal.value?.show()
}

const today = ref(new Date())
const selected = ref(new Date())

const almanac = computed(() => getAlmanac(selected.value))
const isToday = computed(() => dateKey(selected.value) === dateKey(today.value))
const dayLabel = computed(() => formatDayLabel(selected.value))
const weekdayLabel = computed(() => formatWeekday(selected.value))
const dateLabel = computed(() => almanac.value.solarLabel)

const yiTags = computed(() => almanac.value.yi.slice(0, tagLimit.value))
const jiTags = computed(() => almanac.value.ji.slice(0, tagLimit.value))
const hiddenYi = computed(() => Math.max(0, almanac.value.yi.length - tagLimit.value))
const hiddenJi = computed(() => Math.max(0, almanac.value.ji.length - tagLimit.value))

/** One truncated line for the 2x1 card, so the meta block never wraps. */
const shortMeta = computed(() => {
    const parts = [dayLabel.value, weekdayLabel.value, almanac.value.lunarLabel]
    if (almanac.value.ganZhiDay) parts.push(almanac.value.ganZhiDay)
    return parts.filter(Boolean).join(' · ')
})

const pengZu = computed(() => {
    const parts = [almanac.value.pengZuGan, almanac.value.pengZuZhi].filter(Boolean)
    return parts.join(' · ')
})

const facts = computed(() => {
    const day = almanac.value
    const sha = day.sha ? `${formatMessage(messages.sha)} ${day.sha}` : ''
    return [
        {
            key: 'zhishen',
            label: formatMessage(messages.zhishen),
            value: day.zhiShen,
            tone: luckTone(day.zhiShenLuck),
        },
        {
            key: 'zhixing',
            label: formatMessage(messages.zhixing),
            value: day.zhiXing,
            tone: 'neutral' as const,
        },
        {
            key: 'nayin',
            label: formatMessage(messages.nayin),
            value: day.naYin,
            tone: 'neutral' as const,
        },
        {
            key: 'chongsha',
            label: formatMessage(messages.chongsha),
            value: [day.chong, sha].filter(Boolean).join(' · '),
            tone: 'neutral' as const,
        },
        {
            key: 'xiu',
            label: formatMessage(messages.xiu),
            value: day.xiu,
            tone: luckTone(day.xiuLuck),
        },
        {
            key: 'taishen',
            label: formatMessage(messages.taishen),
            value: day.taiShen,
            tone: 'neutral' as const,
        },
    ]
})

/**
 * The 2x1 card has room for one line, so it shows the facts every source
 * agrees on instead of the activity tags.
 */
const compactFactKeys = ['zhishen', 'chongsha', 'zhixing']
const compactFacts = computed(() =>
    facts.value.filter((fact) => compactFactKeys.includes(fact.key)),
)

function luckTone(luck: string): 'good' | 'bad' | 'neutral' {
    if (!luck) return 'neutral'
    return isAuspicious(luck) ? 'good' : 'bad'
}

function hourLabel(lucky: boolean) {
    return formatMessage(lucky ? messages.lucky : messages.unlucky)
}

function shiftDay(offset: number) {
    const current = selected.value
    // Noon keeps the calendar day stable across daylight-saving transitions.
    selected.value = new Date(
        current.getFullYear(),
        current.getMonth(),
        current.getDate() + offset,
        12,
    )
}

function goToToday() {
    selected.value = new Date()
}

/** Follows the clock when the shown day was today and the date rolled over. */
function refreshToday() {
    const previousKey = dateKey(today.value)
    const now = new Date()
    if (dateKey(now) === previousKey) return

    const following = dateKey(selected.value) === previousKey
    today.value = now
    if (following) selected.value = now
}

let midnightTimer: ReturnType<typeof setTimeout> | undefined

function scheduleMidnightRefresh() {
    if (midnightTimer) clearTimeout(midnightTimer)
    const now = new Date()
    const nextMidnight = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 0, 0, 1)
    midnightTimer = setTimeout(
        () => {
            refreshToday()
            scheduleMidnightRefresh()
        },
        Math.max(1000, nextMidnight.getTime() - now.getTime()),
    )
}

function handleWindowFocus() {
    refreshToday()
    scheduleMidnightRefresh()
}

onMounted(() => {
    refreshToday()
    scheduleMidnightRefresh()
    window.addEventListener('focus', handleWindowFocus)
})

onBeforeUnmount(() => {
    if (midnightTimer) clearTimeout(midnightTimer)
    window.removeEventListener('focus', handleWindowFocus)
})
</script>

<template>
    <section
        class="home-almanac flex min-w-0 min-h-0 h-full cursor-pointer flex-col overflow-hidden"
        :class="isShort ? 'gap-1 p-1.5' : 'gap-2 p-2'"
        :data-size="dashboardSize ?? undefined"
        @click="openDetail"
    >
        <header
            class="flex min-w-0 h-8 flex-none items-center"
            :class="isShort ? 'gap-1' : 'gap-2'"
        >
            <div class="home-almanac-title flex min-w-0 items-center gap-2">
                <CalendarIcon class="size-5 shrink-0 text-brand" aria-hidden="true" />
                <h2 class="home-widget-heading" :title="formatMessage(messages.sourceNote)">
                    {{ formatMessage(messages.title) }}
                </h2>
            </div>
            <div class="ml-auto flex min-w-0 items-center gap-0.5" @click.stop>
                <!--
                    The 2x1 card has no room for the date nav: the same date is
                    already spelled out in the meta line below the header.
                -->
                <template v-if="!isShort">
                    <Button
                        v-tooltip="formatMessage(messages.previousDay)"
                        type="quiet"
                        size="2xs"
                        circular
                        icon-only
                        @click="shiftDay(-1)"
                        ><ChevronLeftIcon />
                    </Button>
                    <span
                        class="home-almanac-date min-w-0 truncate text-xs font-semibold text-[var(--color-text-tertiary)]"
                        >{{ dateLabel }}</span
                    >
                    <Button
                        v-tooltip="formatMessage(messages.nextDay)"
                        type="quiet"
                        size="2xs"
                        circular
                        icon-only
                        @click="shiftDay(1)"
                        ><ChevronRightIcon />
                    </Button>
                </template>
                <Button v-if="!isToday" type="quiet" size="2xs" @click="goToToday">{{
                    formatMessage(messages.today)
                }}</Button>
            </div>
        </header>

        <div class="flex min-w-0 min-h-0 flex-1 flex-col gap-1.5 overflow-hidden">
            <div v-if="isShort" class="home-almanac-sub min-w-0 flex-none truncate">
                {{ shortMeta }}
            </div>
            <div v-else class="flex-none">
                <div class="home-almanac-day text-[var(--color-text-primary)]">
                    {{ dayLabel }}
                    <span class="home-almanac-sub">
                        {{ weekdayLabel }}
                        <template v-if="almanac.zodiac"> · {{ almanac.zodiac }}</template>
                    </span>
                </div>
                <div class="home-almanac-sub truncate">
                    {{ almanac.lunarLabel }}
                    <template v-if="almanac.ganZhiDay">
                        · {{ almanac.ganZhiYear }} {{ almanac.ganZhiMonth }}
                        {{ almanac.ganZhiDay }}
                    </template>
                </div>
            </div>

            <div v-if="showActivities" class="home-almanac-row flex-none">
                <span class="home-almanac-row-label home-almanac-good">{{
                    formatMessage(messages.yi)
                }}</span>
                <span
                    class="home-almanac-tags flex min-w-0 items-center"
                    :class="[
                        'flex-wrap gap-1',
                        isShort && 'flex-nowrap gap-0.5 overflow-hidden',
                        isWide && 'home-almanac-tags-clamped',
                    ]"
                >
                    <span
                        v-for="tag in yiTags"
                        :key="`yi-${tag}`"
                        class="home-almanac-tag home-almanac-tag-good"
                        >{{ termLabel(tag) }}</span
                    >
                    <span
                        v-if="hiddenYi > 0"
                        v-tooltip="formatMessage(messages.more)"
                        class="home-almanac-tag home-almanac-tag-muted"
                        >+{{ hiddenYi }}</span
                    >
                </span>
            </div>

            <div v-if="showActivities" class="home-almanac-row flex-none">
                <span class="home-almanac-row-label home-almanac-bad">{{
                    formatMessage(messages.ji)
                }}</span>
                <span
                    class="home-almanac-tags flex min-w-0 items-center"
                    :class="[
                        'flex-wrap gap-1',
                        isShort && 'flex-nowrap gap-0.5 overflow-hidden',
                        isWide && 'home-almanac-tags-clamped',
                    ]"
                >
                    <span
                        v-for="tag in jiTags"
                        :key="`ji-${tag}`"
                        class="home-almanac-tag home-almanac-tag-bad"
                        >{{ termLabel(tag) }}</span
                    >
                    <span
                        v-if="hiddenJi > 0"
                        v-tooltip="formatMessage(messages.more)"
                        class="home-almanac-tag home-almanac-tag-muted"
                        >+{{ hiddenJi }}</span
                    >
                </span>
            </div>

            <div
                v-if="showFactGrid"
                class="home-almanac-facts"
                :class="!isWide && 'home-almanac-facts-narrow'"
            >
                <div
                    v-for="fact in facts"
                    :key="fact.key"
                    class="home-almanac-fact"
                    :class="`home-almanac-${fact.tone}`"
                >
                    <span class="home-almanac-fact-label">{{ fact.label }}</span>
                    <span class="home-almanac-fact-value truncate">{{ fact.value }}</span>
                </div>
            </div>

            <div v-if="showFactLine" class="home-almanac-compact-facts min-w-0 flex-none truncate">
                <template v-for="(fact, index) in compactFacts" :key="fact.key">
                    <span v-if="index > 0" class="home-almanac-sub"> · </span>
                    <span class="home-almanac-sub">{{ fact.label }}</span>
                    <span class="home-almanac-fact-value" :class="`home-almanac-${fact.tone}`">{{
                        fact.value
                    }}</span>
                </template>
            </div>

            <div v-if="showHours && almanac.hours.length > 0" class="flex-none">
                <div class="home-almanac-sub mb-1">{{ formatMessage(messages.hours) }}</div>
                <div class="home-almanac-hours">
                    <div
                        v-for="hour in almanac.hours"
                        :key="hour.zhi"
                        class="home-almanac-hour"
                        :class="hour.lucky ? 'home-almanac-good' : 'home-almanac-bad'"
                        :title="`${hour.ganZhi} ${hourLabel(hour.lucky)}`"
                    >
                        <span class="home-almanac-hour-zhi">{{ hour.zhi }}</span>
                        <span class="home-almanac-hour-luck">{{ hourLabel(hour.lucky) }}</span>
                    </div>
                </div>
            </div>

            <div v-if="isTall && pengZu" class="home-almanac-foot mt-auto flex-none">
                <span class="home-almanac-sub shrink-0">{{ formatMessage(messages.pengzu) }}</span>
                <span class="home-almanac-sub min-w-0 truncate">{{ pengZu }}</span>
                <button type="button" class="home-almanac-link text-brand" @click.stop="openDetail">
                    {{ formatMessage(messages.viewDetail) }}
                </button>
            </div>
        </div>
    </section>

    <AlmanacDetailModal
        ref="detailModal"
        :date="selected"
        :show-activities="showActivities"
        :term-style="termStyle"
    />
</template>

<style scoped>
.home-almanac-title h2 {
    overflow: hidden;
    margin: 0;
    color: var(--color-text-primary);
    font-size: 1rem;
    font-weight: 700;
    letter-spacing: 0;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.home-almanac-date {
    max-width: 6.5rem;
}

.home-almanac-day {
    font-size: 1.0625rem;
    font-weight: 700;
    line-height: 1.2;
}

.home-almanac-sub {
    color: var(--color-text-tertiary);
    font-size: 0.6875rem;
    font-weight: 500;
}

.home-almanac-row {
    display: flex;
    min-width: 0;
    align-items: flex-start;
    gap: 0.375rem;
}

.home-almanac-row-label {
    display: flex;
    width: 1.25rem;
    height: 1.25rem;
    flex: none;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    font-size: 0.6875rem;
    font-weight: 700;
}

.home-almanac-tag {
    border: 1px solid transparent;
    border-radius: 999px;
    padding: 0.0625rem 0.375rem;
    font-size: 0.6875rem;
    font-weight: 600;
    line-height: 1.35;
    white-space: nowrap;
}

.home-almanac-facts {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.375rem;
}

.home-almanac-fact {
    display: flex;
    min-width: 0;
    flex-direction: column;
    border-radius: var(--radius-sm);
    background: var(--surface-4);
    padding: 0.25rem 0.375rem;
}

.home-almanac-fact-label {
    color: var(--color-text-tertiary);
    font-size: 0.5625rem;
    font-weight: 500;
}

.home-almanac-fact-value {
    font-size: 0.6875rem;
    font-weight: 600;
}

/*
 * The tag rows are clamped to two rows and the fact grid absorbs whatever is
 * left, so the hourly strip and the footer always stay inside the card.
 */
.home-almanac-facts {
    min-height: 0;
    flex: 0 1 auto;
    overflow: hidden;
}

/*
 * The 1x2 card is a single column wide, so its fact grid falls back to two
 * columns when the activity rows are hidden.
 */
.home-almanac-facts-narrow {
    grid-template-columns: repeat(2, minmax(0, 1fr));
}

/* One summary line for the 2x1 card when the activity rows are hidden. */
.home-almanac-compact-facts {
    display: flex;
    align-items: baseline;
    gap: 0.125rem;
}

/*
 * The 2x2 card has the fact grid and the hourly strip competing for the same
 * height, so its tag rows stop after two lines. The 1x2 card is narrow but
 * tall, and the 2x1 card keeps a single nowrap line, so neither is clamped.
 */
.home-almanac-tags-clamped {
    max-height: 2.5rem;
    overflow: hidden;
}

.home-almanac-hours {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 0.1875rem;
}

.home-almanac-hour {
    display: flex;
    flex-direction: column;
    align-items: center;
    border-radius: var(--radius-sm);
    padding: 0.1875rem 0 0.125rem;
    line-height: 1.25;
}

.home-almanac-hour-zhi {
    font-size: 0.625rem;
    font-weight: 600;
}

.home-almanac-hour-luck {
    font-size: 0.5625rem;
}

.home-almanac-foot {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 0.375rem;
    border-top: 1px solid var(--color-divider);
    padding-top: 0.375rem;
}

.home-almanac-link {
    flex: none;
    border: 0;
    background: none;
    padding: 0;
    color: var(--color-brand);
    font-size: 0.6875rem;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
}

/* Matches the approved mock: the label is followed by a thin chevron. */
.home-almanac-link::after {
    content: '\203A';
    margin-left: 0.25rem;
}

.home-almanac-link:hover {
    filter: brightness(var(--hover-brightness));
}

.home-almanac-link:focus-visible {
    outline: 2px solid var(--color-focus-ring, var(--color-brand));
    outline-offset: 2px;
}

/*
 * The 2x1 card only has one row of height, so the label and the tags are
 * tightened up. The tag rows never wrap, which keeps the height fixed.
 */
[data-size='2x1'] .home-almanac-row-label {
    width: 1rem;
    height: 1rem;
    font-size: 0.625rem;
}

[data-size='2x1'] .home-almanac-tag {
    padding: 0 0.25rem;
    line-height: 1.25;
}

/*
 * Almanac accents reuse the shared status palette so the card follows the
 * active theme (light / dark / OLED) instead of hard-coded hues. Keep these in
 * sync with AlmanacDetailModal.vue.
 */
.home-almanac-good {
    color: var(--color-green);
}

.home-almanac-bad {
    color: var(--color-red);
}

.home-almanac-row-label.home-almanac-good {
    background: var(--color-green);
    color: var(--color-accent-contrast);
}

.home-almanac-row-label.home-almanac-bad {
    background: var(--color-red);
    color: var(--color-accent-contrast);
}

.home-almanac-tag-good {
    border-color: color-mix(in srgb, var(--color-green) 30%, transparent);
    background: var(--color-green-bg);
    color: var(--color-green);
}

.home-almanac-tag-bad {
    border-color: color-mix(in srgb, var(--color-red) 30%, transparent);
    background: var(--color-red-bg);
    color: var(--color-red);
}

.home-almanac-tag-muted {
    border-color: var(--color-divider);
    background: var(--surface-4);
    color: var(--color-text-tertiary);
}

.home-almanac-hour.home-almanac-good {
    background: var(--color-green-bg);
}

.home-almanac-hour.home-almanac-bad {
    background: var(--color-red-bg);
}
</style>
