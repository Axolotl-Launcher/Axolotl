/**
 * Port of apps/app-frontend/src/components/home/home-utils.ts, plus the world
 * and server helpers of helpers/worlds.ts (and app-lib's local-address check)
 * that the Home widgets use.
 */
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

/* ---------------------------------------------------------------- worlds -- */

export type WorldDisplayStatus = 'normal' | 'hidden' | 'favorite'
export type SingleplayerGameMode = 'survival' | 'creative' | 'adventure' | 'spectator'

type BaseWorld = {
    instance_id: string
    name: string
    /**
     * Singleplayer: the save's folder inside `saves/` (the app's `path`).
     * Server: the entry's place in `servers.dat`, so every world has a unique
     * path within its instance; servers are identified by `address`.
     */
    path: string
    last_played?: string
    icon?: string | null
    display_status: WorldDisplayStatus
}

export type SingleplayerWorld = BaseWorld & {
    type: 'singleplayer'
    game_mode: SingleplayerGameMode
    hardcore: boolean
    locked: boolean
}

export type ServerWorld = BaseWorld & {
    type: 'server'
    index: number
    address: string
    pack_status: 'enabled' | 'disabled' | 'prompt'
    project_id?: string
    content_kind?: string
}

/**
 * helpers/worlds.ts `WorldWithInstance`: the worlds of every instance in one
 * list, which the instance pages share (screens/instance/state.ts).
 */
export type HomeWorld = SingleplayerWorld | ServerWorld

export function getWorldIdentifier(world: HomeWorld): string {
    return world.type === 'singleplayer' ? world.path : world.address
}

const DEFAULT_MINECRAFT_SERVER_PORT = 25565

function parseServerPort(port: string): number | null {
    const parsed = Number.parseInt(port, 10)
    return Number.isInteger(parsed) && parsed > 0 && parsed <= 65535 ? parsed : null
}

function parseServerHost(address: string): string {
    const trimmedAddress = address.trim()
    if (!trimmedAddress) return ''

    if (trimmedAddress.startsWith('[')) {
        const closingBracket = trimmedAddress.indexOf(']')
        if (closingBracket > 0) {
            return trimmedAddress.slice(1, closingBracket).trim().toLowerCase()
        }
    }

    const firstColon = trimmedAddress.indexOf(':')
    const lastColon = trimmedAddress.lastIndexOf(':')

    if (firstColon !== -1 && firstColon === lastColon) {
        return trimmedAddress.slice(0, firstColon).trim().toLowerCase()
    }

    return trimmedAddress.toLowerCase()
}

/** Normalization converts addresses to a canonical form (lowercase-host:port, default port 25565). */
export function normalizeServerAddress(address: string): string {
    const trimmedAddress = address.trim()
    const host = parseServerHost(trimmedAddress)
    if (!host) return ''
    let port = DEFAULT_MINECRAFT_SERVER_PORT

    if (trimmedAddress.startsWith('[')) {
        const closingBracket = trimmedAddress.indexOf(']')
        if (closingBracket > 0) {
            const suffix = trimmedAddress.slice(closingBracket + 1)
            if (suffix.startsWith(':')) {
                const parsedPort = parseServerPort(suffix.slice(1))
                if (parsedPort != null) port = parsedPort
            }
        }
    } else {
        const firstColon = trimmedAddress.indexOf(':')
        const lastColon = trimmedAddress.lastIndexOf(':')
        if (firstColon !== -1 && firstColon === lastColon) {
            const parsedPort = parseServerPort(trimmedAddress.slice(firstColon + 1))
            if (parsedPort != null) port = parsedPort
        }
    }

    return `${host}:${port}`
}

/** Splits the host out of a server address: `host`, `host:port`, or `[v6]:port` (app-lib `address_host`). */
function addressHost(address: string): string {
    const trimmed = address.trim()
    if (!trimmed) return trimmed
    if (isIpAddress(trimmed)) return trimmed
    if (trimmed.startsWith('[')) {
        const end = trimmed.indexOf(']')
        if (end > 0) return trimmed.slice(1, end)
    }
    const separator = trimmed.lastIndexOf(':')
    if (separator > 0) {
        const port = trimmed.slice(separator + 1)
        if (/^\d*$/.test(port)) return trimmed.slice(0, separator)
    }
    return trimmed
}

