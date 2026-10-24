import { computed, effectScope, reactive, ref, watch } from 'vue'

import { loadTags } from '../../data'
import type { PreviewState } from '../../lib/state'
import { usePreviewDomain } from '../../lib/state'
import type { ManagedServer, ManagedServerManifest } from '../../lib/state/servers'
import { serverStatusSnapshot } from '../../lib/state/servers'
import type { GameInstance } from '../../lib/types'
import { createDefaultHomeDashboard, type HomeDashboardConfig } from './home-dashboard'
import {
    type GameVersionTag,
    getWorldIdentifier,
    type HomeWorld,
    isLocalAddress,
    motdToHtml,
    normalizeServerAddress,
    type ServerWorld,
    type SingleplayerWorld,
    toDateKey,
} from './home-utils'

const MINUTE = 60 * 1000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/** The app's status-ping timeout (app-lib `SERVER_STATUS_TIMEOUT`), paid twice with the legacy retry. */
const SERVER_STATUS_TIMEOUT_MS = 5000

const PENDING_SERVER_DATA: { readonly refreshing: true } = Object.freeze({ refreshing: true })

/* ----------------------------------------------------------------- types -- */

/** helpers/instance.ts `DailyPlaytime`: one day of the playtime table. */
export type DailyPlaytime = {
    date: string
    played_seconds: number
    session_count: number
    top_instance_id: string | null
    top_instance_name: string | null
}

/** helpers/instance.ts `DailyPlaytimeEntry`: one instance's share of a day. */
export type DailyPlaytimeEntry = {
    instance_id: string
    instance_name: string
    played_seconds: number
    session_count: number
}

/** One successful launch, the unit the app's playtime table records when a game exits. */
export type HomeSession = {
    instance_id: string
    instance_name: string
    started_at: number
    seconds: number
}

/** helpers/worlds.ts `ServerStatus`, filled from the Server List Ping snapshot. */
export type HomeServerStatus = {
    description?: unknown
    players?: { max: number; online: number; sample: { name: string; id: string }[] }
    version?: { name: string; protocol: number; legacy: boolean }
    favicon?: string
    enforces_secure_chat: boolean
    ping?: number
}

/** helpers/worlds.ts `ServerData`. */
export type HomeServerData = {
    refreshing: boolean
    lastSuccessfulRefresh?: number
    status?: HomeServerStatus
    rawMotd?: unknown
    renderedMotd?: string
}

export type HomeNewsItem = {
    title: string
    category: string | null
    tag: string | null
    date: string | null
    image_url: string | null
    read_more_url: string
}

/** helpers/servers.ts `ServerInfoData`, reduced to what the Home widgets read. */
export type HomeLocalServer = {
    id: string
    name: string
    serverType: ManagedServerManifest['serverType']
    gameVersion: string
    port: number
    running: boolean
    starting?: boolean
    stopping?: boolean
    iconPath: string | null
    homePinned: boolean
    onlineMode?: boolean | null
    linkedWorld: { instanceId: string; address: string } | null
}

/* ----------------------------------------------------------------- seeds -- */

/**
 * The worlds of the preview user's instances: the saves and servers.dat
 * entries the app lists for each instance. Home and the instance pages read
 * this one list, as the app reads one backend. Each instance's latest world is
 * the one it was last played in; a few are favorites, which pins them to Home.
 */
