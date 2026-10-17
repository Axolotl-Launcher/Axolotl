<script setup lang="ts">
import { XIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, useFormatDateTime, useVIntl } from '@modrinth/ui'
import { computed, ref } from 'vue'

import type { AlmanacHour } from './almanac'
import { dateKey, getAlmanac, isAuspicious } from './almanac'
import { useAlmanacTerms } from './almanac-terms'
import {
    HOME_ALMANAC_DEFAULT_SHOW_ACTIVITIES,
    HOME_ALMANAC_DEFAULT_TERM_STYLE,
    type HomeAlmanacTermStyle,
} from './home-dashboard'

const props = defineProps<{
    date: Date
    showActivities?: boolean | null
    termStyle?: HomeAlmanacTermStyle | null
}>()

const { formatMessage } = useVIntl()
const formatFullDate = useFormatDateTime({ dateStyle: 'full' })

const messages = defineMessages({
    title: { id: 'app.home.almanac.title', defaultMessage: 'Almanac' },
    today: { id: 'app.home.almanac.today', defaultMessage: 'Today' },
    close: { id: 'app.home.almanac.close', defaultMessage: 'Close' },
    yi: { id: 'app.home.almanac.yi', defaultMessage: 'Do' },
    ji: { id: 'app.home.almanac.ji', defaultMessage: 'Avoid' },
    zhishen: { id: 'app.home.almanac.zhishen', defaultMessage: 'Day officer' },
    zhixing: { id: 'app.home.almanac.zhixing', defaultMessage: 'Twelve officers' },
    nayin: { id: 'app.home.almanac.nayin', defaultMessage: 'Element' },
    chongsha: { id: 'app.home.almanac.chongsha', defaultMessage: 'Clash' },
    sha: { id: 'app.home.almanac.sha', defaultMessage: 'Sha' },
    xiu: { id: 'app.home.almanac.xiu', defaultMessage: 'Mansion' },
    jishen: { id: 'app.home.almanac.jishen', defaultMessage: 'Auspicious deities' },
    xiongsha: { id: 'app.home.almanac.xiongsha', defaultMessage: 'Inauspicious influences' },
    positions: { id: 'app.home.almanac.positions', defaultMessage: 'Deity directions' },
    xi: { id: 'app.home.almanac.xi', defaultMessage: 'Joy god' },
    fu: { id: 'app.home.almanac.fu', defaultMessage: 'Fortune god' },
    cai: { id: 'app.home.almanac.cai', defaultMessage: 'Wealth god' },
    yangGui: { id: 'app.home.almanac.yang-gui', defaultMessage: 'Yang noble' },
    yinGui: { id: 'app.home.almanac.yin-gui', defaultMessage: 'Yin noble' },
    taishen: { id: 'app.home.almanac.taishen', defaultMessage: 'Fetal god' },
    lucky: { id: 'app.home.almanac.lucky', defaultMessage: 'Auspicious' },
    unlucky: { id: 'app.home.almanac.unlucky', defaultMessage: 'Inauspicious' },
    hours: { id: 'app.home.almanac.hours', defaultMessage: 'Hourly fortune' },
    hourName: { id: 'app.home.almanac.hour-name', defaultMessage: '{zhi} hour' },
    hourYi: { id: 'app.home.almanac.hour-yi', defaultMessage: 'Do: {items}' },
    pengzu: { id: 'app.home.almanac.pengzu', defaultMessage: 'Peng Zu taboos' },
    listSeparator: { id: 'app.home.almanac.list-separator', defaultMessage: ', ' },
    none: { id: 'app.home.almanac.no-activity', defaultMessage: 'None' },
    dataNote: {
        id: 'app.home.almanac.data-note',
        defaultMessage: 'Computed locally with lunar-typescript (MIT); works offline.',
    },
    sourceNote: {
        id: 'app.home.almanac.source-note',
        defaultMessage:
            'Auspicious and inauspicious activities follow a local almanac table and may differ from other almanac sources.',
    },
})

const modal = ref<InstanceType<typeof NewModal>>()
const today = ref(new Date())

const almanac = computed(() => getAlmanac(props.date))
const termStyle = computed(() => props.termStyle ?? HOME_ALMANAC_DEFAULT_TERM_STYLE)
/** Activity lists are opt-in; see `HomeAlmanac.vue` for why. */
const showActivities = computed(() => props.showActivities ?? HOME_ALMANAC_DEFAULT_SHOW_ACTIVITIES)
const { termLabel, termList } = useAlmanacTerms(() => termStyle.value)
const headerLabel = computed(
    () => `${formatMessage(messages.title)} · ${formatFullDate(props.date)}`,
)
const isToday = computed(() => dateKey(props.date) === dateKey(today.value))

