import assert from 'node:assert/strict'
import test from 'node:test'

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
    setHomeAlmanacOptions,
    setHomeDashboardLayout,
    setHomeGreetingOptions,
    setHomeGreetingOverride,
    setHomeRecentLimit,
    setHomeWidgetPosition,
} from './home-dashboard.ts'

function greetingOptionsOf(config: ReturnType<typeof createDefaultHomeDashboard>) {
    const widget = config.widgets.find((candidate) => candidate.kind === 'greeting')
    assert.ok(widget, 'the default dashboard has a greeting widget')
    return { widget, options: widget.options }
}

test('derives one to four columns from the dashboard container width', () => {
    assert.equal(getHomeGridColumnCount(0), 1)
    assert.equal(getHomeGridColumnCount(495), 1)
    assert.equal(getHomeGridColumnCount(496), 2)
    assert.equal(getHomeGridColumnCount(752), 3)
    assert.equal(getHomeGridColumnCount(2000), 4)
})

test('derives responsive widget dimensions from the current grid', () => {
    assert.deepEqual(getHomeWidgetDimensions('2x2', 4, 1008), {
        width: 496,
        height: 336,
    })
    assert.deepEqual(getHomeWidgetDimensions('2x1', 1, 320), {
        width: 320,
        height: 160,
    })
    assert.deepEqual(getHomeWidgetDimensions('3x2', 4, 1008), {
        width: 752,
        height: 336,
    })
})

test('temporarily clamps wide widgets without changing their preferred size', () => {
    const config = createDefaultHomeDashboard()
    const preferredSize = config.widgets[0].size
    assert.deepEqual(getHomeWidgetSpan('2x2', 1), { columns: 1, rows: 2 })
    assert.deepEqual(getHomeWidgetSpan('2x2', 2), { columns: 2, rows: 2 })
    packHomeWidgets(config.widgets, 1)
    packHomeWidgets(config.widgets, 4)
    assert.equal(config.widgets[0].size, preferredSize)
})

test('adds, reorders, resizes, and removes independent placements', () => {
    const original = createDefaultHomeDashboard()
    const pinnedInstances = original.widgets.find((widget) => widget.kind === 'pinned-instances')!
    const duplicate = { ...pinnedInstances, id: 'duplicate-pinned-instances' }
    const added = addHomeWidget(original, duplicate)
    assert.equal(added.widgets.length, original.widgets.length + 1)
    assert.equal(added.widgets.filter((widget) => widget.kind === 'pinned-instances').length, 2)

    const moved = moveHomeWidget(added, added.widgets.length - 1, -1)
    assert.equal(moved.widgets.at(-2)?.id, duplicate.id)
    const resized = resizeHomeWidget(moved, duplicate.id, '1x1')
    assert.equal(resized.widgets.find((widget) => widget.id === duplicate.id)?.size, '1x1')
    const removed = removeHomeWidget(resized, duplicate.id)
    assert.deepEqual(removed.widgets, original.widgets)
})

test('accepts draggable order and restores the complete default layout', () => {
    const original = createDefaultHomeDashboard()
    const reversed = replaceHomeDashboardWidgets(original, [...original.widgets].reverse())
    assert.deepEqual(
        reversed.widgets.map((widget) => widget.id),
        original.widgets.map((widget) => widget.id).reverse(),
    )
    assert.deepEqual(
        createDefaultHomeDashboard().widgets.map(({ kind, size }) => ({ kind, size })),
        [
            { kind: 'greeting', size: '2x1' },
            { kind: 'calendar', size: '1x2' },
            { kind: 'recent', size: '2x2' },
            { kind: 'pinned-worlds', size: '1x2' },
            { kind: 'pinned-servers', size: '2x2' },
            { kind: 'pinned-instances', size: '2x1' },
        ],
    )
})

test('enables free layout from packed positions and preserves manual coordinates', () => {
    const original = createDefaultHomeDashboard()
    const positioned = setHomeWidgetPosition(original, original.widgets[0].id, {
        column: 1,
        row: 3,
    })
    const free = enableFreeHomeDashboard(positioned, 4)

    assert.equal(free.layout, 'free')
    assert.deepEqual(free.widgets[0].position, { column: 1, row: 3 })
    assert.deepEqual(free.widgets[1].position, { column: 2, row: 0 })
    assert.equal(setHomeDashboardLayout(free, 'grid').layout, 'grid')
})

