import { reactive } from 'vue'

import serverSnapshot from '../../data/servers.json'

/**
 * Multiplayer state shared by Home and the Multiplayer screen: the Server List
 * Ping snapshot of real public servers (`data/servers.json`) and the dedicated
 * servers the launcher manages.
 *
 * A managed server is the folder the app keeps under `servers/`, with its
 * `axolotl-server.json` manifest. The two servers of this window were set up
 * as the app's wizard does (default names, ports and settings) and their
 * console output is a recording of the real server software
 * (`data/servers/*.json`); `screens/multiplayer/backend.ts` replays it.
 */

/** Ping snapshot of a server address, when the address was part of the snapshot. */
export function serverStatusSnapshot(address: string) {
    return serverSnapshot.servers[address as keyof typeof serverSnapshot.servers] ?? null
}

export const SERVERS_DIR = '/Users/steve/Library/Application Support/red.ghs.axolotl/servers'

/** `ServerManifestData` (`apps/app-frontend/src/helpers/servers.ts`). */
export interface ManagedServerManifest {
    id: string
    name: string
    serverType: 'vanilla' | 'fabric' | 'quilt' | 'paper' | 'forge' | 'neoforge'
    gameVersion: string
    loaderVersion?: string
    jarName?: string
    iconPath?: string
    modpack?: { projectId: string; versionId: string; title: string; iconUrl?: string }
    installState?: 'incomplete' | 'failed' | null
    installError?: string | null
    javaPath?: string
    memoryMb?: number
    jvmArgs: string[]
    preLaunchHook?: string
    stopCommand?: string
    homePinnedAt?: string
    linkedWorld?: { instanceId: string; address: string } | null
    createdAt: string
    lastStartedAt?: string
    lastExitCrashed: boolean
}

export type ManagedServerPhase = 'stopped' | 'starting' | 'running' | 'stopping'

export interface ManagedServer {
    manifest: ManagedServerManifest
    /** `server-port` and `online-mode` of server.properties. */
    port: number | null
    onlineMode: boolean | null
    eulaExists: boolean
    eulaAccepted: boolean
    phase: ManagedServerPhase
    /** Recording of the server software this folder runs. */
    recording: 'vanilla-26.3' | 'fabric-26.3' | null
    /** `protocol_version` of the server jar's version.json, which status pings report. */
    protocol: number | null
}

const TEMURIN_25 = '/Library/Java/JavaVirtualMachines/temurin-25.jdk/Contents/Home/bin/java'

function managed(
    manifest: Omit<ManagedServerManifest, 'jvmArgs' | 'lastExitCrashed' | 'stopCommand'>,
    port: number,
    recording: ManagedServer['recording'],
): ManagedServer {
    return {
        manifest: { ...manifest, jvmArgs: [], stopCommand: 'stop', lastExitCrashed: false },
        port,
        onlineMode: true,
        eulaExists: true,
        eulaAccepted: true,
        phase: 'stopped',
        recording,
        protocol: 777,
    }
}

export function createServersState() {
    const managedServers = reactive<ManagedServer[]>([
        managed(
            {
                id: 'vanilla-26-3-4f1c-8e2a91d3',
                name: 'vanilla-26.3-4f1c',
                serverType: 'vanilla',
                gameVersion: '26.3',
                javaPath: TEMURIN_25,
                memoryMb: 2048,
                homePinnedAt: '2026-10-23T16:15:02Z',
                createdAt: '2026-10-23T16:10:51Z',
                lastStartedAt: '2026-10-23T16:11:38Z',
            },
            25565,
            'vanilla-26.3',
        ),
        managed(
            {
                id: 'fabric-26-3-0-19-5-a07e-3c5b0f19',
                name: 'fabric-26.3-0.19.5-a07e',
                serverType: 'fabric',
                gameVersion: '26.3',
                loaderVersion: '0.19.5',
                jarName: 'fabric-server.jar',
                javaPath: TEMURIN_25,
                memoryMb: 2048,
                linkedWorld: { instanceId: 'fabric-26-3', address: 'localhost:25566' },
                createdAt: '2026-10-23T16:12:36Z',
                lastStartedAt: '2026-10-23T16:13:25Z',
            },
            25566,
            'fabric-26.3',
        ),
    ])

    function get(id: string) {
        return managedServers.find((entry) => entry.manifest.id === id)
    }

    /** Backend events (`server` in the app), delivered to `serverEventListener` callbacks. */
    const listeners = new Set<
        (serverId: string, payload: { event: string } & Record<string, unknown>) => void
    >()

    function emit(serverId: string, payload: { event: string } & Record<string, unknown>) {
        for (const listener of [...listeners]) listener(serverId, payload)
    }

    return { managed: managedServers, get, listeners, emit }
}

export type ServersState = ReturnType<typeof createServersState>
