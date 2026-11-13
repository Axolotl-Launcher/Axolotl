import { expect, it } from 'vitest'

import { HOME_WIDGET_DEFAULT_SIZE, type HomeWidgetKind } from './home-dashboard'
import {
    countHomeWidgets,
    createHomeWidget,
    filterHomeWidgetCatalog,
    HOME_WIDGET_CATALOG,
    homeWidgetNeedsTarget,
    homeWidgetPreviewSize,
    resolveHomeWidgetSize,
} from './home-widget-catalog'

it('offers every dashboard kind exactly once', () => {
    const kinds = HOME_WIDGET_CATALOG.map((entry) => entry.kind)
    expect(new Set(kinds).size).toBe(kinds.length)
    expect([...kinds].sort()).toEqual(Object.keys(HOME_WIDGET_DEFAULT_SIZE).sort())
})

it('asks for a target only for the single-item shortcuts', () => {
    const needing = HOME_WIDGET_CATALOG.filter((entry) => homeWidgetNeedsTarget(entry.kind))
    expect(needing.map((entry) => entry.kind)).toEqual(['instance', 'world', 'server'])
    expect(needing.every((entry) => entry.category === 'shortcuts')).toBe(true)
})

it('counts the kinds already on the Home', () => {
    const counts = countHomeWidgets([
        { id: 'a', kind: 'calendar', size: '1x2' },
        { id: 'b', kind: 'calendar', size: '1x2' },
        { id: 'c', kind: 'launch', size: '2x2' },
    ])
    expect(counts).toEqual({ calendar: 2, launch: 1 })
    expect(counts['pinned-worlds']).toBeUndefined()
})

it('filters by category and by name or description, ignoring case', () => {
    const entries = [
        {
            kind: 'calendar' as HomeWidgetKind,
            category: 'activity' as const,
            label: 'Calendar',
            description: 'Your month',
        },
        {
            kind: 'server' as HomeWidgetKind,
            category: 'shortcuts' as const,
            label: 'Single server',
            description: 'Join it',
        },
        {
            kind: 'pinned-servers' as HomeWidgetKind,
            category: 'collections' as const,
            label: 'All favorite servers',
            description: 'Collects them',
        },
    ]
    expect(filterHomeWidgetCatalog(entries, '', 'all')).toHaveLength(3)
    expect(filterHomeWidgetCatalog(entries, 'SERVER', 'all').map((entry) => entry.kind)).toEqual([
        'server',
        'pinned-servers',
    ])
    expect(
        filterHomeWidgetCatalog(entries, 'server', 'shortcuts').map((entry) => entry.kind),
    ).toEqual(['server'])
    expect(filterHomeWidgetCatalog(entries, 'month', 'all').map((entry) => entry.kind)).toEqual([
        'calendar',
    ])
    expect(filterHomeWidgetCatalog(entries, 'nothing', 'all')).toEqual([])
})

it('keeps a chosen size only when the kind allows it', () => {
    expect(resolveHomeWidgetSize('launch', '3x1')).toBe('3x1')
    expect(resolveHomeWidgetSize('calendar', '2x2')).toBe('1x2')
    expect(resolveHomeWidgetSize('recent', null)).toBe(HOME_WIDGET_DEFAULT_SIZE.recent)
})

it('creates widgets with the defaults of the default layout', () => {
    const recent = createHomeWidget('recent', '3x1')
    expect(recent).toMatchObject({ kind: 'recent', size: '3x1', options: { recentLimit: 4 } })
    expect(createHomeWidget('launch').options).toMatchObject({ greetingMode: 'greeting' })
    expect(createHomeWidget('calendar').target).toBeUndefined()
    const target = { instanceId: 'a', fallbackLabel: 'A' }
    expect(createHomeWidget('instance', '2x1', target)).toMatchObject({ size: '2x1', target })
    // A collection never carries a target, whatever is passed.
    expect(createHomeWidget('pinned-worlds', null, target).target).toBeUndefined()
    expect(createHomeWidget('calendar').id).not.toBe(createHomeWidget('calendar').id)
})

it('draws previews at the dashboard’s narrowest column', () => {
    expect(homeWidgetPreviewSize('1x1')).toEqual({ width: 240, height: 160 })
    expect(homeWidgetPreviewSize('2x2')).toEqual({ width: 496, height: 336 })
    expect(homeWidgetPreviewSize('3x1')).toEqual({ width: 752, height: 160 })
})
