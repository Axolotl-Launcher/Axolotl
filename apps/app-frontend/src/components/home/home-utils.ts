import type { InjectionKey, Ref } from 'vue'

export type HomeTimeBucket = 'late-night' | 'dawn' | 'morning' | 'afternoon' | 'evening' | 'night'
export type PlaytimeView = 'month' | 'year'

export type HeatmapDay = {
    date: Date
    dateKey: string
    inPeriod: boolean
}

export type MinecraftAccountLike = {
    account_id?: string
    account_type?: string
    profile?: {
        id?: string
        name?: string
    }
}

export function getTimeBucket(date: Date): HomeTimeBucket {
    const hour = date.getHours()
    if (hour < 5) return 'late-night'
    if (hour < 8) return 'dawn'
    if (hour < 12) return 'morning'
    if (hour < 17) return 'afternoon'
    if (hour < 21) return 'evening'
    return 'night'
}

export function stableGreetingIndex(seed: string, count: number): number {
    if (count <= 0) return 0

    let hash = 0
    for (const character of seed) {
        hash = (hash * 31 + character.charCodeAt(0)) | 0
    }
    return Math.abs(hash) % count
}

export function toDateKey(date: Date): string {
    const year = date.getFullYear()
    const month = String(date.getMonth() + 1).padStart(2, '0')
    const day = String(date.getDate()).padStart(2, '0')
    return `${year}-${month}-${day}`
}

export function dateFromKey(dateKey: string): Date {
    const [year, month, day] = dateKey.split('-').map(Number)
    return new Date(year, month - 1, day, 12)
}

export function startOfPeriod(anchor: Date, view: PlaytimeView): Date {
    return view === 'month'
        ? new Date(anchor.getFullYear(), anchor.getMonth(), 1, 12)
        : new Date(anchor.getFullYear(), 0, 1, 12)
}

export function endOfPeriod(anchor: Date, view: PlaytimeView): Date {
    return view === 'month'
        ? new Date(anchor.getFullYear(), anchor.getMonth() + 1, 0, 12)
        : new Date(anchor.getFullYear(), 11, 31, 12)
}

export function shiftPeriod(anchor: Date, view: PlaytimeView, amount: number): Date {
    return view === 'month'
        ? new Date(anchor.getFullYear(), anchor.getMonth() + amount, 1, 12)
        : new Date(anchor.getFullYear() + amount, 0, 1, 12)
}

export function buildHeatmapDays(anchor: Date, view: PlaytimeView): HeatmapDay[] {
    const periodStart = startOfPeriod(anchor, view)
    const periodEnd = endOfPeriod(anchor, view)
    const periodStartKey = toDateKey(periodStart)
    const periodEndKey = toDateKey(periodEnd)
    const gridStart = new Date(periodStart)
    gridStart.setDate(periodStart.getDate() - ((periodStart.getDay() + 6) % 7))
    const gridEnd = new Date(periodEnd)
    gridEnd.setDate(periodEnd.getDate() + ((7 - gridEnd.getDay()) % 7))

    const days: HeatmapDay[] = []
    const cursor = new Date(gridStart)
    while (cursor <= gridEnd) {
        const date = new Date(cursor)
        const dateKey = toDateKey(date)
        days.push({
            date,
            dateKey,
            inPeriod: dateKey >= periodStartKey && dateKey <= periodEndKey,
        })
        cursor.setDate(cursor.getDate() + 1)
    }
    return days
}

/** How many coloured steps the playtime heatmap has above the empty one. */
export const HEAT_LEVELS = 4

/**
 * The least playtime that can be the top of the heatmap scale. With only a
 * day or two of data, a couple of ordinary sessions are not shown as the
 * deepest colour; in a busy month the busiest day sets the scale.
 */
export const HEAT_SCALE_FLOOR_SECONDS = 4 * 3600

/**
 * A day's step on the playtime heatmap, 0 to `HEAT_LEVELS`, scaled to the
 * busiest day in view (but never to less than four hours). Any play at all is
 * at least step 1, so a short session still shows; the scale is a square
 * root, so the long days stand clearly apart without the rest collapsing to
 * step 1.
 */