test('snaps manual placement to the nearest open cell without moving other widgets', () => {
    const free = enableFreeHomeDashboard(createDefaultHomeDashboard(), 4)
    const before = free.widgets.map((widget) => widget.position)
    const moving = free.widgets.at(-1)!
    const occupied = free.widgets[0].position!
    const resolved = findNearestFreeHomeWidgetPosition(free.widgets, moving, occupied, 4)

    assert.notDeepEqual(resolved, occupied)
    assert.deepEqual(
        free.widgets.map((widget) => widget.position),
        before,
    )
})

test('rolls back the latest failed save and reports the error', async () => {
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
    assert.deepEqual(current, original)
    assert.equal(errors.length, 1)
})

test('normalizes a saved layout when entering Home again', () => {
    const saved = createDefaultHomeDashboard()
    const restored = normalizeHomeDashboard(JSON.parse(JSON.stringify(saved)))
    assert.deepEqual(restored, saved)
})

test('normalizes legacy layouts and persisted free positions', () => {
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

    assert.equal(legacy?.layout, 'grid')
    assert.equal(free?.layout, 'free')
    assert.deepEqual(free?.widgets[0].position, { column: 2, row: 4 })
    assert.equal(free?.widgets[1].position, undefined)
})

test('packs widgets into the earliest available cells', () => {
    const config = createDefaultHomeDashboard()
    const packed = packHomeWidgets(config.widgets, 4)
    assert.deepEqual(
        packed.map(({ column, row, effectiveColumns, effectiveRows }) => ({
            column,
            row,
            effectiveColumns,
            effectiveRows,
        })),
        [
            { column: 1, row: 1, effectiveColumns: 2, effectiveRows: 1 },
            { column: 3, row: 1, effectiveColumns: 1, effectiveRows: 2 },
            { column: 1, row: 2, effectiveColumns: 2, effectiveRows: 2 },
            { column: 4, row: 1, effectiveColumns: 1, effectiveRows: 2 },
            { column: 3, row: 3, effectiveColumns: 2, effectiveRows: 2 },
            { column: 1, row: 4, effectiveColumns: 2, effectiveRows: 1 },
        ],
    )
})

test('normalizes malformed sizes while retaining duplicate widgets', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [
            { id: 'one', kind: 'calendar', size: '9x9' },
            { id: 'two', kind: 'calendar', size: '1x1' },
            { id: 'three', kind: 'world', size: '1x1', target: { instanceId: 'a' } },
        ],
    })

    assert.deepEqual(
        normalized?.widgets.map(({ kind, size }) => ({ kind, size })),
        [
            { kind: 'calendar', size: '1x2' },
            { kind: 'calendar', size: '1x2' },
        ],
    )
})

test('normalizes every persisted calendar placement to the fixed 1x2 size', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [
            { id: 'small', kind: 'calendar', size: '1x1' },
            { id: 'wide', kind: 'calendar', size: '2x2' },
        ],
    })

    assert.deepEqual(
        normalized?.widgets.map((widget) => widget.size),
        ['1x2', '1x2'],
    )
})

test('normalizes every persisted greeting placement to the fixed 2x1 size', () => {
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

    assert.deepEqual(
        normalized?.widgets.map((widget) => widget.size),
        ['2x1', '2x1'],
    )
    assert.deepEqual(
        normalized?.widgets.map((widget) => widget.options),
        [
            { greetingMode: 'greeting', greetingFont: 'sans', greetingFontSize: 22 },
            {
                greetingMode: 'text',
                greetingText: 'Ready to play',
                greetingFont: 'sans',
                greetingFontSize: 22,
            },
        ],
    )
    assert.deepEqual(
        resizeHomeWidget(normalized!, 'small', '1x1').widgets.map((widget) => widget.size),
        ['2x1', '2x1'],
    )
    assert.deepEqual(
        setHomeGreetingOptions(normalized!, 'small', 'text-and-greeting', '  Hi  ', 'minecraft', 29)
            .widgets[0].options,
        {
            greetingMode: 'text-and-greeting',
            greetingText: 'Hi',
            greetingFont: 'minecraft',
            greetingFontSize: 29,
        },
    )
})

