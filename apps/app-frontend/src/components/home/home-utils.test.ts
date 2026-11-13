import { expect, it } from 'vitest'

import {
    buildHeatmapDays,
    buildHeatWeeks,
    getActivePlayerAccount,
    getActivePlayerName,
    getBandColumns,
    getCalendarLayout,
    getHeatLevel,
    getHomeItemVariant,
    getHomeWorldRowState,
    getLongestStreak,
    getQuickSwitchInstances,
    getServerFill,
    getTimeBucket,
    getWeekSpan,
    resolveHomeLaunchInstance,
    stableGreetingIndex,
    toDateKey,
} from './home-utils.ts'

it('resolves the greeting account only when it can carry a name', () => {
    const accounts = [
        { account_id: 'ms', account_type: 'microsoft', profile: { id: 'p1', name: 'Alex' } },
        { account_id: 'yg', account_type: 'yggdrasil', profile: { id: 'p2', name: 'Yin' } },
        { account_id: 'off', account_type: 'offline', profile: { id: 'p3', name: 'Steve' } },
        { account_id: 'noname', account_type: 'microsoft', profile: { id: 'p4' } },
    ]

    expect(getActivePlayerAccount('ms', accounts)).toEqual({ id: 'ms', name: 'Alex' })
    expect(getActivePlayerAccount('yg', accounts)).toEqual({ id: 'yg', name: 'Yin' })
    // An offline account is selected the same way but has no profile name to greet.
    expect(getActivePlayerAccount('off', accounts)).toBe(null)
    expect(getActivePlayerAccount('noname', accounts)).toBe(null)
    expect(getActivePlayerAccount('gone', accounts)).toBe(null)
    expect(getActivePlayerAccount(undefined, accounts)).toBe(null)
    expect(getActivePlayerName('ms', accounts)).toBe('Alex')
})

it('uses the six local greeting time buckets', () => {
    const hour = (value: number) => new Date(2026, 6, 25, value, 0)
    expect(getTimeBucket(hour(0))).toBe('late-night')
    expect(getTimeBucket(hour(5))).toBe('dawn')
    expect(getTimeBucket(hour(8))).toBe('morning')
    expect(getTimeBucket(hour(12))).toBe('afternoon')
    expect(getTimeBucket(hour(17))).toBe('evening')
    expect(getTimeBucket(hour(21))).toBe('night')
    expect(stableGreetingIndex('2026-07-25:morning', 16)).toBe(
        stableGreetingIndex('2026-07-25:morning', 16),
    )
    expect(stableGreetingIndex('any', 0)).toBe(0)
})

it('builds Monday-first month and year heatmap grids', () => {
    const month = buildHeatmapDays(new Date(2026, 1, 17, 12), 'month')
    expect(month[0]?.date.getDay()).toBe(1)
    expect(month.at(-1)?.date.getDay()).toBe(0)
    expect(month.filter((day) => day.inPeriod).length).toBe(28)
    expect(month.find((day) => day.inPeriod)?.dateKey).toBe('2026-02-01')

    const year = buildHeatmapDays(new Date(2024, 6, 1, 12), 'year')
    expect(year[0]?.date.getDay()).toBe(1)
    expect(year.at(-1)?.date.getDay()).toBe(0)
    expect(year.filter((day) => day.inPeriod).length).toBe(366)
})