const positions = computed(() => [
    { key: 'xi', label: formatMessage(messages.xi), value: almanac.value.xi },
    { key: 'fu', label: formatMessage(messages.fu), value: almanac.value.fu },
    { key: 'cai', label: formatMessage(messages.cai), value: almanac.value.cai },
    { key: 'yang-gui', label: formatMessage(messages.yangGui), value: almanac.value.yangGui },
    { key: 'yin-gui', label: formatMessage(messages.yinGui), value: almanac.value.yinGui },
    { key: 'taishen', label: formatMessage(messages.taishen), value: almanac.value.taiShen },
])

const jiShen = computed(() => almanac.value.jiShen)
const xiongSha = computed(() => almanac.value.xiongSha)
const pengZu = computed(() =>
    [almanac.value.pengZuGan, almanac.value.pengZuZhi].filter(Boolean).join(' · '),
)

const facts = computed(() => {
    const day = almanac.value
    const sha = day.sha ? `${formatMessage(messages.sha)} ${day.sha}` : ''
    return [
        {
            key: 'zhishen',
            label: formatMessage(messages.zhishen),
            value: [day.zhiShen, day.zhiShenType].filter(Boolean).join(' · '),
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
            key: 'xiu',
            label: formatMessage(messages.xiu),
            value: [day.xiu, day.xiuLuck].filter(Boolean).join(' · '),
            tone: luckTone(day.xiuLuck),
        },
        {
            key: 'chongsha',
            label: formatMessage(messages.chongsha),
            value: [day.chong, sha].filter(Boolean).join(' · '),
            tone: 'neutral' as const,
        },
    ]
})

function luckTone(luck: string): 'good' | 'bad' | 'neutral' {
    if (!luck) return 'neutral'
    return isAuspicious(luck) ? 'good' : 'bad'
}

function hourName(hour: AlmanacHour) {
    return `${formatMessage(messages.hourName, { zhi: hour.zhi })} ${hour.ganZhi}`.trim()
}

function hourActivity(hour: AlmanacHour) {
    if (hour.yi.length === 0) return formatMessage(messages.none)
    return termList(hour.yi).join(formatMessage(messages.listSeparator))
}

