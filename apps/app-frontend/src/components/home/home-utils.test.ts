import { expect, it } from 'vitest'

import {
    buildHeatmapDays,
    getActivePlayerAccount,
    getActivePlayerName,
    getPlaytimeLevel,
    getTimeBucket,
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

it('maps playtime thresholds and missing days deterministically', () => {
    expect(
        [0, 1, 30 * 60, 30 * 60 + 1, 90 * 60, 90 * 60 + 1, 180 * 60, 180 * 60 + 1].map(
            getPlaytimeLevel,
        ),
    ).toEqual([0, 1, 1, 2, 2, 3, 3, 4])
    expect(toDateKey(new Date(2026, 6, 25, 12))).toBe('2026-07-25')
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