function isIpv4(host: string): boolean {
    const segments = host.split('.')
    return (
        segments.length === 4 &&
        segments.every((segment) => /^\d{1,3}$/.test(segment) && Number(segment) <= 255)
    )
}

function isIpv6(host: string): boolean {
    return host.split(':').length >= 3 && /^[\da-f:.]+$/i.test(host)
}

function isIpAddress(host: string): boolean {
    return isIpv4(host) || isIpv6(host)
}

/**
 * app-lib `is_local_address`: localhost, a loopback or unspecified address.
 * (The desktop app also matches the machine's own interface addresses, which a
 * web page cannot list.)
 */
export function isLocalAddress(address: string): boolean {
    const host = addressHost(address).replace(/\.+$/, '').toLowerCase()
    if (!host) return false
    if (host === 'localhost' || host.endsWith('.localhost')) return true
    if (isIpv4(host)) return host.startsWith('127.') || host === '0.0.0.0'
    if (isIpv6(host)) {
        return host === '::1' || host === '::' || /^(0{1,4}:){7}0{0,3}[01]$/.test(host)
    }
    return false
}

/** The tag list's game versions, newest first, as `get_game_versions` returns them. */
export type GameVersionTag = { version: string }

export function hasServerQuickPlaySupport(gameVersions: GameVersionTag[], currentVersion: string) {
    if (!gameVersions.length) return true

    const versionIndex = gameVersions.findIndex((v) => v.version === currentVersion)
    const targetIndex = gameVersions.findIndex((v) => v.version === 'a1.0.5_01')

    return versionIndex === -1 || targetIndex === -1 || versionIndex <= targetIndex
}

export function hasWorldQuickPlaySupport(gameVersions: GameVersionTag[], currentVersion: string) {
    if (!gameVersions.length) return false

    const versionIndex = gameVersions.findIndex((v) => v.version === currentVersion)
    const targetIndex = gameVersions.findIndex((v) => v.version === '23w14a')

    return versionIndex !== -1 && targetIndex !== -1 && versionIndex <= targetIndex
}

/* ------------------------------------------------------------------- MOTD -- */

/*
 * Port of @sfirew/minecraft-motd-parser's `autoToHTML` (MIT), which the app
 * uses to render a server's description: legacy `§` codes and chat components.
 */
const LEGACY_STYLES: Record<string, string> = {
    '§k': 'obfuscated;',
    '§l': 'font-weight: bold;',
    '§m': 'text-decoration: line-through;',
    '§n': 'text-decoration: underline;',
    '§o': 'font-style: italic;',
    '§r': 'color: inherit;text-decoration: none !important;font-weight:normal!important;font-style: normal!important;',
}

const JSON_STYLES: Record<string, string> = {
    bold: 'font-weight: bold;',
    italic: 'font-style: italic;',
    underlined: 'text-decoration:underline;',
    strikethrough: 'text-decoration: line-through;',
    obfuscated: 'mc_obfuscated;',
    reset: 'color: inherit;text-decoration: none !important;font-weight:normal!important;font-style: normal!important;',
}

const LEGACY_COLORS: Record<string, string> = {
    '§0': '#000000',
    '§1': '#0000AA',
    '§2': '#00AA00',
    '§3': '#00AAAA',
    '§4': '#AA0000',
    '§5': '#AA00AA',
    '§6': '#FFAA00',
    '§7': '#AAAAAA',
    '§8': '#555555',
    '§9': '#5555FF',
    '§a': '#55FF55',
    '§b': '#55FFFF',
    '§c': '#FF5555',
    '§d': '#FF55FF',
    '§e': '#FFFF55',
    '§f': '#FFFFFF',
}

