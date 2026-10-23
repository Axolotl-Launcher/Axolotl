import { getProperty, parseEula, parseProperties, setEulaAccepted } from '../../lib/server-core.ts'
import { currentPreview, previewDomain, type PreviewState } from '../../lib/state'
import {
    type ManagedServer,
    type ManagedServerManifest,
    SERVERS_DIR,
} from '../../lib/state/servers'
import { desktopOnlyError } from './app-helpers.ts'

/**
 * `apps/app-frontend/src/helpers/servers.ts` for the preview: the `servers`
 * plugin commands and the `server` event, answered by the window's managed
 * servers. Starting a server replays a recording of the real server software
 * (`data/servers/*.json`) line by line with its original timing; commands that
 * were recorded get their recorded reply. Server files are the folder the
 * recorded server left behind. Downloading server files is left to the desktop
 * app, so creating a server stops at that step with its notice.
 */

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

export type ModpackInfoData = NonNullable<ManagedServerManifest['modpack']>
export type LinkedWorldData = NonNullable<ManagedServerManifest['linkedWorld']>
export type ServerManifestData = ManagedServerManifest

export interface ServerInfoData extends ServerManifestData {
    path: string
    running: boolean
    starting?: boolean
    stopping?: boolean
    eulaExists: boolean
    eulaAccepted: boolean
    port: number | null
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

interface RecordedLine {
    t: number
    line: string
}

interface Recording {
    source: Record<string, unknown>
    files: Record<string, string>
    tree: { path: string; size: number }[]
    start: RecordedLine[]
    commands: Record<string, RecordedLine[]>
    stop: RecordedLine[]
}

const RECORDINGS: Record<NonNullable<ManagedServer['recording']>, () => Promise<Recording>> = {
    'vanilla-26.3': () =>
        import('../../data/servers/vanilla-26.3.json').then(
            (module) => module.default as Recording,
        ),
    'fabric-26.3': () =>
        import('../../data/servers/fabric-26.3.json').then((module) => module.default as Recording),
}

export interface ServerFile {
    kind: 'file' | 'directory'
    size: number
    modified: number
    content?: string
}

interface Runtime {
    log: { epoch: number; lines: string[] }
    console: { epoch: number; data: string }
    cancels: (() => void)[]
    input: string
    pid: number
    /** The last command the preview has no recording of, for the terminal's notice. */
    unrecorded: string | null
    files: Map<string, ServerFile> | null
}

function createRuntimes(state: PreviewState) {
    const runtimes = new Map<string, Runtime>()
    let nextPid = 48213
    return {
        value: {
            runtime(id: string): Runtime {
                let runtime = runtimes.get(id)
                if (!runtime) {
                    runtime = {
                        log: { epoch: 1, lines: [] },
                        console: { epoch: 1, data: '' },
                        cancels: [],
                        input: '',
                        pid: nextPid++,
                        unrecorded: null,
                        files: null,
                    }
                    runtimes.set(id, runtime)
                }
                return runtime
            },
            forget(id: string) {
                runtimes.get(id)?.cancels.forEach((cancel) => cancel())
                runtimes.delete(id)
            },
        },
        dispose() {
            for (const runtime of runtimes.values()) runtime.cancels.forEach((cancel) => cancel())
            runtimes.clear()
            void state
        },
    }
}

function store() {
    const preview = currentPreview()
    return {
        preview,
        servers: preview.servers,
        runtimes: previewDomain(preview, 'multiplayer:server-runtimes', createRuntimes),
    }
}

function find(id: string): ManagedServer {
    const server = store().servers.get(id)
    if (!server) throw new Error(`Server not found: ${id}`)
    return server
}

/** Copies of reactive records for callers, as the app receives serialized results. */
function copy<T>(value: T): T {
    return JSON.parse(JSON.stringify(value)) as T
}

function serverPath(id: string) {
    return `${SERVERS_DIR}/${id}`
}

function info(server: ManagedServer): ServerInfoData {
    return {
        ...copy(server.manifest),
        path: serverPath(server.manifest.id),
        running: server.phase === 'running' || server.phase === 'stopping',
        starting: server.phase === 'starting',
        stopping: server.phase === 'stopping',
        eulaExists: server.eulaExists,
        eulaAccepted: server.eulaAccepted,
        port: server.port,
        onlineMode: server.onlineMode,
    }
}

function emit(id: string, payload: ServerEventPayload) {
    store().servers.emit(id, payload as { event: string } & Record<string, unknown>)
}

// ---- files -------------------------------------------------------------------

function manifestText(server: ManagedServer) {
    return JSON.stringify(server.manifest, null, 2)
}

const encoder = new TextEncoder()

function textFile(content: string, modified: number): ServerFile {
    return { kind: 'file', size: encoder.encode(content).length, modified, content }
}

/** The server folder: the recorded server's files, or a new server's manifest. */
async function files(id: string): Promise<Map<string, ServerFile>> {
    const server = find(id)
    const runtime = store().runtimes.runtime(id)
    if (runtime.files) return runtime.files
    const tree = new Map<string, ServerFile>()
    const created = Date.parse(server.manifest.createdAt)
    const started = Date.parse(server.manifest.lastStartedAt ?? server.manifest.createdAt)
    if (server.recording) {
        const recording = await RECORDINGS[server.recording]()
        for (const entry of recording.tree) {
            const path = entry.path.replace(/\/$/, '')
            tree.set(path, {
                kind: entry.path.endsWith('/') ? 'directory' : 'file',
                size: entry.size,
                modified: path.startsWith('world') || path.startsWith('logs') ? started : created,
            })
        }
        const eula = setEulaAccepted(recording.files['eula.txt'], server.eulaAccepted)
        tree.set('eula.txt', textFile(eula, started))
        // The second server listens on the next port, as its wizard picked it.
        const properties = recording.files['server.properties'].replace(
            /^server-port=25565$/m,
            `server-port=${server.port}`,
        )
        tree.set('server.properties', textFile(properties, created))
    }
    tree.set('axolotl-server.json', textFile(manifestText(server), created))
    runtime.files = tree
    return tree
}

/** Entries of a folder of a server, for the files panel. */
export async function serverDirectory(id: string, folder: string) {
    const tree = await files(id)
    const prefix = folder ? `${folder.replace(/\/$/, '')}/` : ''
    const children: { name: string; path: string; file: ServerFile; count: number }[] = []
    for (const [path, file] of tree) {
        if (!path.startsWith(prefix)) continue
        const rest = path.slice(prefix.length)
        if (!rest || rest.includes('/')) continue
        const count =
            file.kind === 'directory'
                ? [...tree.keys()].filter(
                      (other) =>
                          other.startsWith(`${path}/`) &&
                          !other.slice(path.length + 1).includes('/'),
                  ).length
                : 0
        children.push({ name: rest, path, file, count })
    }
    return children.sort(
        (a, b) =>
            Number(b.file.kind === 'directory') - Number(a.file.kind === 'directory') ||
            a.name.localeCompare(b.name),
    )
}

/** Adds, replaces or removes files of a server folder from the files panel. */
export async function updateServerFiles(
    id: string,
    change: (tree: Map<string, ServerFile>) => void,
) {
    change(await files(id))
}

// ---- process -------------------------------------------------------------------

function stamp(date = new Date()) {
    return [date.getHours(), date.getMinutes(), date.getSeconds()]
        .map((part) => String(part).padStart(2, '0'))
        .join(':')
}

function fill(server: ManagedServer, line: string) {
    return line
        .replace('{time}', stamp())
        .replaceAll('{serverUrl}', `file:${encodeURI(serverPath(server.manifest.id))}`)
        .replace(
            'Starting Minecraft server on *:25565',
            `Starting Minecraft server on *:${server.port ?? 25565}`,
        )
}

function write(server: ManagedServer, text: string, logLine?: string) {
    const runtime = store().runtimes.runtime(server.manifest.id)
    runtime.console.data += text
    emit(server.manifest.id, { event: 'console_output', data: '' })
    if (logLine !== undefined && logLine !== '' && !logLine.startsWith('> ')) {
        runtime.log.lines.push(logLine)
        emit(server.manifest.id, { event: 'log', line: logLine })
    }
}

function output(server: ManagedServer, line: string) {
    write(server, `${line}\r\n`, line)
}

/** Launcher lines, such as the pre-launch hook's, go to the log but not the process's terminal. */
function logLine(server: ManagedServer, line: string) {
    const runtime = store().runtimes.runtime(server.manifest.id)
    runtime.log.lines.push(line)
    emit(server.manifest.id, { event: 'log', line })
}

/** Plays recorded lines with their original offsets; resolves after the last. */
function replay(server: ManagedServer, lines: RecordedLine[], onLine?: (line: string) => void) {
    const { preview, runtimes } = store()
    const runtime = runtimes.runtime(server.manifest.id)
    return new Promise<void>((resolve) => {
        if (!lines.length) {
            resolve()
            return
        }
        lines.forEach((entry, index) => {
            runtime.cancels.push(
                preview.clock.after(entry.t, () => {
                    const line = fill(server, entry.line)
                    output(server, line)
                    onLine?.(line)
                    if (index === lines.length - 1) resolve()
                }),
            )
        })
    })
}

function setPhase(server: ManagedServer, phase: ManagedServer['phase']) {
    server.phase = phase
}

function exit(server: ManagedServer) {
    const runtime = store().runtimes.runtime(server.manifest.id)
    runtime.cancels.forEach((cancel) => cancel())
    runtime.cancels = []
    runtime.input = ''
    setPhase(server, 'stopped')
    emit(server.manifest.id, { event: 'stopped', crashed: false })
}

async function runStop(server: ManagedServer) {
    if (!server.recording) return exit(server)
    const recording = await RECORDINGS[server.recording]()
    await replay(server, recording.stop)
    const { preview } = store()
    preview.clock.after(400, () => {
        if (server.phase === 'stopping') exit(server)
    })
}

async function runCommand(server: ManagedServer, raw: string) {
    const command = raw.trim()
    const runtime = store().runtimes.runtime(server.manifest.id)
    if (!command) return
    if (server.phase === 'running' && command === (server.manifest.stopCommand ?? 'stop')) {
        setPhase(server, 'stopping')
        emit(server.manifest.id, { event: 'stopping' })
        await runStop(server)
        return
    }
    const recording = server.recording ? await RECORDINGS[server.recording]() : null
    const reply = recording?.commands[command]
    if (!reply || server.phase !== 'running') {
        runtime.unrecorded = command
        emit(server.manifest.id, { event: 'console_output', data: '' })
        return
    }
    runtime.unrecorded = null
    await replay(server, reply)
}

/** Text the terminal shows under the output: the command the preview has no recording of. */
export function unrecordedCommand(id: string): string | null {
    return store().runtimes.runtime(id).unrecorded
}

function toBase64(text: string) {
    let binary = ''
    for (const byte of encoder.encode(text)) binary += String.fromCharCode(byte)
    return btoa(binary)
}

async function start(serverId: string) {
    const server = find(serverId)
    if (server.phase !== 'stopped') throw new Error('The server is already running')
    if (!server.eulaAccepted) {
        throw new Error('You need to agree to the EULA in order to run the server.')
    }
    if (!server.recording) throw desktopOnlyError('server-download')
    const runtime = store().runtimes.runtime(serverId)
    runtime.log = { epoch: runtime.log.epoch + 1, lines: [] }
    runtime.console = { epoch: runtime.console.epoch + 1, data: '' }
    runtime.unrecorded = null
    setPhase(server, 'starting')
    server.manifest.lastStartedAt = new Date().toISOString()
    emit(serverId, { event: 'starting' })
    const hook = server.manifest.preLaunchHook?.trim()
    if (hook) {
        logLine(server, `[hook] Running pre-launch hook: ${hook}`)
        logLine(server, '[hook] Pre-launch hook finished')
    }
    const recording = await RECORDINGS[server.recording]()
    void replay(server, recording.start, (line) => {
        if (line.includes('Done (') && line.includes('For help, type')) {
            setPhase(server, 'running')
            emit(serverId, { event: 'started' })
        }
    })
}

/** The PTY's line discipline: typed text echoes, Enter submits the line. */
function input(serverId: string, text: string) {
    const server = find(serverId)
    if (server.phase === 'stopped') return
    const runtime = store().runtimes.runtime(serverId)
    for (const char of text) {
        if (char === '\r' || char === '\n') {
            const line = runtime.input
            runtime.input = ''
            write(server, '\r\n')
            void runCommand(server, line)
        } else if (char === '\x7f' || char === '\b') {
            if (!runtime.input) continue
            runtime.input = runtime.input.slice(0, -1)
            write(server, '\b \b')
        } else if (char === '\x03') {
            runtime.input = ''
            write(server, '^C\r\n')
            // Ctrl+C stops the server through the JVM's shutdown hook.
            if (server.phase === 'running') {
                setPhase(server, 'stopping')
                emit(serverId, { event: 'stopping' })
                void runStop(server)
            }
        } else if (char >= ' ') {
            runtime.input += char
            write(server, char)
        }
    }
}

function delta<T extends string | string[]>(
    buffer: { epoch: number; content: T },
    cursor: ServerLogCursor | undefined,
) {
    const size = buffer.content.length
    const reset = !cursor || cursor.epoch !== buffer.epoch || cursor.sequence > size
    const from = reset ? 0 : cursor!.sequence
    return {
        cursor: { epoch: buffer.epoch, sequence: size },
        reset,
        part: buffer.content.slice(from) as T,
    }
}

// ---- the `servers` commands ----------------------------------------------------

function sanitizeFolderName(name: string) {
    const sanitized = [...name].map((c) => (/[\p{L}\p{N}_-]/u.test(c) ? c : '-')).join('')
    const trimmed = sanitized.replace(/^-+|-+$/g, '')
    return trimmed ? [...trimmed].slice(0, 32).join('') : 'server'
}

export const servers = {
    list: async (): Promise<ServerInfoData[]> => store().servers.managed.map(info),
    get: async (serverId: string) => info(find(serverId)),
    create: async (options: {
        name: string
        serverType: ManagedServerManifest['serverType']
        gameVersion: string
        loaderVersion?: string
        javaPath?: string
        memoryMb?: number
    }): Promise<ServerManifestData> => {
        const name = options.name.trim()
        if (!name) throw new Error('Server name cannot be empty')
        const id = `${sanitizeFolderName(name)}-${crypto.randomUUID().slice(0, 8)}`
        const manifest: ManagedServerManifest = {
            id,
            name,
            serverType: options.serverType,
            gameVersion: options.gameVersion,
            loaderVersion: options.loaderVersion,
            jarName:
                options.serverType === 'fabric'
                    ? 'fabric-server.jar'
                    : options.serverType === 'quilt'
                      ? 'quilt-server.jar'
                      : undefined,
            javaPath: options.javaPath,
            memoryMb: options.memoryMb,
            jvmArgs: [],
            stopCommand: 'stop',
            createdAt: new Date().toISOString(),
            lastExitCrashed: false,
        }
        store().servers.managed.push({
            manifest,
            port: null,
            onlineMode: null,
            eulaExists: false,
            eulaAccepted: false,
            phase: 'stopped',
            recording: null,
            protocol: null,
        })
        return copy(manifest)
    },
    updateSettings: async (
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
    ): Promise<ServerManifestData> => {
        const server = find(serverId)
        const manifest = server.manifest
        if (options.name !== undefined) manifest.name = options.name.trim() || manifest.name
        if (options.javaPath !== undefined) manifest.javaPath = options.javaPath || undefined
        if (options.memoryMb !== undefined) manifest.memoryMb = options.memoryMb
        if (options.jvmArgs !== undefined) manifest.jvmArgs = options.jvmArgs
        if (options.preLaunchHook !== undefined)
            manifest.preLaunchHook = options.preLaunchHook.trim() || undefined
        if (options.stopCommand !== undefined) manifest.stopCommand = options.stopCommand || 'stop'
        if (options.homePinned !== undefined)
            manifest.homePinnedAt = options.homePinned
                ? (manifest.homePinnedAt ?? new Date().toISOString())
                : undefined
        await updateServerFiles(serverId, (tree) =>
            tree.set('axolotl-server.json', textFile(manifestText(server), Date.now())),
        )
        return copy(manifest)
    },
    setIcon: async (serverId: string, iconPath: string | null) => {
        const server = find(serverId)
        server.manifest.iconPath = iconPath ?? undefined
        return copy(server.manifest)
    },
    setLinkedWorld: async (serverId: string, linked: LinkedWorldData | null) => {
        const server = find(serverId)
        const normalized =
            linked && linked.instanceId.trim() && linked.address.trim()
                ? { instanceId: linked.instanceId.trim(), address: linked.address.trim() }
                : null
        if (normalized) {
            for (const other of store().servers.managed) {
                const link = other.manifest.linkedWorld
                if (
                    other !== server &&
                    link?.instanceId === normalized.instanceId &&
                    link.address === normalized.address
                )
                    other.manifest.linkedWorld = null
            }
        }
        server.manifest.linkedWorld = normalized
        return copy(server.manifest)
    },
    isLocalAddress: async (address: string) => {
        const host = address
            .trim()
            .replace(/:\d+$/, '')
            .replace(/^\[|\]$/g, '')
            .toLowerCase()
        return host === 'localhost' || host === '::1' || host.startsWith('127.')
    },
    waitUntilReady: async (serverId: string, timeoutMs = 120_000) => {
        const deadline = Date.now() + timeoutMs
        for (;;) {
            const server = find(serverId)
            if (server.phase === 'running') return true
            if (server.phase !== 'starting' || Date.now() >= deadline) return false
            await new Promise<void>((resolve) => store().preview.clock.after(250, resolve))
        }
    },
    delete: async (serverId: string) => {
        const server = find(serverId)
        if (server.phase !== 'stopped') throw new Error('Stop the server before deleting it')
        const { servers: state, runtimes } = store()
        state.managed.splice(state.managed.indexOf(server), 1)
        runtimes.forget(serverId)
    },
    readFile: async (serverId: string, file: string) => {
        const entry = (await files(serverId)).get(file)
        if (!entry || entry.kind !== 'file' || entry.content === undefined) {
            throw new Error(`No such file: ${file}`)
        }
        return entry.content
    },
    writeFile: async (serverId: string, file: string, contents: string) => {
        const server = find(serverId)
        await updateServerFiles(serverId, (tree) => tree.set(file, textFile(contents, Date.now())))
        if (file === 'eula.txt') {
            server.eulaExists = true
            server.eulaAccepted = parseEula(contents).accepted
        } else if (file === 'server.properties') {
            const entries = parseProperties(contents)
            const port = Number(getProperty(entries, 'server-port'))
            server.port = Number.isInteger(port) && port > 0 ? port : 25565
            const online = getProperty(entries, 'online-mode')
            server.onlineMode = online === undefined ? null : online === 'true'
        }
    },
    downloadFile: async (_serverId: string, _url: string, _filename: string, _sha1?: string) => {
        throw desktopOnlyError('server-download')
    },
    installModpack: async (_serverId: string, _options: InstallModpackOptions) => {
        throw desktopOnlyError('server-download')
    },
    installForge: async (
        _serverId: string,
        _mcVersion: string,
        _build: string,
        _javaPath?: string,
    ) => {
        throw desktopOnlyError('server-download')
    },
    installNeoForge: async (
        _serverId: string,
        _mcVersion: string,
        _build: string,
        _javaPath?: string,
    ) => {
        throw desktopOnlyError('server-download')
    },
    start: async (
        serverId: string,
        _options?: { javaPath?: string; memoryMb?: number; jvmArgs?: string[] },
    ) => start(serverId),
    sendConsoleInput: async (serverId: string, data: Uint8Array) => {
        input(serverId, new TextDecoder().decode(data))
    },
    resizeConsole: async (_serverId: string, _cols: number, _rows: number) => {},
    stop: async (serverId: string) => {
        const server = find(serverId)
        if (server.phase === 'starting') {
            // The app types the stop command; a starting server reads it once ready.
            setPhase(server, 'stopping')
            emit(serverId, { event: 'stopping' })
            exit(server)
            return
        }
        if (server.phase !== 'running') return
        // The app types the stop command into the server's terminal.
        const command = server.manifest.stopCommand ?? 'stop'
        input(serverId, command === '^C' ? '\x03' : `${command}\r`)
    },
    kill: async (serverId: string) => {
        const server = find(serverId)
        if (server.phase === 'stopped') return
        exit(server)
    },
    killPortProcess: async (port: number) => {
        const owner = store().servers.managed.find(
            (server) => server.port === port && server.phase !== 'stopped',
        )
        if (owner) exit(owner)
    },
    portProcess: async (port: number): Promise<PortProcessInfoData | null> => {
        const owner = store().servers.managed.find(
            (server) => server.port === port && server.phase !== 'stopped',
        )
        return owner ? { pid: store().runtimes.runtime(owner.manifest.id).pid, name: 'java' } : null
    },
    getLogBuffer: async (serverId: string) => [...store().runtimes.runtime(serverId).log.lines],
    getLogDelta: async (serverId: string, cursor?: ServerLogCursor): Promise<ServerLogDelta> => {
        const { log } = store().runtimes.runtime(serverId)
        const result = delta({ epoch: log.epoch, content: log.lines }, cursor)
        return { cursor: result.cursor, reset: result.reset, lines: [...result.part] }
    },
    getConsoleDelta: async (
        serverId: string,
        cursor?: ServerLogCursor,
    ): Promise<ServerConsoleDelta> => {
        const { console } = store().runtimes.runtime(serverId)
        const result = delta({ epoch: console.epoch, content: console.data }, cursor)
        return { cursor: result.cursor, reset: result.reset, data: toBase64(result.part) }
    },
    clearLog: async (serverId: string) => {
        const runtime = store().runtimes.runtime(serverId)
        runtime.log = { epoch: runtime.log.epoch + 1, lines: [] }
    },
}

export function base64ToBytes(data: string): Uint8Array {
    return Uint8Array.from(atob(data), (character) => character.charCodeAt(0))
}

export async function serverEventListener(
    callback: (serverId: string, payload: ServerEventPayload) => void,
): Promise<() => void> {
    const { listeners } = store().servers
    const listener = (serverId: string, payload: { event: string } & Record<string, unknown>) =>
        callback(serverId, payload as unknown as ServerEventPayload)
    listeners.add(listener)
    return () => listeners.delete(listener)
}