test('normalizes greeting font settings and clamps font size', () => {
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

    assert.deepEqual(
        normalized.widgets.map((widget) => ({
            font: widget.options?.greetingFont,
            fontSize: widget.options?.greetingFontSize,
        })),
        [
            { font: 'serif', fontSize: 32 },
            { font: 'sans', fontSize: 22 },
        ],
    )
})

test('normalizes and updates recently played item limits', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [
            { id: 'legacy', kind: 'recent', size: '2x2' },
            { id: 'valid', kind: 'recent', size: '2x1', options: { recentLimit: 8 } },
            { id: 'invalid', kind: 'recent', size: '1x2', options: { recentLimit: 99 } },
        ],
    })!

    assert.deepEqual(
        normalized.widgets.map((widget) => widget.options?.recentLimit),
        [4, 8, 4],
    )
    assert.equal(setHomeRecentLimit(normalized, 'legacy', 6).widgets[0].options?.recentLimit, 6)
})

test('normalizes and updates almanac options', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [
            { id: 'legacy', kind: 'almanac', size: '2x2' },
            {
                id: 'valid',
                kind: 'almanac',
                size: '2x2',
                options: {
                    almanacShowHours: false,
                    almanacShowActivities: true,
                    almanacTagLimit: 9,
                    almanacTermStyle: 'plain',
                },
            },
            {
                id: 'invalid',
                kind: 'almanac',
                size: '2x2',
                options: {
                    almanacShowHours: 'yes',
                    almanacShowActivities: 'yes',
                    almanacTagLimit: 42,
                    almanacTermStyle: 'fancy',
                },
            },
        ],
    })!

    assert.deepEqual(
        normalized.widgets.map((widget) => widget.options?.almanacTagLimit),
        [6, 9, 6],
    )
    assert.deepEqual(
        normalized.widgets.map((widget) => widget.options?.almanacShowHours),
        [true, false, true],
    )
    assert.deepEqual(
        normalized.widgets.map((widget) => widget.options?.almanacTermStyle),
        ['traditional', 'plain', 'traditional'],
    )
    assert.deepEqual(
        normalized.widgets.map((widget) => widget.options?.almanacShowActivities),
        [false, true, false],
    )
    assert.equal(
        setHomeAlmanacOptions(normalized, 'legacy', false, 3, 'traditional', true).widgets[0]
            .options?.almanacTagLimit,
        3,
    )
    assert.equal(
        setHomeAlmanacOptions(normalized, 'legacy', false, 3, 'traditional', true).widgets[0]
            .options?.almanacTermStyle,
        'traditional',
    )
    assert.equal(
        setHomeAlmanacOptions(normalized, 'legacy', false, 3, 'traditional', true).widgets[0]
            .options?.almanacShowActivities,
        true,
    )
    // The setter clamps out-of-range values instead of storing them.
    assert.equal(
        setHomeAlmanacOptions(
            normalized,
            'legacy',
            true,
            42 as never,
            'fancy' as never,
            'yes' as never,
        ).widgets[0].options?.almanacTagLimit,
        6,
    )
    assert.equal(
        setHomeAlmanacOptions(
            normalized,
            'legacy',
            true,
            42 as never,
            'fancy' as never,
            'yes' as never,
        ).widgets[0].options?.almanacTermStyle,
        'traditional',
    )
    assert.equal(
        setHomeAlmanacOptions(
            normalized,
            'legacy',
            true,
            42 as never,
            'fancy' as never,
            'yes' as never,
        ).widgets[0].options?.almanacShowActivities,
        false,
    )
})