export function getHeatLevel(seconds: number, maxSeconds: number): number {
    if (!(seconds > 0)) return 0
    const top = Math.max(maxSeconds, HEAT_SCALE_FLOOR_SECONDS)
    const ratio = Math.min(1, seconds / top)
    return Math.min(HEAT_LEVELS, Math.max(1, Math.ceil(Math.sqrt(ratio) * HEAT_LEVELS)))
}

/** Week columns from the week holding `fromKey` to the week holding `toKey`, both counted. */
export function getWeekSpan(fromKey: string, toKey: string): number {
    const monday = (key: string) => {
        const date = dateFromKey(key)
        date.setDate(date.getDate() - ((date.getDay() + 6) % 7))
        return date.getTime()
    }
    return Math.round((monday(toKey) - monday(fromKey)) / (7 * 86400e3)) + 1
}

/** The fewest weeks a heatmap band draws, so a new player still sees a band. */
export const HEAT_BAND_MIN_WEEKS = 12

/**
 * How many week columns a band draws: back to the first week with play and
 * three more, at least `HEAT_BAND_MIN_WEEKS`; with no play at all, as many as
 * fit (`Infinity`, for the caller to cap).
 */
export function getBandColumns(dataWeeks: number): number {
    if (dataWeeks <= 0) return Number.POSITIVE_INFINITY
    return Math.max(HEAT_BAND_MIN_WEEKS, dataWeeks + 3)
}

/**
 * `weeks` columns of seven days (Monday first) ending with the week that holds
 * `end`, oldest first: the layout of a contribution-style heatmap band.
 */
export function buildHeatWeeks(end: Date, weeks: number): HeatmapDay[][] {
    const last = new Date(end.getFullYear(), end.getMonth(), end.getDate(), 12)
    const start = new Date(last)
    start.setDate(last.getDate() - ((last.getDay() + 6) % 7) - (weeks - 1) * 7)
    const columns: HeatmapDay[][] = []
    const cursor = new Date(start)
    for (let week = 0; week < weeks; week++) {
        const column: HeatmapDay[] = []
        for (let day = 0; day < 7; day++) {
            const date = new Date(cursor)
            column.push({ date, dateKey: toDateKey(date), inPeriod: true })
            cursor.setDate(cursor.getDate() + 1)
        }
        columns.push(column)
    }
    return columns
}

/** The longest run of consecutive days among the given played days. */
export function getLongestStreak(dateKeys: Iterable<string>): number {
    const days = [...new Set(dateKeys)].sort()
    let longest = 0
    let run = 0
    let previous: Date | null = null
    for (const key of days) {
        const date = dateFromKey(key)
        const next = previous ? new Date(previous) : null
        next?.setDate(next.getDate() + 1)
        run = next && toDateKey(next) === key ? run + 1 : 1
        longest = Math.max(longest, run)
        previous = date
    }
    return longest
}

export type ActivePlayerAccount = {
    id: string
    name: string
}

/**
 * The account the greeting is about: the selected one, but only when it can
 * carry a profile name. Offline accounts have no name, so they resolve to null
 * and the greeting falls back to the plain welcome.
 */
export function getActivePlayerAccount(
    selectedUser: string | null | undefined,
    accounts: readonly MinecraftAccountLike[],
): ActivePlayerAccount | null {
    if (!selectedUser) return null
    const account = accounts.find(
        (candidate) =>
            candidate.account_id === selectedUser &&
            (candidate.account_type === 'microsoft' || candidate.account_type === 'yggdrasil'),
    )
    const name = account?.profile?.name
    return name ? { id: selectedUser, name } : null
}

export function getActivePlayerName(
    selectedUser: string | null | undefined,
    accounts: readonly MinecraftAccountLike[],
): string | null {
    return getActivePlayerAccount(selectedUser, accounts)?.name ?? null
}

export type LaunchCandidate = {
    id: string
    last_played?: string | Date | null
}

function lastPlayedTime(candidate: LaunchCandidate): number {
    if (!candidate.last_played) return Number.NEGATIVE_INFINITY
    const time = new Date(candidate.last_played).getTime()
    return Number.isNaN(time) ? Number.NEGATIVE_INFINITY : time
}

/**
 * The instance Home offers to launch: the one chosen for Home, or else the one
 * played most recently. Returns null only when no instance was ever played and
 * none was chosen, so Home can ask the player to pick one.
 */
