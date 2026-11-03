import { expect, it } from 'vitest'

import {
    addHomeWidget,
    createDefaultHomeDashboard,
    createHomeDashboardSaveQueue,
    enableFreeHomeDashboard,
    findNearestFreeHomeWidgetPosition,
    getHomeGreetingOverride,
    getHomeGridColumnCount,
    getHomeWidgetDimensions,
    getHomeWidgetSpan,
    HOME_GREETING_GLOBAL_KEY,
    HOME_GREETING_NICKNAME_MAX,
    HOME_GREETING_TITLE_MAX,
    moveHomeWidget,
    normalizeHomeDashboard,
    packHomeWidgets,
    pruneHomeGreetingOverrides,
    removeHomeWidget,
    replaceHomeDashboardWidgets,
    resizeHomeWidget,
    setHomeDashboardLayout,
    setHomeGreetingOptions,
    setHomeGreetingOverride,
    setHomeRecentLimit,
    setHomeWidgetPosition,
} from './home-dashboard.ts'

function greetingOptionsOf(config: ReturnType<typeof createDefaultHomeDashboard>) {
    const widget = config.widgets.find((candidate) => candidate.kind === 'greeting')
    expect(widget, 'the default dashboard has a greeting widget').toBeTruthy()
    return { widget, options: widget.options }
}

it('derives one to four columns from the dashboard container width', () => {
    expect(getHomeGridColumnCount(0)).toBe(1)
    expect(getHomeGridColumnCount(495)).toBe(1)
    expect(getHomeGridColumnCount(496)).toBe(2)
    expect(getHomeGridColumnCount(752)).toBe(3)
    expect(getHomeGridColumnCount(2000)).toBe(4)
})

it('derives responsive widget dimensions from the current grid', () => {
    expect(getHomeWidgetDimensions('2x2', 4, 1008)).toEqual({
        width: 496,
        height: 336,
    })
    expect(getHomeWidgetDimensions('2x1', 1, 320)).toEqual({
        width: 320,
        height: 160,
    })
    expect(getHomeWidgetDimensions('3x2', 4, 1008)).toEqual({
        width: 752,
        height: 336,
    })
})

it('temporarily clamps wide widgets without changing their preferred size', () => {
    const config = createDefaultHomeDashboard()
    const preferredSize = config.widgets[0].size
    expect(getHomeWidgetSpan('2x2', 1)).toEqual({ columns: 1, rows: 2 })
    expect(getHomeWidgetSpan('2x2', 2)).toEqual({ columns: 2, rows: 2 })
    packHomeWidgets(config.widgets, 1)
    packHomeWidgets(config.widgets, 4)
    expect(config.widgets[0].size).toBe(preferredSize)
})

it('adds, reorders, resizes, and removes independent placements', () => {
    const original = createDefaultHomeDashboard()
    const pinnedInstances = original.widgets.find((widget) => widget.kind === 'pinned-instances')!
    const duplicate = { ...pinnedInstances, id: 'duplicate-pinned-instances' }
    const added = addHomeWidget(original, duplicate)
    expect(added.widgets.length).toBe(original.widgets.length + 1)
    expect(added.widgets.filter((widget) => widget.kind === 'pinned-instances').length).toBe(2)

    const moved = moveHomeWidget(added, added.widgets.length - 1, -1)
    expect(moved.widgets.at(-2)?.id).toBe(duplicate.id)
    const resized = resizeHomeWidget(moved, duplicate.id, '1x1')
    expect(resized.widgets.find((widget) => widget.id === duplicate.id)?.size).toBe('1x1')
    const removed = removeHomeWidget(resized, duplicate.id)
    expect(removed.widgets).toEqual(original.widgets)
})

