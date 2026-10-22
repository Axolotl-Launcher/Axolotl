import type { GameVersion } from '@modrinth/ui'
import type { InjectionKey, Ref } from 'vue'
import { inject, onUnmounted, provide, reactive, ref } from 'vue'

import { instance_listener, process_listener } from '@/helpers/events'
import { get_all } from '@/helpers/process'
import { serverEventListener, type ServerInfoData, servers as serversApi } from '@/helpers/servers'
import { get_game_versions } from '@/helpers/tags'
import {
    get_favorite_worlds,
    get_instance_protocol_version,
    get_instance_worlds,
    get_recent_worlds,
    type ProtocolVersion,
    refreshServerData,
    type ServerData,
    type World,
    type WorldWithInstance,
} from '@/helpers/worlds'

import { createHomeRefresh, homeWorldsChanged } from './home-refresh'

type ErrorHandler = (error: unknown) => void

export type HomeDashboardRuntime = {
    favoriteWorlds: Ref<WorldWithInstance[]>
    recentWorlds: Ref<WorldWithInstance[]>
    runningInstanceIds: Ref<string[]>
    /** Every managed local server, including the ones not pinned to Home. */
    localServers: Ref<ServerInfoData[]>
    pinnedLocalServers: Ref<ServerInfoData[]>
    gameVersions: Ref<GameVersion[]>
    instanceRevision: Ref<number>
    refreshFavorites: () => Promise<void>
    refreshRecentWorlds: () => Promise<void>
    refreshPinnedLocalServers: () => Promise<void>
    getInstanceWorlds: (instanceId: string, force?: boolean) => Promise<World[]>
    getServerData: (instanceId: string, address: string) => ServerData
    getProtocolVersion: (instanceId: string) => ProtocolVersion | null | undefined
    refreshServer: (instanceId: string, address: string, force?: boolean) => Promise<void>
}

const HOME_DASHBOARD_RUNTIME_KEY: InjectionKey<HomeDashboardRuntime> =
    Symbol('home-dashboard-runtime')

function serverKey(instanceId: string, address: string) {
    return `${instanceId}:${address}`
}

