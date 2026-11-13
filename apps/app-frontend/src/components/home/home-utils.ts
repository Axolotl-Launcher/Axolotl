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

export function getPlaytimeLevel(seconds: number): number {
    if (seconds <= 0) return 0
    if (seconds <= 30 * 60) return 1
    if (seconds <= 90 * 60) return 2
    if (seconds <= 180 * 60) return 3
    return 4
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