it('accepts draggable order and restores the complete default layout', () => {
    const original = createDefaultHomeDashboard()
    const reversed = replaceHomeDashboardWidgets(original, [...original.widgets].reverse())
    expect(reversed.widgets.map((widget) => widget.id)).toEqual(
        original.widgets.map((widget) => widget.id).reverse(),
    )
    expect(createDefaultHomeDashboard().widgets.map(({ kind, size }) => ({ kind, size }))).toEqual([
        { kind: 'greeting', size: '2x1' },
        { kind: 'calendar', size: '1x2' },
        { kind: 'recent', size: '2x2' },
        { kind: 'pinned-worlds', size: '1x2' },
        { kind: 'pinned-servers', size: '2x2' },
        { kind: 'pinned-instances', size: '2x1' },
    ])
})

it('enables free layout from packed positions and preserves manual coordinates', () => {
    const original = createDefaultHomeDashboard()
    const positioned = setHomeWidgetPosition(original, original.widgets[0].id, {
        column: 1,
        row: 3,
    })
    const free = enableFreeHomeDashboard(positioned, 4)

    expect(free.layout).toBe('free')
    expect(free.widgets[0].position).toEqual({ column: 1, row: 3 })
    expect(free.widgets[1].position).toEqual({ column: 2, row: 0 })
    expect(setHomeDashboardLayout(free, 'grid').layout).toBe('grid')
})

it('snaps manual placement to the nearest open cell without moving other widgets', () => {
    const free = enableFreeHomeDashboard(createDefaultHomeDashboard(), 4)
    const before = free.widgets.map((widget) => widget.position)
    const moving = free.widgets.at(-1)!
    const occupied = free.widgets[0].position!
    const resolved = findNearestFreeHomeWidgetPosition(free.widgets, moving, occupied, 4)

    expect(resolved).not.toEqual(occupied)
    expect(free.widgets.map((widget) => widget.position)).toEqual(before)
})

it('rolls back the latest failed save and reports the error', async () => {
    const original = createDefaultHomeDashboard()
    const changed = removeHomeWidget(original, original.widgets[0].id)
    let current = changed
    const errors: unknown[] = []
    const queue = createHomeDashboardSaveQueue(
        async () => {
            throw new Error('save failed')
        },
        (config) => {
            current = config
        },
        (error) => errors.push(error),
    )

    await queue.enqueue(changed, original)
    await queue.flush()
    expect(current).toEqual(original)
    expect(errors.length).toBe(1)
})

it('normalizes a saved layout when entering Home again', () => {
    const saved = createDefaultHomeDashboard()
    const restored = normalizeHomeDashboard(JSON.parse(JSON.stringify(saved)))
    expect(restored).toEqual(saved)
})

it('normalizes legacy layouts and persisted free positions', () => {
    const legacy = normalizeHomeDashboard({
        version: 1,
        widgets: [{ id: 'legacy', kind: 'calendar', size: '1x2' }],
    })
    const free = normalizeHomeDashboard({
        version: 1,
        layout: 'free',
        widgets: [
            { id: 'valid', kind: 'calendar', size: '1x2', position: { column: 2, row: 4 } },
            { id: 'invalid', kind: 'calendar', size: '1x2', position: { column: -10, row: 'top' } },
        ],
    })

    expect(legacy?.layout).toBe('grid')
    expect(free?.layout).toBe('free')
    expect(free?.widgets[0].position).toEqual({ column: 2, row: 4 })
    expect(free?.widgets[1].position).toBe(undefined)
})

it('packs widgets into the earliest available cells', () => {
    const config = createDefaultHomeDashboard()
    const packed = packHomeWidgets(config.widgets, 4)
    expect(
        packed.map(({ column, row, effectiveColumns, effectiveRows }) => ({
            column,
            row,
            effectiveColumns,
            effectiveRows,
        })),
    ).toEqual([
        { column: 1, row: 1, effectiveColumns: 2, effectiveRows: 1 },
        { column: 3, row: 1, effectiveColumns: 1, effectiveRows: 2 },
        { column: 1, row: 2, effectiveColumns: 2, effectiveRows: 2 },
        { column: 4, row: 1, effectiveColumns: 1, effectiveRows: 2 },
        { column: 3, row: 3, effectiveColumns: 2, effectiveRows: 2 },
        { column: 1, row: 4, effectiveColumns: 2, effectiveRows: 1 },
    ])
})