function show() {
    // Re-check the clock so the "today" chip is correct on every open.
    today.value = new Date()
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
    <NewModal
        ref="modal"
        hide-header
        :aria-label="headerLabel"
        max-width="720px"
        width="min(720px, calc(100vw - 2rem))"
        scrollable
        max-content-height="min(40rem, 76vh)"
    >
        <div class="almanac-modal">
            <!--
                NewModal hides its own header, so the title bar is drawn here. The
                negative margins cancel the modal content padding (p-6) to keep the
                bar flush with the modal edges, like the approved mock.
            -->
            <header class="almanac-modal-head">
                <span class="almanac-modal-title">{{ headerLabel }}</span>
                <div class="almanac-modal-head-actions">
                    <span v-if="isToday" class="almanac-today">{{
                        formatMessage(messages.today)
                    }}</span>
                    <Button
                        circular
                        icon-only
                        :aria-label="formatMessage(messages.close)"
                        @click="hide"
                    >
                        <XIcon aria-hidden="true" />
                    </Button>
                </div>
            </header>

            <div class="almanac-modal-body">
                <div class="almanac-modal-facts">
                    <div
                        v-for="fact in facts"
                        :key="fact.key"
                        class="almanac-fact"
                        :class="`almanac-${fact.tone}`"
                    >
                        <span class="almanac-fact-label">{{ fact.label }}</span>
                        <span class="almanac-fact-value">{{ fact.value }}</span>
                    </div>
                </div>
                <div class="almanac-modal-col">
                    <section v-if="showActivities" class="almanac-sect">
                        <h3 class="almanac-sect-title">{{ formatMessage(messages.yi) }}</h3>
                        <div class="almanac-tags">
                            <span
                                v-for="tag in almanac.yi"
                                :key="`yi-${tag}`"
                                class="almanac-tag almanac-tag-good"
                                >{{ termLabel(tag) }}</span
                            >
                            <span v-if="almanac.yi.length === 0" class="almanac-tag-empty">{{
                                formatMessage(messages.none)
                            }}</span>
                        </div>
                    </section>

                    <section v-if="showActivities" class="almanac-sect">
                        <h3 class="almanac-sect-title">{{ formatMessage(messages.ji) }}</h3>
                        <div class="almanac-tags">
                            <span
                                v-for="tag in almanac.ji"
                                :key="`ji-${tag}`"
                                class="almanac-tag almanac-tag-bad"
                                >{{ termLabel(tag) }}</span
                            >
                            <span v-if="almanac.ji.length === 0" class="almanac-tag-empty">{{
                                formatMessage(messages.none)
                            }}</span>
                        </div>
                    </section>

                    <section class="almanac-sect">
                        <h3 class="almanac-sect-title">{{ formatMessage(messages.jishen) }}</h3>
                        <div class="almanac-tags">
                            <span
                                v-for="tag in jiShen"
                                :key="`jishen-${tag}`"
                                class="almanac-tag almanac-tag-good"
                                >{{ termLabel(tag) }}</span
                            >
                            <span v-if="jiShen.length === 0" class="almanac-tag-empty">{{
                                formatMessage(messages.none)
                            }}</span>
                        </div>
                    </section>

                    <section class="almanac-sect">
                        <h3 class="almanac-sect-title">{{ formatMessage(messages.xiongsha) }}</h3>
                        <div class="almanac-tags">
                            <span
                                v-for="tag in xiongSha"
                                :key="`xiongsha-${tag}`"
                                class="almanac-tag almanac-tag-bad"
                                >{{ termLabel(tag) }}</span
                            >
                            <span v-if="xiongSha.length === 0" class="almanac-tag-empty">{{
                                formatMessage(messages.none)
                            }}</span>
                        </div>
                    </section>
                </div>

                <div class="almanac-modal-col">
                    <section class="almanac-sect">
                        <h3 class="almanac-sect-title">{{ formatMessage(messages.positions) }}</h3>
                        <div class="almanac-kv">
                            <span v-for="item in positions" :key="item.key" class="almanac-kv-item">
                                <span class="almanac-kv-label">{{ item.label }}</span>
                                <span class="almanac-kv-value">{{
                                    item.value || formatMessage(messages.none)
                                }}</span>
                            </span>
                        </div>
                    </section>

                    <section class="almanac-sect">
                        <h3 class="almanac-sect-title">{{ formatMessage(messages.hours) }}</h3>
                        <table v-if="almanac.hours.length > 0" class="almanac-hourly">
                            <tbody>
                                <tr v-for="hour in almanac.hours" :key="hour.zhi">
                                    <td class="almanac-hourly-name">{{ hourName(hour) }}</td>
                                    <td
                                        class="almanac-hourly-luck"
                                        :class="hour.lucky ? 'almanac-good' : 'almanac-bad'"
                                    >
                                        {{
                                            formatMessage(
                                                hour.lucky ? messages.lucky : messages.unlucky,
                                            )
                                        }}
                                    </td>
                                    <td v-if="showActivities" class="almanac-hourly-yi">
                                        {{
                                            formatMessage(messages.hourYi, {
                                                items: hourActivity(hour),
                                            })
                                        }}
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                        <span v-else class="almanac-tag-empty">{{
                            formatMessage(messages.none)
                        }}</span>
                    </section>

                    <p class="almanac-note">
                        <span class="almanac-note-line">
                            <span class="almanac-note-label">{{
                                formatMessage(messages.pengzu)
                            }}</span>
                            {{ pengZu || formatMessage(messages.none) }}
                        </span>
                        <span class="almanac-note-line">{{
                            formatMessage(messages.dataNote)
                        }}</span>
                        <span v-if="showActivities" class="almanac-note-line">{{
                            formatMessage(messages.sourceNote)
                        }}</span>
                    </p>
                </div>
            </div>
        </div>
    </NewModal>
</template>

<style scoped>
.almanac-modal {
    display: flex;
    min-width: 0;
    flex-direction: column;
}

.almanac-modal-head {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 0.5rem;
    margin: -1.5rem -1.5rem 0.875rem;
    border-bottom: 1px solid color-mix(in srgb, var(--color-brand) 22%, transparent);
    border-radius: var(--radius-lg) var(--radius-lg) 0 0;
    background: color-mix(in srgb, var(--color-brand) 10%, var(--surface-3));
    padding: 0.6875rem 0.875rem;
}

.almanac-modal-title {
    min-width: 0;
    overflow: hidden;
    color: var(--color-brand);
    font-size: 0.8125rem;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.almanac-modal-head-actions {
    display: flex;
    margin-left: auto;
    flex: none;
    align-items: center;
    gap: 0.375rem;
}

.almanac-today {
    border-radius: 999px;
    background: var(--color-brand-highlight);
    padding: 0.125rem 0.5rem;
    color: var(--color-brand);
    font-size: 0.65625rem;
    font-weight: 600;
}

.almanac-modal-body {
    display: grid;
    min-width: 0;
    grid-template-columns: 1.15fr 1fr;
    gap: 0.875rem;
}

@media (max-width: 40rem) {
    .almanac-modal-body {
        grid-template-columns: minmax(0, 1fr);
    }
}

/*
 * The day facts span both columns above the two-column body, mirroring the
 * card's fact grid at a larger size.
 */
.almanac-modal-facts {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 0.375rem;
}

@media (max-width: 60rem) {
    .almanac-modal-facts {
        grid-template-columns: repeat(3, minmax(0, 1fr));
    }
}

@media (max-width: 40rem) {
    .almanac-modal-facts {
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
}

.almanac-fact {
    display: flex;
    min-width: 0;
    flex-direction: column;
    border-radius: var(--radius-sm);
    background: var(--surface-4);
    padding: 0.375rem 0.4375rem;
}

.almanac-fact-label {
    color: var(--color-text-tertiary);
    font-size: 0.625rem;
    font-weight: 500;
}

.almanac-fact-value {
    overflow: hidden;
    font-size: 0.6875rem;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.almanac-fact.almanac-good .almanac-fact-value {
    color: var(--color-green);
}

.almanac-fact.almanac-bad .almanac-fact-value {
    color: var(--color-red);
}

.almanac-modal-col {
    display: flex;
    min-width: 0;
    flex-direction: column;
}

.almanac-sect {
    margin-bottom: 0.6875rem;
}

.almanac-sect:last-child {
    margin-bottom: 0;
}

.almanac-sect-title {
    margin: 0 0 0.3125rem;
    color: var(--color-text-tertiary);
    font-size: 0.6875rem;
    font-weight: 600;
}

.almanac-tags {
    display: flex;
    min-width: 0;
    flex-wrap: wrap;
    gap: 0.25rem;
}

.almanac-tag {
    border: 1px solid transparent;
    border-radius: 999px;
    padding: 0.125rem 0.4375rem;
    font-size: 0.65625rem;
    font-weight: 600;
    line-height: 1.35;
    white-space: nowrap;
}

.almanac-tag-empty {
    color: var(--color-text-tertiary);
    font-size: 0.6875rem;
}

.almanac-tag-good {
    border-color: color-mix(in srgb, var(--color-green) 30%, transparent);
    background: var(--color-green-bg);
    color: var(--color-green);
}

.almanac-tag-bad {
    border-color: color-mix(in srgb, var(--color-red) 30%, transparent);
    background: var(--color-red-bg);
    color: var(--color-red);
}

.almanac-kv {
    display: flex;
    min-width: 0;
    flex-wrap: wrap;
    gap: 0.375rem 0.75rem;
}

.almanac-kv-item {
    display: inline-flex;
    align-items: baseline;
    gap: 0.25rem;
    font-size: 0.71875rem;
}

.almanac-kv-label {
    color: var(--color-text-tertiary);
}

.almanac-kv-value {
    color: var(--color-text-primary);
    font-weight: 600;
}

.almanac-hourly {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.6875rem;
}

.almanac-hourly td {
    border-bottom: 1px solid color-mix(in srgb, var(--color-divider) 55%, transparent);
    padding: 0.21875rem 0.3125rem;
}

.almanac-hourly tr:last-child td {
    border-bottom: 0;
}

.almanac-hourly-name {
    font-weight: 600;
    white-space: nowrap;
}

.almanac-hourly-luck {
    font-weight: 600;
    white-space: nowrap;
}

.almanac-hourly-yi {
    color: var(--color-text-tertiary);
}

.almanac-note {
    display: flex;
    margin: 0.625rem 0 0;
    flex-direction: column;
    gap: 0.125rem;
    color: var(--color-text-tertiary);
    font-size: 0.65625rem;
    line-height: 1.5;
}

.almanac-note-label {
    font-weight: 600;
}

.almanac-good {
    color: var(--color-green);
}

.almanac-bad {
    color: var(--color-red);
}
</style>