export function provideHomeDashboardRuntime(handleError: ErrorHandler): HomeDashboardRuntime {
    const favoriteWorlds = ref<WorldWithInstance[]>([])
    const recentWorlds = ref<WorldWithInstance[]>([])
    const runningInstanceIds = ref<string[]>([])
    const localServers = ref<ServerInfoData[]>([])
    const pinnedLocalServers = ref<ServerInfoData[]>([])
    const gameVersions = ref<GameVersion[]>([])
    const instanceRevision = ref(0)
    const worldsByInstance = reactive<Record<string, World[]>>({})
    const serverData = reactive<Record<string, ServerData>>({})
    const protocolVersions = reactive<Record<string, ProtocolVersion | null>>({})
    const loadedWorlds = new Set<string>()
    const loadedServers = new Set<string>()
    const worldRequests = new Map<string, Promise<World[]>>()
    const serverRequests = new Map<string, Promise<void>>()
    const protocolRequests = new Map<string, Promise<ProtocolVersion | null>>()
    const unlisteners: Array<() => void> = []
    let disposed = false

    const refreshRunningInstances = createHomeRefresh(
        async () => {
            try {
                const processes = await get_all()
                if (!disposed)
                    runningInstanceIds.value = processes.map((process) => process.instance_id)
            } catch (error) {
                handleError(error)
            }
        },
        () => disposed,
    )

    async function ensureProtocolVersion(instanceId: string) {
        if (Object.hasOwn(protocolVersions, instanceId)) return protocolVersions[instanceId]
        const pending = protocolRequests.get(instanceId)
        if (pending) return pending

        const request = get_instance_protocol_version(instanceId)
            .catch(() => null)
            .then((protocolVersion) => {
                protocolVersions[instanceId] = protocolVersion
                return protocolVersion
            })
            .finally(() => protocolRequests.delete(instanceId))
        protocolRequests.set(instanceId, request)
        return request
    }

    function getServerData(instanceId: string, address: string) {
        return (serverData[serverKey(instanceId, address)] ??= { refreshing: true })
    }

    async function refreshServer(instanceId: string, address: string, force = false) {
        const key = serverKey(instanceId, address)
        if (!force && loadedServers.has(key)) return
        const pending = serverRequests.get(key)
        if (pending) return pending

        const request = (async () => {
            const protocolVersion = await ensureProtocolVersion(instanceId)
            await refreshServerData(getServerData(instanceId, address), protocolVersion, address)
            loadedServers.add(key)
        })().finally(() => serverRequests.delete(key))
        serverRequests.set(key, request)
        return request
    }

    function warmServerData(worlds: WorldWithInstance[]) {
        for (const world of worlds) {
            if (world.type === 'server') void refreshServer(world.instance_id, world.address)
        }
    }

    const refreshFavorites = createHomeRefresh(
        async () => {
            try {
                const worlds = await get_favorite_worlds()
                if (disposed) return
                favoriteWorlds.value = worlds
                warmServerData(favoriteWorlds.value)
            } catch (error) {
                handleError(error)
            }
        },
        () => disposed,
    )

    const refreshRecentWorlds = createHomeRefresh(
        async () => {
            try {
                const worlds = await get_recent_worlds(8, ['normal', 'favorite'])
                if (disposed) return
                recentWorlds.value = worlds
                warmServerData(recentWorlds.value)
            } catch (error) {
                handleError(error)
            }
        },
        () => disposed,
    )

    const refreshPinnedLocalServers = createHomeRefresh(
        async () => {
            try {
                const all = await serversApi.list()
                if (disposed) return
                localServers.value = all
                pinnedLocalServers.value = all.filter((server) => Boolean(server.homePinnedAt))
            } catch (error) {
                if (disposed) return
                handleError(error)
            }
        },
        () => disposed,
    )

    async function getInstanceWorlds(instanceId: string, force = false) {
        if (!force && loadedWorlds.has(instanceId)) return worldsByInstance[instanceId] ?? []
        const pending = worldRequests.get(instanceId)
        if (pending) return pending

        const request = get_instance_worlds(instanceId)
            .then((worlds) => {
                worldsByInstance[instanceId] = worlds
                loadedWorlds.add(instanceId)
                return worlds
            })
            .catch((error) => {
                handleError(error)
                return worldsByInstance[instanceId] ?? []
            })
            .finally(() => worldRequests.delete(instanceId))
        worldRequests.set(instanceId, request)
        return request
    }

    void get_game_versions()
        .then((versions) => {
            gameVersions.value = versions
        })
        .catch(() => undefined)
    void refreshRunningInstances()
    void refreshPinnedLocalServers()
    void refreshFavorites()
    void refreshRecentWorlds()

    void process_listener((event: { event: string }) => {
        if (event.event === 'launched' || event.event === 'finished') void refreshRunningInstances()
    })
        .then((unlisten) => {
            if (disposed) unlisten()
            else unlisteners.push(unlisten)
        })
        .catch(handleError)
    void serverEventListener((_serverId, payload) => {
        if (['starting', 'stopping', 'started', 'stopped', 'eula_required'].includes(payload.event))
            void refreshPinnedLocalServers()
    })
        .then((unlisten) => {
            if (disposed) unlisten()
            else unlisteners.push(unlisten)
        })
        .catch(handleError)
    void instance_listener(async (event: { instance_id?: string; event: string }) => {
        if (disposed || !homeWorldsChanged(event.event)) return
        if (event.instance_id) {
            loadedWorlds.delete(event.instance_id)
            Reflect.deleteProperty(worldsByInstance, event.instance_id)
            Reflect.deleteProperty(protocolVersions, event.instance_id)
            for (const key of loadedServers) {
                if (key.startsWith(`${event.instance_id}:`)) loadedServers.delete(key)
            }
        }
        instanceRevision.value += 1
        await Promise.all([refreshFavorites(), refreshRecentWorlds()])
    })
        .then((unlisten) => {
            if (disposed) unlisten()
            else unlisteners.push(unlisten)
        })
        .catch(handleError)

    onUnmounted(() => {
        disposed = true
        for (const unlisten of unlisteners) unlisten()
    })

    const runtime: HomeDashboardRuntime = {
        favoriteWorlds,
        recentWorlds,
        runningInstanceIds,
        localServers,
        pinnedLocalServers,
        gameVersions,
        instanceRevision,
        refreshFavorites,
        refreshRecentWorlds,
        refreshPinnedLocalServers,
        getInstanceWorlds,
        getServerData,
        getProtocolVersion: (instanceId) => protocolVersions[instanceId],
        refreshServer,
    }
    provide(HOME_DASHBOARD_RUNTIME_KEY, runtime)
    return runtime
}

export function useHomeDashboardRuntime() {
    const runtime = inject(HOME_DASHBOARD_RUNTIME_KEY)
    if (!runtime) throw new Error('Home dashboard runtime was not provided')
    return runtime
}
