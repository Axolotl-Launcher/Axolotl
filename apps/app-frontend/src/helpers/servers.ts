import type { ServerTypeId } from '@modrinth/server'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export interface ServerLogCursor {
    epoch: number
    sequence: number
}

export interface ServerLogDelta {
    cursor: ServerLogCursor
    reset: boolean
    lines: string[]
}

export interface ServerConsoleDelta {
    cursor: ServerLogCursor
    reset: boolean
    data: string
}

export interface ModpackInfoData {
    projectId: string
    versionId: string
    title: string
    iconUrl?: string
}

/** A saved multiplayer entry that a managed server is the backend of. */
export interface LinkedWorldData {
    instanceId: string
    address: string
}

export interface ServerManifestData {
    id: string
    name: string
    serverType: ServerTypeId
    gameVersion: string
    loaderVersion?: string
    jarName?: string
    iconPath?: string
    modpack?: ModpackInfoData
    installState?: 'incomplete' | 'failed' | null
    installError?: string | null
    javaPath?: string
    memoryMb?: number
    jvmArgs: string[]
    preLaunchHook?: string
    stopCommand?: string
    homePinnedAt?: string
    linkedWorld?: LinkedWorldData | null
    createdAt: string
    lastStartedAt?: string
    lastExitCrashed: boolean
}

export interface ServerInfoData extends ServerManifestData {
    path: string
    running: boolean
    starting?: boolean
    stopping?: boolean
    eulaExists: boolean
    eulaAccepted: boolean
    port: number | null
    /** `online-mode` from server.properties; null when unset (defaults to on). */
    onlineMode: boolean | null
}

export type ServerExitReason = 'eula'

export type ServerEventPayload =
    | { event: 'log'; line: string }
    | { event: 'console_output'; data: string }
    | { event: 'download_progress'; downloaded: number; total?: number }
    | { event: 'starting' | 'stopping' | 'started' }
    | { event: 'stopped'; crashed: boolean; reason?: ServerExitReason }
    | { event: 'eula_required'; server_id: string; eula_text: string }

export interface PortProcessInfoData {
    pid: number
    name?: string | null
}

export interface InstallModpackOptions {
    mrpackUrl: string
    mrpackSha1?: string
    jarUrl?: string
    jarFilename?: string
    jarSha1?: string
    modpackProjectId?: string
    modpackVersionId?: string
    modpackTitle?: string
    modpackIconUrl?: string
}

const command = (name: string) => `plugin:servers|${name}`

export const servers = {
    list: () => invoke<ServerInfoData[]>(command('servers_list')),
    get: (serverId: string) => invoke<ServerInfoData>(command('servers_get'), { serverId }),
    create: (options: {
        name: string
        serverType: ServerTypeId
        gameVersion: string
        loaderVersion?: string
        javaPath?: string
        memoryMb?: number
    }) => invoke<ServerManifestData>(command('servers_create'), options),
    updateSettings: (
        serverId: string,
        options: {
            name?: string
            javaPath?: string
            memoryMb?: number
            jvmArgs?: string[]
            preLaunchHook?: string
            stopCommand?: string
            homePinned?: boolean
        },
    ) => invoke<ServerManifestData>(command('servers_update_settings'), { serverId, ...options }),
    setIcon: (serverId: string, iconPath: string | null) =>
        invoke<ServerManifestData>(command('servers_set_icon'), { serverId, iconPath }),
    setLinkedWorld: (serverId: string, linked: LinkedWorldData | null) =>
        invoke<ServerManifestData>(command('servers_set_linked_world'), { serverId, linked }),
    isLocalAddress: (address: string) =>
        invoke<boolean>(command('servers_is_local_address'), { address }),
    waitUntilReady: (serverId: string, timeoutMs?: number) =>
        invoke<boolean>(command('servers_wait_until_ready'), { serverId, timeoutMs }),
    delete: (serverId: string) => invoke<void>(command('servers_delete'), { serverId }),
    readFile: (serverId: string, file: string) =>
        invoke<string>(command('servers_read_file'), { serverId, file }),
    writeFile: (serverId: string, file: string, contents: string) =>
        invoke<void>(command('servers_write_file'), { serverId, file, contents }),
    downloadFile: (serverId: string, url: string, filename: string, expectedSha1?: string) =>
        invoke<void>(command('servers_download_file'), {
            serverId,
            url,
            filename,
            expectedSha1,
        }),
    installModpack: (serverId: string, options: InstallModpackOptions) =>
        invoke<void>(command('servers_install_modpack'), { serverId, ...options }),
    installForge: (serverId: string, mcVersion: string, build: string, javaPath?: string) =>
        invoke<void>(command('servers_install_forge'), { serverId, mcVersion, build, javaPath }),
    installNeoForge: (serverId: string, mcVersion: string, build: string, javaPath?: string) =>
        invoke<void>(command('servers_install_neoforge'), { serverId, mcVersion, build, javaPath }),
    start: (
        serverId: string,
        options?: { javaPath?: string; memoryMb?: number; jvmArgs?: string[] },
    ) => invoke<void>(command('servers_start'), { serverId, ...options }),
    sendConsoleInput: (serverId: string, data: Uint8Array) =>
        invoke<void>(command('servers_send_console_input'), {
            serverId,
            data: bytesToBase64(data),
        }),
    resizeConsole: (serverId: string, cols: number, rows: number) =>
        invoke<void>(command('servers_resize_console'), { serverId, cols, rows }),
    stop: (serverId: string) => invoke<void>(command('servers_stop'), { serverId }),
    kill: (serverId: string) => invoke<void>(command('servers_kill'), { serverId }),
    killPortProcess: (port: number) => invoke<void>(command('servers_kill_port_process'), { port }),
    portProcess: (port: number) =>
        invoke<PortProcessInfoData | null>(command('servers_port_process'), { port }),
    getLogBuffer: (serverId: string) =>
        invoke<string[]>(command('servers_get_log_buffer'), { serverId }),
    getLogDelta: (serverId: string, cursor?: ServerLogCursor) =>
        invoke<ServerLogDelta>(command('servers_get_log_delta'), { serverId, cursor }),
    getConsoleDelta: (serverId: string, cursor?: ServerLogCursor) =>
        invoke<ServerConsoleDelta>(command('servers_get_console_delta'), { serverId, cursor }),
    clearLog: (serverId: string) => invoke<void>(command('servers_clear_log'), { serverId }),
}

export function base64ToBytes(data: string): Uint8Array {
    return Uint8Array.from(atob(data), (character) => character.charCodeAt(0))
}

function bytesToBase64(data: Uint8Array): string {
    let binary = ''
    for (const byte of data) binary += String.fromCharCode(byte)
    return btoa(binary)
}

export async function serverEventListener(
    callback: (serverId: string, payload: ServerEventPayload) => void,
): Promise<() => void> {
    const unlisten = await listen<{ serverId: string; event: string } & ServerEventPayload>(
        'server',
        (event) => callback(event.payload.serverId, event.payload),
    )
    return unlisten
}
