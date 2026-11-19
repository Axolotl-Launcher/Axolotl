import {
    HOME_GREETING_DEFAULT_FONT,
    HOME_GREETING_DEFAULT_FONT_SIZE,
    HOME_GREETING_DEFAULT_MODE,
    HOME_RECENT_DEFAULT_LIMIT,
    HOME_WIDGET_DEFAULT_SIZE,
    HOME_WIDGET_GRID_GAP,
    HOME_WIDGET_GRID_ROW_HEIGHT,
    HOME_WIDGET_SIZE_OPTIONS,
    homeWidgetCarriesGreeting,
    type HomeWidgetKind,
    type HomeWidgetPlacement,
    type HomeWidgetSize,
    type HomeWidgetTarget,
} from './home-dashboard'

/**
 * What the widget picker offers, grouped the way people look for it: ways to
 * start playing, their play data, the things they collected, and shortcuts to
 * one thing. The sidebar cards (insights, the daily challenge, the news) are
 * not dashboard widgets, so they are not in it.
 */
export const HOME_WIDGET_CATEGORIES = ['start', 'activity', 'collections', 'shortcuts'] as const
export type HomeWidgetCategory = (typeof HOME_WIDGET_CATEGORIES)[number]

export const HOME_WIDGET_CATALOG: readonly {
    kind: HomeWidgetKind
    category: HomeWidgetCategory
}[] = [
    { kind: 'launch', category: 'start' },
    { kind: 'recent', category: 'start' },
    { kind: 'greeting', category: 'start' },
    { kind: 'calendar', category: 'activity' },
    { kind: 'pinned-instances', category: 'collections' },
    { kind: 'pinned-worlds', category: 'collections' },
    { kind: 'pinned-servers', category: 'collections' },
    { kind: 'instance', category: 'shortcuts' },
    { kind: 'world', category: 'shortcuts' },
    { kind: 'server', category: 'shortcuts' },
]

/** Shortcut widgets point at one thing, so adding one takes a second choice. */
export function homeWidgetNeedsTarget(kind: HomeWidgetKind): boolean {
    return kind === 'instance' || kind === 'world' || kind === 'server'
}

/** How many of each kind the Home already has, for the picker's "added" marks. */
export function countHomeWidgets(
    widgets: readonly HomeWidgetPlacement[],
): Partial<Record<HomeWidgetKind, number>> {
    const counts: Partial<Record<HomeWidgetKind, number>> = {}
    for (const widget of widgets) counts[widget.kind] = (counts[widget.kind] ?? 0) + 1
    return counts
}

/**
 * Narrows the catalog to a category and a search. The search matches the
 * name and the description, ignoring case, so "server" finds both server
 * widgets and "play" finds what mentions playing.
 */
export function filterHomeWidgetCatalog<T extends { category: HomeWidgetCategory }>(
    entries: readonly (T & { label: string; description: string })[],
    query: string,
    category: HomeWidgetCategory | 'all',
    locale = 'en-US',
): (T & { label: string; description: string })[] {
    const needle = query.trim().toLocaleLowerCase(locale)
    return entries.filter(
        (entry) =>
            (category === 'all' || entry.category === category) &&
            (!needle ||
                entry.label.toLocaleLowerCase(locale).includes(needle) ||
                entry.description.toLocaleLowerCase(locale).includes(needle)),
    )
}

/** A size the kind can take: the requested one when it is allowed, else its default. */
export function resolveHomeWidgetSize(
    kind: HomeWidgetKind,
    size: HomeWidgetSize | null | undefined,
): HomeWidgetSize {
    return size && HOME_WIDGET_SIZE_OPTIONS[kind].includes(size)
        ? size
        : HOME_WIDGET_DEFAULT_SIZE[kind]
}

/** A new widget as the picker adds it, with the same defaults the default layout uses. */
export function createHomeWidget(
    kind: HomeWidgetKind,
    size?: HomeWidgetSize | null,
    target?: HomeWidgetTarget | null,
): HomeWidgetPlacement {
    return {
        id: crypto.randomUUID(),
        kind,
        size: resolveHomeWidgetSize(kind, size),
        ...(target && homeWidgetNeedsTarget(kind) ? { target } : {}),
        ...(kind === 'recent' ? { options: { recentLimit: HOME_RECENT_DEFAULT_LIMIT } } : {}),
        ...(homeWidgetCarriesGreeting(kind)
            ? {
                  options: {
                      greetingMode: HOME_GREETING_DEFAULT_MODE,
                      greetingFont: HOME_GREETING_DEFAULT_FONT,
                      greetingFontSize: HOME_GREETING_DEFAULT_FONT_SIZE,
                  },
              }
            : {}),
    }
}

/** The narrowest column the dashboard lays out, which the previews are drawn at. */
export const HOME_WIDGET_PREVIEW_COLUMN = 240

/** A widget's size in pixels on a dashboard of the narrowest columns. */
export function homeWidgetPreviewSize(size: HomeWidgetSize): { width: number; height: number } {
    const [columns, rows] = size.split('x').map(Number)
    return {
        width: columns * HOME_WIDGET_PREVIEW_COLUMN + (columns - 1) * HOME_WIDGET_GRID_GAP,
        height: rows * HOME_WIDGET_GRID_ROW_HEIGHT + (rows - 1) * HOME_WIDGET_GRID_GAP,
    }
}