const NAMED_COLORS: Record<string, string> = {
    black: '#000000',
    dark_blue: '#0000AA',
    dark_green: '#00AA00',
    dark_aqua: '#00AAAA',
    dark_red: '#AA0000',
    dark_purple: '#AA00AA',
    gold: '#FFAA00',
    gray: '#AAAAAA',
    dark_gray: '#555555',
    blue: '#5555FF',
    green: '#55FF55',
    aqua: '#55FFFF',
    red: '#FF5555',
    light_purple: '#FF55FF',
    yellow: '#FFFF55',
    white: '#FFFFFF',
}

function escapeMotd(text: string): string {
    return text
        .replace(/&(?!(?:amp|lt|gt|quot|#39|#x[0-9A-Fa-f]+|#[0-9]+);)/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;')
        .replace(/\n/g, '<br/>')
}

function legacyToHtml(motd: string): string {
    const parts = motd.split(/([§][0-9a-fA-FklmnorKLMNOR])/).filter((part) => part !== '')
    let fontStyle = ''
    let color = ''
    let html = ''
    for (const part of parts) {
        const code = part.toLowerCase()
        if (Object.hasOwn(LEGACY_COLORS, code)) {
            color = LEGACY_COLORS[code]
            if (code === '§f') fontStyle = ''
        } else if (Object.hasOwn(LEGACY_STYLES, code)) {
            if (code === '§r') {
                color = ''
                fontStyle = ''
            } else {
                fontStyle += LEGACY_STYLES[code]
            }
        } else {
            const colorStyle = color ? `color:${color};` : ''
            const text = escapeMotd(part)
            html +=
                colorStyle || fontStyle
                    ? `<span style="${colorStyle}${fontStyle}">${text}</span>`
                    : text
        }
    }
    return html
}

type MotdComponent = { [key: string]: unknown; text?: unknown; extra?: unknown }

function isMotdComponent(value: unknown): value is MotdComponent {
    if (!value || typeof value !== 'object' || Array.isArray(value)) return false
    return (
        'text' in value ||
        'translate' in value ||
        ('extra' in value && Array.isArray((value as MotdComponent).extra))
    )
}

function componentToHtml(source: MotdComponent): string {
    let html = ''
    let colorStyle = ''
    let fontStyle = ''
    for (const rawKey of Object.keys(source)) {
        const key = rawKey.toLowerCase()
        if (Object.hasOwn(JSON_STYLES, key) && source[rawKey]) fontStyle += JSON_STYLES[key]
        if (key === 'color' && typeof source[rawKey] === 'string') {
            const value = source[rawKey] as string
            let hex = ''
            if (Object.hasOwn(NAMED_COLORS, value)) hex = NAMED_COLORS[value]
            else if (Object.hasOwn(LEGACY_COLORS, value)) hex = LEGACY_COLORS[value]
            else {
                const match = value.match(/^#([-+]?0+|\+?0*[1-9A-Fa-f][0-9A-Fa-f]{0,5})$/)
                if (match) hex = '#' + match[1].replace(/^[-+]?0*/, '').padStart(6, '0')
            }
            if (hex) colorStyle = `color:${hex};`
        }
        if (key === 'extra' && Array.isArray(source.extra)) {
            if (typeof source.text === 'string' || typeof source.text === 'number') {
                html += legacyToHtml(String(source.text))
            }
            for (const extra of source.extra) {
                if (typeof extra === 'string') html += legacyToHtml(extra)
                else if (isMotdComponent(extra)) html += componentToHtml(extra)
            }
        }
    }
    if (source.extra === undefined && source.text !== undefined) {
        if (typeof source.text === 'string' || typeof source.text === 'number') {
            html += legacyToHtml(String(source.text))
        }
    }
    return fontStyle || colorStyle ? `<span style="${colorStyle}${fontStyle}">${html}</span>` : html
}

export function motdToHtml(motd: string | object): string {
    if (typeof motd === 'object' && motd !== null) return componentToHtml(motd as MotdComponent)
    if (typeof motd === 'string') return legacyToHtml(motd)
    return ''
}

/** `getPingLevel` of @modrinth/utils, which picks the bars the signal icon tints. */
export function getPingLevel(ping: number): 1 | 2 | 3 | 4 | 5 {
    if (ping < 120) return 5
    if (ping < 200) return 4
    if (ping < 300) return 3
    if (ping < 400) return 2
    return 1
}