function seedWorlds(now: number): HomeWorld[] {
    const ago = (offset: number) => new Date(now - offset).toISOString()
    const singleplayer = (
        instanceId: string,
        name: string,
        lastPlayed: number,
        extra: Partial<SingleplayerWorld> = {},
    ): SingleplayerWorld => ({
        type: 'singleplayer',
        instance_id: instanceId,
        name,
        path: name,
        game_mode: 'survival',
        hardcore: false,
        locked: false,
        icon: null,
        display_status: 'normal',
        last_played: ago(lastPlayed),
        ...extra,
    })
    const server = (
        instanceId: string,
        name: string,
        address: string,
        index: number,
        lastPlayed: number,
        extra: Partial<ServerWorld> = {},
    ): ServerWorld => ({
        type: 'server',
        instance_id: instanceId,
        name,
        address,
        index,
        path: `servers.dat#${index}`,
        pack_status: 'prompt',
        icon: null,
        display_status: 'normal',
        last_played: ago(lastPlayed),
        ...extra,
    })
    const cobblemonIcon = '/launcher-preview/projects/5FFgwNNP.webp'

    return [
        singleplayer('fabulously-optimized', 'New World', 5 * MINUTE, {
            game_mode: 'creative',
            display_status: 'favorite',
        }),
        singleplayer('fabulously-optimized', '生存挑战', 3 * DAY),
        singleplayer('fabulously-optimized', 'Hardcore Run', 11 * DAY, { hardcore: true }),
        server('fabulously-optimized', 'Cobblemon SMP', 'play.cobblemon.com', 0, 30 * HOUR, {
            icon: cobblemonIcon,
        }),
        server('fabulously-optimized', 'Hypixel', 'mc.hypixel.net', 1, 9 * DAY, {
            display_status: 'favorite',
        }),
        singleplayer('cobblemon', 'PokéWorld', 26 * HOUR, { display_status: 'favorite' }),
        singleplayer('cobblemon', 'Team Rocket', 6 * DAY),
        server('cobblemon', 'Cobblemon SMP', 'play.cobblemon.com', 0, 30 * HOUR, {
            icon: cobblemonIcon,
            display_status: 'favorite',
        }),
        // The local server the user hosts on this machine (Multiplayer's "Cobblemon SMP").
        server('cobblemon', '本地测试服', 'localhost:25565', 1, 4 * DAY, {
            display_status: 'favorite',
        }),
        singleplayer('fabric-26-3', 'Redstone Lab', 2 * DAY, { game_mode: 'creative' }),
        singleplayer('fabric-26-3', 'New World', 6 * DAY),
        singleplayer('better-mc', 'Better World', 3 * DAY),
        singleplayer('zombie-invade', '100 Days', 9 * DAY, {
            hardcore: true,
            display_status: 'favorite',
        }),
        singleplayer('vanilla', 'World', 16 * DAY, { game_mode: 'creative' }),
    ]
}

/**
 * The playtime table: every launch of the last weeks, ending when the world it
 * opened was last saved and adding up to each instance's recorded time played.
 */
const SESSION_SEED: [instanceId: string, endedAgo: number, minutes: number][] = [
    ['fabulously-optimized', 5 * MINUTE, 48],
    ['fabulously-optimized', 3 * DAY, 64],
    ['fabulously-optimized', 9 * DAY, 40],
    ['fabulously-optimized', 11 * DAY, 70],
    ['cobblemon', 26 * HOUR, 95],
    ['cobblemon', 30 * HOUR, 110],
    ['cobblemon', 6 * DAY, 120],
    ['cobblemon', 8 * DAY, 130],
    ['cobblemon', 10 * DAY, 105],
    ['cobblemon', 13 * DAY, 140],
    ['cobblemon', 15 * DAY, 90],
    ['cobblemon', 18 * DAY, 125],
    ['cobblemon', 21 * DAY, 150],
    ['cobblemon', 24 * DAY, 100],
    ['cobblemon', 28 * DAY, 135],
    ['cobblemon', 33 * DAY, 120],
    ['cobblemon', 37 * DAY, 95],
    ['cobblemon', 41 * DAY, 105],
    ['fabric-26-3', 2 * DAY, 70],
    ['fabric-26-3', 4 * DAY, 95],
    ['fabric-26-3', 6 * DAY, 80],
    ['fabric-26-3', 8 * DAY, 65],
    ['better-mc', 3 * DAY + 3 * HOUR, 85],
    ['better-mc', 5 * DAY, 95],
    ['better-mc', 12 * DAY, 110],
    ['better-mc', 19 * DAY, 90],
    ['better-mc', 27 * DAY, 75],
    ['better-mc', 35 * DAY, 100],
    ['zombie-invade', 9 * DAY, 60],
    ['zombie-invade', 22 * DAY, 55],
    ['zombie-invade', 30 * DAY, 53],
    ['vanilla', 16 * DAY, 41],
]