export function resolveHomeLaunchInstance<T extends LaunchCandidate>(
    instances: readonly T[],
    selectedId: string | null | undefined,
): { instance: T; chosen: boolean } | null {
    const selected = selectedId ? instances.find((instance) => instance.id === selectedId) : null
    if (selected) return { instance: selected, chosen: true }

    let latest: T | null = null
    for (const instance of instances) {
        if (lastPlayedTime(instance) === Number.NEGATIVE_INFINITY) continue
        if (!latest || lastPlayedTime(instance) > lastPlayedTime(latest)) latest = instance
    }
    return latest ? { instance: latest, chosen: false } : null
}

/** Other instances to switch to in one click, most recently played first. */
export function getQuickSwitchInstances<T extends LaunchCandidate>(
    instances: readonly T[],
    currentId: string | null | undefined,
    limit: number,
): T[] {
    return instances
        .filter((instance) => instance.id !== currentId)
        .map((instance, index) => ({ instance, index }))
        .sort(
            (left, right) =>
                lastPlayedTime(right.instance) - lastPlayedTime(left.instance) ||
                left.index - right.index,
        )
        .slice(0, Math.max(0, limit))
        .map(({ instance }) => instance)
}

export type HomeWorldRowState = {
    /** The row offers Stop instead of Play. */
    showStop: boolean
    playDisabled: boolean
    /** Why Play is disabled, for its tooltip. */
    reason: 'no-quick-play' | 'game-open' | 'world-in-use' | null
}

/**
 * Play or stop for a world or server row, by the rule the shared world rows
 * use: Stop belongs to the world the game was joined through (or to a world
 * the running game holds locked). Any other world of a running instance gets
 * a disabled Play that says the game is already open, so a running instance
 * does not turn every one of its worlds and servers into a Stop button.
 */
export function getHomeWorldRowState(input: {
    instanceRunning: boolean
    /** The game was started by joining this world from this widget. */
    joinedHere: boolean
    starting: boolean
    locked?: boolean
    supportsQuickPlay: boolean
}): HomeWorldRowState {
    const locked = input.locked === true
    const showStop = (input.joinedHere || (locked && input.instanceRunning)) && !input.starting
    if (showStop) return { showStop, playDisabled: false, reason: null }

    const playingOther = input.instanceRunning && !input.joinedHere
    const reason = !input.supportsQuickPlay
        ? 'no-quick-play'
        : playingOther
          ? 'game-open'
          : locked
            ? 'world-in-use'
            : null
    return {
        showStop: false,
        playDisabled: reason !== null || input.starting,
        reason,
    }
}

/**
 * How a collection widget lays out its items at a given size: a plain list in
 * one column, a strip of cards when it is wide but one row tall, and a grid of
 * tiles with the icon as the cover once it is both wide and tall.
 */
export type HomeItemVariant = 'row' | 'card' | 'tile'

/**
 * Where a widget's list puts its "+N" button: a slot in the widget header,
 * beside the count, provided by HomeWidgetFrame.
 */
export const homeWidgetMoreTargetKey: InjectionKey<Ref<HTMLElement | null>> =
    Symbol('home-widget-more-target')

export function getHomeItemVariant(size: string | null | undefined): HomeItemVariant {
    if (!size) return 'row'
    const [columns, rows] = size.split('x').map(Number)
    if (!(columns >= 2)) return 'row'
    return rows >= 2 ? 'tile' : 'card'
}

/**
 * How the calendar draws at a size: `week` (one tile: this week's seven
 * squares), `month` (one column: the month as a grid of weeks), `band` (wide
 * and tall: a band of week columns with the day's instances under it) or
 * `strip` (wide and one row: the band beside the figures).
 */
export type HomeCalendarLayout = 'week' | 'month' | 'band' | 'strip'

export function getCalendarLayout(size: string | null | undefined): HomeCalendarLayout {
    const [columns, rows] = (size ?? '1x2').split('x').map(Number)
    if (!(columns >= 2)) return rows === 1 ? 'week' : 'month'
    return rows === 1 ? 'strip' : 'band'
}

/** The share of a server's slots in use, 0 to 1; null when it does not say. */
export function getServerFill(online: number | null | undefined, max: number | null | undefined) {
    if (typeof online !== 'number' || typeof max !== 'number' || max <= 0) return null
    return Math.min(1, Math.max(0, online / max))
}