it('scales heatmap steps to the busiest day in view, from four hours up', () => {
    // Nothing played: every day is the empty step, even with a zero maximum.
    expect([0, 0, 0].map((seconds) => getHeatLevel(seconds, 0))).toEqual([0, 0, 0])
    expect(getHeatLevel(0, 3600)).toBe(0)
    expect(getHeatLevel(-5, 3600)).toBe(0)
    // Only one day played: an ordinary session is not the deepest colour...
    expect(getHeatLevel(2 * 3600, 2 * 3600)).toBe(3)
    expect(getHeatLevel(10 * 60, 10 * 60)).toBe(1)
    // ...but a long one is.
    expect(getHeatLevel(5 * 3600, 5 * 3600)).toBe(4)
    // One extreme day: a short session still shows, the long one is the top.
    const max = 10 * 3600
    expect(getHeatLevel(60, max)).toBe(1)
    expect(getHeatLevel(10 * 60, max)).toBe(1)
    expect(getHeatLevel(max, max)).toBe(4)
    // Steps rise with the share of the busiest day and never pass the top.
    const busiest = 8 * 3600
    expect(
        [0.05, 0.2, 0.45, 0.8, 1, 2].map((share) => getHeatLevel(share * busiest, busiest)),
    ).toEqual([1, 2, 3, 4, 4, 4])
    expect(toDateKey(new Date(2026, 6, 25, 12))).toBe('2026-07-25')
})

it('draws a band back to the first week played and three more, at least twelve', () => {
    expect(getWeekSpan('2026-11-13', '2026-11-13')).toBe(1)
    expect(getWeekSpan('2026-11-09', '2026-11-15')).toBe(1)
    expect(getWeekSpan('2026-11-08', '2026-11-09')).toBe(2)
    expect(getWeekSpan('2026-09-01', '2026-11-13')).toBe(11)
    expect(getBandColumns(0)).toBe(Number.POSITIVE_INFINITY)
    expect(getBandColumns(1)).toBe(12)
    expect(getBandColumns(11)).toBe(14)
    expect(getBandColumns(40)).toBe(43)
})

it('lays a heatmap band out in Monday-first week columns ending this week', () => {
    // Thursday 13 November 2026.
    const weeks = buildHeatWeeks(new Date(2026, 10, 13, 9), 3)
    expect(weeks).toHaveLength(3)
    expect(weeks.every((week) => week.length === 7)).toBe(true)
    expect(weeks[0][0].dateKey).toBe('2026-10-26')
    expect(weeks[0][0].date.getDay()).toBe(1)
    expect(weeks[2][0].dateKey).toBe('2026-11-09')
    expect(weeks[2][6].dateKey).toBe('2026-11-15')
    // Weeks cross month and year ends without gaps.
    const turn = buildHeatWeeks(new Date(2027, 0, 2, 12), 2).flat()
    expect(turn.map((day) => day.dateKey).slice(5, 9)).toEqual([
        '2026-12-26',
        '2026-12-27',
        '2026-12-28',
        '2026-12-29',
    ])
})

it('draws the calendar as a week, a month, a band or a strip by size', () => {
    expect(['1x1', '1x2', '2x1', '2x2', '3x2', null].map(getCalendarLayout)).toEqual([
        'week',
        'month',
        'strip',
        'band',
        'band',
        'month',
    ])
})

it('finds the longest run of consecutive played days', () => {
    expect(getLongestStreak([])).toBe(0)
    expect(getLongestStreak(['2026-11-05'])).toBe(1)
    expect(
        getLongestStreak([
            '2026-11-03',
            '2026-10-31',
            '2026-11-01',
            '2026-11-02',
            '2026-11-02',
            '2026-11-09',
            '2026-11-10',
        ]),
    ).toBe(4)
})

it('uses only active online accounts for player greetings', () => {
    const accounts = [
        {
            account_id: 'offline',
            account_type: 'offline',
            profile: { id: 'shared', name: 'Local player' },
        },
        {
            account_id: 'microsoft',
            account_type: 'microsoft',
            profile: { id: 'shared', name: 'Alex' },
        },
        {
            account_id: 'yggdrasil',
            account_type: 'yggdrasil',
            profile: { id: 'shared', name: 'Steve' },
        },
    ]
    expect(getActivePlayerName('microsoft', accounts)).toBe('Alex')
    expect(getActivePlayerName('yggdrasil', accounts)).toBe('Steve')
    expect(getActivePlayerName('offline', accounts)).toBe(null)
    expect(getActivePlayerName(undefined, accounts)).toBe(null)
    expect(getActivePlayerName('missing', accounts)).toBe(null)
})