function seedSessions(now: number, instances: GameInstance[]): HomeSession[] {
    const names = new Map(instances.map((instance) => [instance.id, instance.name]))
    return SESSION_SEED.flatMap(([instanceId, endedAgo, minutes]) => {
        const name = names.get(instanceId)
        if (!name) return []
        return [
            {
                instance_id: instanceId,
                instance_name: name,
                started_at: now - endedAgo - minutes * MINUTE,
                seconds: minutes * 60,
            },
        ]
    })
}

/* ----------------------------------------------------------------- state -- */

export function createHomeState(preview: PreviewState) {
    const scope = effectScope(true)
    const now = Date.now()

    const worlds = ref<HomeWorld[]>(seedWorlds(now))
    const sessions = ref<HomeSession[]>(seedSessions(now, preview.instances.instances.value))
    const dashboard = ref<HomeDashboardConfig>(
        createDefaultHomeDashboard(
            preview.settings.feature_flags.worlds_in_home ??
                preview.settings.worlds_in_home ??
                true,
        ),
    )
    const dashboardEditing = ref(false)
    const minimalInstanceId = ref<string | null>(null)
    const instanceRevision = ref(0)
    const gameVersions = ref<GameVersionTag[]>([])

    const tagsReady = loadTags()
        .then((tags) => {
            gameVersions.value = tags.gameVersions
        })
        .catch(() => undefined)

    /* -- playtime ---------------------------------------------------------- */

    const dailyPlaytime = computed(() => {
        const days = new Map<string, Map<string, DailyPlaytimeEntry>>()
        for (const session of sessions.value) {
            const key = toDateKey(new Date(session.started_at))
            let entries = days.get(key)
            if (!entries) {
                entries = new Map()
                days.set(key, entries)
            }
            const entry = entries.get(session.instance_id)
            if (entry) {
                entry.played_seconds += session.seconds
                entry.session_count += 1
            } else {
                entries.set(session.instance_id, {
                    instance_id: session.instance_id,
                    instance_name: session.instance_name,
                    played_seconds: session.seconds,
                    session_count: 1,
                })
            }
        }
        return days
    })

    /** `get_daily_playtime(start, end)`: the days of the range that have playtime. */
    function getDailyPlaytime(start: string, end: string): DailyPlaytime[] {
        const result: DailyPlaytime[] = []
        for (const [date, entries] of dailyPlaytime.value) {
            if (date < start || date > end) continue
            let played = 0
            let count = 0
            let top: DailyPlaytimeEntry | null = null
            for (const entry of entries.values()) {
                played += entry.played_seconds
                count += entry.session_count
                if (!top || entry.played_seconds > top.played_seconds) top = entry
            }
            result.push({
                date,
                played_seconds: played,
                session_count: count,
                top_instance_id: top?.instance_id ?? null,
                top_instance_name: top?.instance_name ?? null,
            })
        }
        return result.sort((a, b) => a.date.localeCompare(b.date))
    }

    /** `get_daily_playtime_details(date)`: the instances played that day, most played first. */
    function getDailyPlaytimeDetails(date: string): DailyPlaytimeEntry[] {
        return [...(dailyPlaytime.value.get(date)?.values() ?? [])]
            .map((entry) => ({ ...entry }))
            .sort((a, b) => b.played_seconds - a.played_seconds)
    }

    /* -- processes: playtime and world activity of launches -------------- */

    /** Launches that should mark a world as played once the game is up. */
    const pendingJoins = new Map<
        string,
        { instanceId: string; type: HomeWorld['type']; identifier: string }
    >()
    const tracked = new Map<
        string,
        { instanceId: string; instanceName: string; runningSince: number | null }
    >()

    function markWorldPlayed(instanceId: string, type: HomeWorld['type'], identifier: string) {
        const timestamp = new Date().toISOString()
        worlds.value = worlds.value.map((world) =>
            world.instance_id === instanceId &&
            world.type === type &&
            (world.type === 'server'
                ? normalizeServerAddress(world.address) === normalizeServerAddress(identifier)
                : world.path === identifier)
                ? { ...world, last_played: timestamp }
                : world,
        )
    }

    function syncProcesses() {
        const present = new Set<string>()
        for (const process of preview.processes.processes.value) {
            present.add(process.uuid)
            let entry = tracked.get(process.uuid)
            if (!entry) {
                entry = {
                    instanceId: process.instance.id,
                    instanceName: process.instance.name,
                    runningSince: null,
                }
                tracked.set(process.uuid, entry)
            }
            if (entry.runningSince === null && process.stage === 'running') {
                entry.runningSince = Date.now()
                const join = pendingJoins.get(process.instance.id)
                if (join) {
                    pendingJoins.delete(process.instance.id)
                    markWorldPlayed(join.instanceId, join.type, join.identifier)
                    instanceRevision.value += 1
                }
            }
        }
        for (const [uuid, entry] of tracked) {
            if (present.has(uuid)) continue
            tracked.delete(uuid)
            if (entry.runningSince === null) continue
            sessions.value = [
                ...sessions.value,
                {
                    instance_id: entry.instanceId,
                    instance_name: entry.instanceName,
                    started_at: entry.runningSince,
                    seconds: Math.max(1, Math.round((Date.now() - entry.runningSince) / 1000)),
                },
            ]
            instanceRevision.value += 1
        }
    }

    scope.run(() => {
        watch(
            () => [preview.processes.processes.value, preview.processes.pendingInstanceId.value],
            syncProcesses,
            { immediate: true },
        )
        // The app clears the minimal Home instance when that instance is gone.
        watch(
            () => preview.instances.instances.value.map((instance) => instance.id),
            (ids) => {
                if (minimalInstanceId.value && !ids.includes(minimalInstanceId.value)) {
                    minimalInstanceId.value = null
                }
            },
        )
    })

    /* -- worlds ------------------------------------------------------------ */

    const loadedWorlds = new Set<string>()
    const worldRequests = new Map<string, Promise<HomeWorld[]>>()

    function delay(ms: number) {
        return new Promise<void>((resolve) => preview.clock.after(ms, resolve))
    }

    function instanceWorlds(instanceId: string) {
        return worlds.value.filter((world) => world.instance_id === instanceId)
    }

    /** `get_instance_worlds`: reads the saves and servers.dat of an instance once, then caches. */
    async function getInstanceWorlds(instanceId: string, force = false): Promise<HomeWorld[]> {
        if (!force && loadedWorlds.has(instanceId)) return instanceWorlds(instanceId)
        const pending = worldRequests.get(instanceId)
        if (pending) return pending
        const request = delay(160)
            .then(() => {
                loadedWorlds.add(instanceId)
                return instanceWorlds(instanceId)
            })
            .finally(() => worldRequests.delete(instanceId))
        worldRequests.set(instanceId, request)
        return request
    }

    function byLastPlayed(a: HomeWorld, b: HomeWorld) {
        return Date.parse(b.last_played ?? '') - Date.parse(a.last_played ?? '') || 0
    }

    function withKnownInstance(world: HomeWorld) {
        return preview.instances.instances.value.some(
            (instance) => instance.id === world.instance_id,
        )
    }

    /** `get_favorite_worlds`: favorites of every instance, most recently played first. */
    const favoriteWorlds = computed(() =>
        worlds.value
            .filter((world) => world.display_status === 'favorite' && withKnownInstance(world))
            .sort((a, b) => {
                if (!a.last_played) return 1
                if (!b.last_played) return -1
                return byLastPlayed(a, b)
            }),
    )

    /** `get_recent_worlds(8, ['normal', 'favorite'])`. */
    const recentWorlds = computed(() =>
        worlds.value
            .filter((world) => world.display_status !== 'hidden' && withKnownInstance(world))
            .sort((a, b) => {
                if (!a.last_played) return 1
                if (!b.last_played) return -1
                return byLastPlayed(a, b)
            })
            .slice(0, 8),
    )

    function setWorldDisplayStatus(
        instanceId: string,
        type: HomeWorld['type'],
        identifier: string,
        status: HomeWorld['display_status'],
    ) {
        worlds.value = worlds.value.map((world) =>
            world.instance_id === instanceId &&
            world.type === type &&
            getWorldIdentifier(world) === identifier
                ? { ...world, display_status: status }
                : world,
        )
    }

    /* -- server status ---------------------------------------------------- */

    const serverData = reactive<Record<string, HomeServerData>>({})
    const loadedServers = new Set<string>()
    const serverRequests = new Map<string, Promise<void>>()

    function serverKey(instanceId: string, address: string) {
        return `${instanceId}:${address}`
    }

    /**
     * The status of a server entry; `{ refreshing: true }` until its first
     * refresh starts. Reading never writes, so templates and computeds can call it.
     */
    function getServerData(instanceId: string, address: string): HomeServerData {
        return serverData[serverKey(instanceId, address)] ?? PENDING_SERVER_DATA
    }

    function ensureServerData(instanceId: string, address: string): HomeServerData {
        const key = serverKey(instanceId, address)
        serverData[key] ??= { refreshing: true }
        // Read back through the reactive record so later writes are tracked.
        return serverData[key]
    }

    /** Port of the local server listening on a loopback address, if any. */
    function localServerAt(address: string): ManagedServer | null {
        if (!isLocalAddress(address)) return null
        const port = Number(normalizeServerAddress(address).split(':').pop())
        return preview.servers.managed.find((server) => server.port === port) ?? null
    }

    /**
     * `get_server_status`: answers from the one-time Server List Ping snapshot
     * after a round trip, or fails after the status timeout for a host that did
     * not answer. A local address reaches the simulated local server.
     */
    async function pingServer(
        address: string,
        simulateNetwork: boolean,
    ): Promise<HomeServerStatus> {
        const local = localServerAt(address)
        if (isLocalAddress(address)) {
            if (simulateNetwork) await delay(40)
            if (!local || local.phase !== 'running') throw new Error('Connection refused')
            // The server answers from its server.properties.
            const [{ servers }, { getProperty, parseProperties }] = await Promise.all([
                import('../multiplayer/backend.ts'),
                import('../../lib/server-core.ts'),
            ])
            const properties = parseProperties(
                await servers.readFile(local.manifest.id, 'server.properties').catch(() => ''),
            )
            return {
                description: getProperty(properties, 'motd') ?? 'A Minecraft Server',
                players: {
                    online: 0,
                    max: Number(getProperty(properties, 'max-players') ?? 20),
                    sample: [],
                },
                version: {
                    name: local.manifest.gameVersion,
                    protocol: local.protocol ?? 0,
                    legacy: false,
                },
                enforces_secure_chat: getProperty(properties, 'enforce-secure-profile') !== 'false',
                ping: 1,
            }
        }
        const host = normalizeServerAddress(address).replace(/:25565$/, '')
        const snapshot = serverStatusSnapshot(host) as unknown as {
            online: boolean
            version?: string
            protocol?: number
            players?: { online: number; max: number }
            motd?: unknown
            latency_ms?: number
            favicon?: string
        } | null
        if (!snapshot?.online) {
            if (simulateNetwork) await delay(SERVER_STATUS_TIMEOUT_MS)
            throw new Error('Server status timed out')
        }
        const latency = snapshot.latency_ms ?? 100
        if (simulateNetwork) await delay(120 + latency * 2)
        return {
            description: snapshot.motd,
            players: {
                online: snapshot.players?.online ?? 0,
                max: snapshot.players?.max ?? 0,
                sample: [],
            },
            version: {
                name: snapshot.version ?? '',
                protocol: snapshot.protocol ?? 0,
                legacy: false,
            },
            favicon: snapshot.favicon,
            enforces_secure_chat: false,
            ping: latency,
        }
    }

    async function refreshServerData(
        data: HomeServerData,
        address: string,
        simulateNetwork: boolean,
        legacy = false,
    ) {
        const refreshTime = Date.now()
        data.refreshing = true
        try {
            const status = await pingServer(address, simulateNetwork)
            if (data.lastSuccessfulRefresh && data.lastSuccessfulRefresh > refreshTime) return
            data.lastSuccessfulRefresh = Date.now()
            data.status = status
            if (status.description) {
                data.rawMotd = status.description
                data.renderedMotd = motdToHtml(status.description as string | object)
            } else {
                delete data.rawMotd
                delete data.renderedMotd
            }
        } catch {
            // The app retries once with the legacy ping before giving up.
            if (simulateNetwork && !legacy) {
                await refreshServerData(data, address, simulateNetwork, true)
                return
            }
            if (!data.lastSuccessfulRefresh || data.lastSuccessfulRefresh <= refreshTime) {
                delete data.status
                delete data.rawMotd
                delete data.renderedMotd
            }
        } finally {
            data.refreshing = false
        }
    }

    async function refreshServer(instanceId: string, address: string, force = false) {
        const key = serverKey(instanceId, address)
        if (!force && loadedServers.has(key)) return
        const pending = serverRequests.get(key)
        if (pending) return pending

        const data = ensureServerData(instanceId, address)
        const request = (async () => {
            // `get_instance_protocol_version` resolves the protocol to ping with.
            if (force) await delay(30)
            // Initial status is already available in the packaged snapshot.
            // Only an explicit refresh simulates the desktop network round trip.
            await refreshServerData(data, address, force)
            loadedServers.add(key)
        })().finally(() => serverRequests.delete(key))
        serverRequests.set(key, request)
        return request
    }

    function warmServerData(list: HomeWorld[]) {
        for (const world of list) {
            if (world.type === 'server') void refreshServer(world.instance_id, world.address)
        }
    }

    scope.run(() => {
        watch(favoriteWorlds, warmServerData, { immediate: true })
        watch(recentWorlds, warmServerData, { immediate: true })
    })
    const initialServersReady = Promise.all([...serverRequests.values()])

    /* -- local servers ---------------------------------------------------- */

    const localServers = computed<HomeLocalServer[]>(() =>
        preview.servers.managed.map((server) => ({
            id: server.manifest.id,
            name: server.manifest.name,
            serverType: server.manifest.serverType,
            gameVersion: server.manifest.gameVersion,
            port: server.port ?? 25565,
            running: server.phase === 'running' || server.phase === 'stopping',
            starting: server.phase === 'starting',
            stopping: server.phase === 'stopping',
            iconPath: server.manifest.iconPath ?? null,
            homePinned: Boolean(server.manifest.homePinnedAt),
            onlineMode: server.onlineMode,
            linkedWorld: server.manifest.linkedWorld ?? null,
        })),
    )
    const pinnedLocalServers = computed(() =>
        localServers.value.filter((server) => server.homePinned),
    )

    /** `servers.setLinkedWorld`: the saved multiplayer entry a local server starts for. */
    async function setLinkedWorld(
        serverId: string,
        link: { instanceId: string; address: string } | null,
    ) {
        const { servers } = await import('../multiplayer/backend.ts')
        await servers.setLinkedWorld(serverId, link)
    }

    /* -- news -------------------------------------------------------------- */

    const news = ref<HomeNewsItem[] | null>(null)
    let newsRequest: Promise<void> | null = null

    /**
     * `get_minecraft_news(12)`: the articles minecraft.net listed when the
     * snapshot was taken, with their images shipped next to the preview.
     */
    function loadNews() {
        if (news.value || newsRequest) return newsRequest ?? Promise.resolve()
        newsRequest = import('./data/news.json')
            .then((module) => {
                news.value = module.default as HomeNewsItem[]
            })
            .catch(() => {
                news.value = []
            })
            .finally(() => {
                newsRequest = null
            })
        return newsRequest
    }

    /** Play insights read the playtime table once, then keep it fresh (the app's query cache). */
    const insightsLoaded = ref(false)
    function loadInsights() {
        if (insightsLoaded.value) return
        insightsLoaded.value = true
    }

    return {
        value: {
            async whenReady() {
                await Promise.all([tagsReady, initialServersReady, newsRequest])
            },
            worlds,
            sessions,
            dashboard,
            dashboardEditing,
            minimalInstanceId,
            instanceRevision,
            gameVersions,
            favoriteWorlds,
            recentWorlds,
            localServers,
            pinnedLocalServers,
            serverData,
            news,
            insightsLoaded,
            pendingJoins,
            getDailyPlaytime,
            getDailyPlaytimeDetails,
            getInstanceWorlds,
            setWorldDisplayStatus,
            getServerData,
            refreshServer,
            localServerAt,
            setLinkedWorld,
            loadNews,
            loadInsights,
            delay,
        },
        dispose: () => scope.stop(),
    }
}

export type HomeState = ReturnType<typeof createHomeState>['value']

/** Home state that survives navigating away and back, owned by the screen's chunk. */
export function useHomeState(): HomeState {
    return usePreviewDomain('home', createHomeState)
}