test('defaults the almanac term style and rejects unknown styles', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [
            { id: 'legacy', kind: 'almanac', size: '2x2' },
            {
                id: 'valid',
                kind: 'almanac',
                size: '2x2',
                options: { almanacTermStyle: 'traditional' },
            },
            {
                id: 'invalid',
                kind: 'almanac',
                size: '2x2',
                options: { almanacTermStyle: 'klingon' },
            },
        ],
    })!

    assert.deepEqual(
        normalized.widgets.map((widget) => widget.options?.almanacTermStyle),
        ['traditional', 'traditional', 'traditional'],
    )
    assert.equal(
        setHomeAlmanacOptions(normalized, 'legacy', true, 6, 'plain', false).widgets[0].options
            ?.almanacTermStyle,
        'plain',
    )
    // The setter clamps an unknown style back to the default.
    assert.equal(
        setHomeAlmanacOptions(normalized, 'legacy', true, 6, 'klingon' as never, false).widgets[0]
            .options?.almanacTermStyle,
        'traditional',
    )
})

test('accepts and resizes the recent widget to 3-column layouts', () => {
    const normalized = normalizeHomeDashboard({
        version: 1,
        widgets: [{ id: 'recent', kind: 'recent', size: '3x2' }],
    })!
    const config = createDefaultHomeDashboard()
    const recent = config.widgets.find((widget) => widget.kind === 'recent')!

    assert.equal(normalized.widgets[0].size, '3x2')
    assert.equal(
        resizeHomeWidget(config, recent.id, '3x1').widgets.find((widget) => widget.id === recent.id)
            ?.size,
        '3x1',
    )
})

test('keeps greeting text per account and falls back when none is saved', () => {
    const config = createDefaultHomeDashboard()
    const { widget, options } = greetingOptionsOf(config)

    assert.equal(getHomeGreetingOverride(options, 'account-a'), null)

    const saved = setHomeGreetingOverride(config, widget.id, 'account-a', {
        nickname: '  Alex  ',
        title: '',
    })
    const savedOptions = greetingOptionsOf(saved).options

    assert.deepEqual(getHomeGreetingOverride(savedOptions, 'account-a'), { nickname: 'Alex' })
    assert.equal(getHomeGreetingOverride(savedOptions, 'account-b'), null)
    // The nickname is one tab and the title another, so one must not invent the other.
    assert.equal(getHomeGreetingOverride(savedOptions, 'account-a')?.title, undefined)
})

test('clears an override when both fields are empty', () => {
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

    assert.equal(getHomeGreetingOverride(greetingOptionsOf(cleared).options, 'account-a'), null)
    assert.equal(greetingOptionsOf(cleared).options?.greetingOverrides, undefined)
})

test('clamps saved greeting text to the field limits', () => {
    const config = createDefaultHomeDashboard()
    const { widget } = greetingOptionsOf(config)

    const saved = setHomeGreetingOverride(config, widget.id, 'account-a', {
        nickname: 'n'.repeat(HOME_GREETING_NICKNAME_MAX + 20),
        title: 't'.repeat(HOME_GREETING_TITLE_MAX + 20),
    })
    const override = getHomeGreetingOverride(greetingOptionsOf(saved).options, 'account-a')

    assert.equal(override?.nickname?.length, HOME_GREETING_NICKNAME_MAX)
    assert.equal(override?.title?.length, HOME_GREETING_TITLE_MAX)
})

test('prunes overrides for accounts that no longer exist', () => {
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

    assert.deepEqual(getHomeGreetingOverride(options, 'account-a'), { nickname: 'Alex' })
    assert.equal(getHomeGreetingOverride(options, 'account-b'), null)
    // The unbound title was not written by an account, so it stays.
    assert.deepEqual(getHomeGreetingOverride(options, HOME_GREETING_GLOBAL_KEY), {
        title: 'Global title',
    })
    // An unchanged dashboard is returned as it is, so the caller can skip a write.
    assert.equal(pruneHomeGreetingOverrides(pruned, ['account-a']), pruned)
})

test('normalizes stored overrides and drops unusable entries', () => {
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

    assert.deepEqual(greetingOptionsOf(normalized).options?.greetingOverrides, {
        'account-a': { nickname: 'Alex' },
        'account-e': { nickname: 'Bea', title: 'Hello there' },
    })
})