it('launches the chosen Home instance, or else the one played last', () => {
    const instances = [
        { id: 'old', last_played: '2026-01-01T10:00:00Z' },
        { id: 'never', last_played: null },
        { id: 'new', last_played: '2026-03-01T10:00:00Z' },
    ]

    expect(resolveHomeLaunchInstance(instances, 'old')).toEqual({
        instance: instances[0],
        chosen: true,
    })
    expect(resolveHomeLaunchInstance(instances, 'deleted')).toEqual({
        instance: instances[2],
        chosen: false,
    })
    expect(resolveHomeLaunchInstance(instances, null)?.instance.id).toBe('new')
    expect(resolveHomeLaunchInstance([{ id: 'never', last_played: null }], null)).toBeNull()
    expect(resolveHomeLaunchInstance([], 'old')).toBeNull()
})

it('offers the other instances to switch to, most recently played first', () => {
    const instances = [
        { id: 'a', last_played: '2026-01-01T10:00:00Z' },
        { id: 'b', last_played: null },
        { id: 'c', last_played: '2026-03-01T10:00:00Z' },
        { id: 'd', last_played: '2026-02-01T10:00:00Z' },
        { id: 'e', last_played: null },
    ]

    expect(getQuickSwitchInstances(instances, 'c', 3).map(({ id }) => id)).toEqual(['d', 'a', 'b'])
    expect(getQuickSwitchInstances(instances, null, 10).map(({ id }) => id)).toEqual([
        'c',
        'd',
        'a',
        'b',
        'e',
    ])
    expect(getQuickSwitchInstances(instances, 'a', 0)).toEqual([])
})

it('offers Stop only on the world the running game was joined through', () => {
    const base = { starting: false, supportsQuickPlay: true }

    expect(getHomeWorldRowState({ ...base, instanceRunning: false, joinedHere: false })).toEqual({
        showStop: false,
        playDisabled: false,
        reason: null,
    })
    expect(getHomeWorldRowState({ ...base, instanceRunning: true, joinedHere: true })).toEqual({
        showStop: true,
        playDisabled: false,
        reason: null,
    })
    // Another world or server of the same running instance: Play, disabled, with the reason.
    expect(getHomeWorldRowState({ ...base, instanceRunning: true, joinedHere: false })).toEqual({
        showStop: false,
        playDisabled: true,
        reason: 'game-open',
    })
    // A world the running game holds open can still be stopped.
    expect(
        getHomeWorldRowState({ ...base, instanceRunning: true, joinedHere: false, locked: true })
            .showStop,
    ).toBe(true)
    expect(
        getHomeWorldRowState({ ...base, instanceRunning: false, joinedHere: false, locked: true }),
    ).toEqual({ showStop: false, playDisabled: true, reason: 'world-in-use' })
    expect(
        getHomeWorldRowState({
            ...base,
            instanceRunning: false,
            joinedHere: false,
            supportsQuickPlay: false,
        }).reason,
    ).toBe('no-quick-play')
    expect(
        getHomeWorldRowState({ ...base, instanceRunning: true, joinedHere: true, starting: true }),
    ).toEqual({ showStop: false, playDisabled: true, reason: null })
})

it('lays collections out as rows, a strip of cards or a grid of tiles by size', () => {
    expect(getHomeItemVariant(null)).toBe('row')
    expect(getHomeItemVariant('1x1')).toBe('row')
    expect(getHomeItemVariant('1x2')).toBe('row')
    expect(getHomeItemVariant('2x1')).toBe('card')
    expect(getHomeItemVariant('3x1')).toBe('card')
    expect(getHomeItemVariant('2x2')).toBe('tile')
    expect(getHomeItemVariant('3x2')).toBe('tile')
    expect(getHomeItemVariant('2x3')).toBe('tile')
})

it('reports how full a server is only when it says', () => {
    expect(getServerFill(50, 200)).toBe(0.25)
    expect(getServerFill(300, 200)).toBe(1)
    expect(getServerFill(10, 0)).toBeNull()
    expect(getServerFill(undefined, 20)).toBeNull()
})