it('normalizes malformed sizes while retaining duplicate widgets', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [
            { id: 'one', kind: 'calendar', size: '9x9' },
            { id: 'two', kind: 'calendar', size: '1x1' },
            { id: 'three', kind: 'world', size: '1x1', target: { instanceId: 'a' } },
        ],
    })

    expect(normalized?.widgets.map(({ kind, size }) => ({ kind, size }))).toEqual([
        { kind: 'calendar', size: '1x2' },
        { kind: 'calendar', size: '1x2' },
    ])
})

it('normalizes every persisted calendar placement to the fixed 1x2 size', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [
            { id: 'small', kind: 'calendar', size: '1x1' },
            { id: 'wide', kind: 'calendar', size: '2x2' },
        ],
    })

    expect(normalized?.widgets.map((widget) => widget.size)).toEqual(['1x2', '1x2'])
})

it('normalizes every persisted greeting placement to the fixed 2x1 size', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [
            { id: 'small', kind: 'greeting', size: '1x1' },
            {
                id: 'tall',
                kind: 'greeting',
                size: '1x2',
                options: { greetingMode: 'text', greetingText: '  Ready to play  ' },
            },
        ],
    })

    expect(normalized?.widgets.map((widget) => widget.size)).toEqual(['2x1', '2x1'])
    expect(normalized?.widgets.map((widget) => widget.options)).toEqual([
        { greetingMode: 'greeting', greetingFont: 'sans', greetingFontSize: 22 },
        {
            greetingMode: 'text',
            greetingText: 'Ready to play',
            greetingFont: 'sans',
            greetingFontSize: 22,
        },
    ])
    expect(
        resizeHomeWidget(normalized!, 'small', '1x1').widgets.map((widget) => widget.size),
    ).toEqual(['2x1', '2x1'])
    expect(
        setHomeGreetingOptions(normalized!, 'small', 'text-and-greeting', '  Hi  ', 'minecraft', 29)
            .widgets[0].options,
    ).toEqual({
        greetingMode: 'text-and-greeting',
        greetingText: 'Hi',
        greetingFont: 'minecraft',
        greetingFontSize: 29,
    })
})

it('normalizes greeting font settings and clamps font size', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [
            {
                id: 'large',
                kind: 'greeting',
                size: '2x1',
                options: { greetingFont: 'serif', greetingFontSize: 80 },
            },
            {
                id: 'invalid',
                kind: 'greeting',
                size: '2x1',
                options: { greetingFont: 'comic-sans', greetingFontSize: 'large' },
            },
        ],
    })!

    expect(
        normalized.widgets.map((widget) => ({
            font: widget.options?.greetingFont,
            fontSize: widget.options?.greetingFontSize,
        })),
    ).toEqual([
        { font: 'serif', fontSize: 32 },
        { font: 'sans', fontSize: 22 },
    ])
})

it('normalizes and updates recently played item limits', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [
            { id: 'legacy', kind: 'recent', size: '2x2' },
            { id: 'valid', kind: 'recent', size: '2x1', options: { recentLimit: 8 } },
            { id: 'invalid', kind: 'recent', size: '1x2', options: { recentLimit: 99 } },
        ],
    })!

    expect(normalized.widgets.map((widget) => widget.options?.recentLimit)).toEqual([4, 8, 4])
    expect(setHomeRecentLimit(normalized, 'legacy', 6).widgets[0].options?.recentLimit).toBe(6)
})

it('accepts and resizes the recent widget to 3-column layouts', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [{ id: 'recent', kind: 'recent', size: '3x2' }],
    })!
    const config = createDefaultHomeDashboard()
    const recent = config.widgets.find((widget) => widget.kind === 'recent')!

    expect(normalized.widgets[0].size).toBe('3x2')
    expect(
        resizeHomeWidget(config, recent.id, '3x1').widgets.find((widget) => widget.id === recent.id)
            ?.size,
    ).toBe('3x1')
})

it('keeps greeting text per account and falls back when none is saved', () => {
    const config = createDefaultHomeDashboard()
    const { widget, options } = greetingOptionsOf(config)

    expect(getHomeGreetingOverride(options, 'account-a')).toBe(null)

    const saved = setHomeGreetingOverride(config, widget.id, 'account-a', {
        nickname: '  Alex  ',
        title: '',
    })
    const savedOptions = greetingOptionsOf(saved).options

    expect(getHomeGreetingOverride(savedOptions, 'account-a')).toEqual({ nickname: 'Alex' })
    expect(getHomeGreetingOverride(savedOptions, 'account-b')).toBe(null)
    // The nickname is one tab and the title another, so one must not invent the other.
    expect(getHomeGreetingOverride(savedOptions, 'account-a')?.title).toBe(undefined)
})

it('clears an override when both fields are empty', () => {
    const config = createDefaultHomeDashboard()
    const { widget } = greetingOptionsOf(config)

    const saved = setHomeGreetingOverride(config, widget.id, 'account-a', {
        nickname: 'Alex',
        title: 'Hello',
    })
    const cleared = setHomeGreetingOverride(saved, widget.id, 'account-a', {
        nickname: '',
        title: '   ',
    })

    expect(getHomeGreetingOverride(greetingOptionsOf(cleared).options, 'account-a')).toBe(null)
    expect(greetingOptionsOf(cleared).options?.greetingOverrides).toBe(undefined)
})

it('clamps saved greeting text to the field limits', () => {
    const config = createDefaultHomeDashboard()
    const { widget } = greetingOptionsOf(config)

    const saved = setHomeGreetingOverride(config, widget.id, 'account-a', {
        nickname: 'n'.repeat(HOME_GREETING_NICKNAME_MAX + 20),
        title: 't'.repeat(HOME_GREETING_TITLE_MAX + 20),
    })
    const override = getHomeGreetingOverride(greetingOptionsOf(saved).options, 'account-a')

    expect(override?.nickname?.length).toBe(HOME_GREETING_NICKNAME_MAX)
    expect(override?.title?.length).toBe(HOME_GREETING_TITLE_MAX)
})

it('prunes overrides for accounts that no longer exist', () => {
    const config = createDefaultHomeDashboard()
    const { widget } = greetingOptionsOf(config)

    let saved = setHomeGreetingOverride(config, widget.id, HOME_GREETING_GLOBAL_KEY, {
        nickname: '',
        title: 'Global title',
    })
    saved = setHomeGreetingOverride(saved, widget.id, 'account-a', { nickname: 'Alex', title: '' })
    saved = setHomeGreetingOverride(saved, widget.id, 'account-b', { nickname: 'Bea', title: '' })

    const pruned = pruneHomeGreetingOverrides(saved, ['account-a'])
    const options = greetingOptionsOf(pruned).options

    expect(getHomeGreetingOverride(options, 'account-a')).toEqual({ nickname: 'Alex' })
    expect(getHomeGreetingOverride(options, 'account-b')).toBe(null)
    // The unbound title was not written by an account, so it stays.
    expect(getHomeGreetingOverride(options, HOME_GREETING_GLOBAL_KEY)).toEqual({
        title: 'Global title',
    })
    // An unchanged dashboard is returned as it is, so the caller can skip a write.
    expect(pruneHomeGreetingOverrides(pruned, ['account-a'])).toBe(pruned)
})

it('normalizes stored overrides and drops unusable entries', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        layout: 'grid',
        widgets: [
            {
                id: 'greeting',
                kind: 'greeting',
                size: '2x1',
                options: {
                    greetingOverrides: {
                        'account-a': { nickname: '  Alex  ' },
                        'account-b': { nickname: '' },
                        'account-c': 'not-an-object',
                        'account-d': { title: 42 },
                        'account-e': { nickname: 'Bea', title: 'Hello there' },
                    },
                },
            },
        ],
    })!

    expect(greetingOptionsOf(normalized).options?.greetingOverrides).toEqual({
        'account-a': { nickname: 'Alex' },
        'account-e': { nickname: 'Bea', title: 'Hello there' },
    })
})
