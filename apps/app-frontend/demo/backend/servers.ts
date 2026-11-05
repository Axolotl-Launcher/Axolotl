import { emit } from '@tauri-apps/api/event'

import type {
    ServerConsoleDelta,
    ServerInfoData,
    ServerLogCursor,
    ServerLogDelta,
    ServerManifestData,
} from '@/helpers/servers'

import fabricRun from '../data/servers/fabric-26.3.json'
import vanillaRun from '../data/servers/vanilla-26.3.json'
import { desktopOnly } from './settings'
import type { Handlers } from './types'
import { isRecordedRemoteServer, pinRemoteServers } from './worlds'

/** Where app-lib keeps managed servers on macOS (packages/app-lib/src/state/dirs.rs). */
const SERVERS_DIR = '/Users/steve/Library/Application Support/red.ghs.axolotl/servers'
const JAVA = '/Library/Java/JavaVirtualMachines/temurin-25.jdk/Contents/Home/bin/java'

type Line = { t: number; line: string }

/** A run of the real server software (data/servers/*.json): lines at their offset in ms. */
interface Recording {
    files: Record<string, string>
    /** The folder the run left behind; directories end in `/`. */
    tree: { path: string; size: number }[]
    start: Line[]
    commands: Record<string, Line[]>
    stop: Line[]
}

const vanilla = vanillaRun as Recording
const fabric = fabricRun as Recording

interface DemoServer {
    manifest: ServerManifestData
    port: number
    recording: Recording
    files: Record<string, string>
    phase: 'stopped' | 'starting' | 'running' | 'stopping'
    epoch: number
    lines: string[]
    console: string
    input: string
    timers: ReturnType<typeof setTimeout>[]
}

/** Two servers set up the way the app's wizard does, each with its EULA accepted. */
function server(
    manifest: Omit<ServerManifestData, 'jvmArgs' | 'lastExitCrashed'>,
    port: number,
    recording: Recording,
): DemoServer {
    return {
        manifest: { ...manifest, jvmArgs: [], stopCommand: 'stop', lastExitCrashed: false },
        port,
        recording,
        files: {
            ...recording.files,
            'eula.txt': recording.files['eula.txt'].replace('eula=false', 'eula=true'),
        },
        phase: 'stopped',
        epoch: 0,
        lines: [],
        console: '',
        input: '',
        timers: [],
    }
}

const servers: DemoServer[] = [
    server(
        {
            id: 'vanilla-26-3-4f1c-8e2a91d3',
            name: 'vanilla-26.3-4f1c',
            serverType: 'vanilla',
            gameVersion: '26.3',
            javaPath: JAVA,
            memoryMb: 2048,
            createdAt: '2026-10-23T16:10:51Z',
            lastStartedAt: '2026-10-23T16:11:38Z',
        },
        25565,
        vanilla,
    ),
    server(
        {
            id: 'fabric-26-3-0-19-5-a07e-3c5b0f19',
            name: 'fabric-26.3-0.19.5-a07e',
            serverType: 'fabric',
            gameVersion: '26.3',
            loaderVersion: '0.19.5',
            jarName: 'fabric-server.jar',
            javaPath: JAVA,
            memoryMb: 2048,
            linkedWorld: { instanceId: 'fabric-26-3', address: 'localhost:25566' },
            createdAt: '2026-10-23T16:12:36Z',
            lastStartedAt: '2026-10-23T16:13:25Z',
        },
        25566,
        fabric,
    ),
]

/** Pins to Home the servers demo-data.json names, and only those. */
export function pinServers(ids: string[]) {
    pinRemoteServers(ids)
    for (const id of ids) {
        if (!isRecordedRemoteServer(id) && !servers.some((entry) => entry.manifest.id === id))
            console.warn(`[demo] demo-data.json: no recorded server ${id}`)
    }
    for (const entry of servers) {
        entry.manifest.homePinnedAt = ids.includes(entry.manifest.id)
            ? (entry.manifest.homePinnedAt ?? new Date().toISOString())
            : undefined
    }
}

function find(serverId: unknown) {
    const entry = servers.find((candidate) => candidate.manifest.id === serverId)
    if (!entry) throw new Error(`Unknown server ${String(serverId)}`)
    return entry
}

function info(entry: DemoServer): ServerInfoData {
    return {
        ...entry.manifest,
        path: `${SERVERS_DIR}/${entry.manifest.name}`,
        running: entry.phase === 'running' || entry.phase === 'stopping',
        starting: entry.phase === 'starting',
        stopping: entry.phase === 'stopping',
        eulaExists: true,
        eulaAccepted: true,
        port: entry.port,
        onlineMode: true,
    }
}

function notify(entry: DemoServer, payload: Record<string, unknown> & { event: string }) {
    void emit('server', { serverId: entry.manifest.id, ...payload })
}

function write(entry: DemoServer, text: string) {
    entry.console += text
    notify(entry, { event: 'console_output', data: '' })
}

/** One line of server output, stamped and placed like the real server's. */
function print(entry: DemoServer, line: string) {
    const time = new Date().toTimeString().slice(0, 8)
    const text = line
        .replaceAll('{time}', time)
        .replaceAll('{serverUrl}', `file:${SERVERS_DIR}/${entry.manifest.name}`)
    entry.lines.push(text)
    notify(entry, { event: 'log', line: text })
    write(entry, `${text}\r\n`)
}

/** Replays recorded output with its original timing, then runs `done`. */
function replay(entry: DemoServer, lines: Line[], done?: () => void) {
    for (const { t, line } of lines) entry.timers.push(setTimeout(() => print(entry, line), t))
    const end = lines[lines.length - 1]?.t ?? 0
    if (done) entry.timers.push(setTimeout(done, end + 1))
}

function start(entry: DemoServer) {
    if (entry.phase !== 'stopped') return
    entry.phase = 'starting'
    entry.epoch += 1
    entry.lines = []
    entry.console = ''
    entry.manifest.lastStartedAt = new Date().toISOString()
    notify(entry, { event: 'starting' })
    const ready = entry.recording.start.findIndex((line) => line.line.includes('Done ('))
    replay(entry, entry.recording.start.slice(0, ready + 1), () => {
        entry.phase = 'running'
        notify(entry, { event: 'started' })
    })
    replay(entry, entry.recording.start.slice(ready + 1))
}

function stop(entry: DemoServer) {
    if (entry.phase !== 'running') return
    entry.phase = 'stopping'
    notify(entry, { event: 'stopping' })
    replay(entry, entry.recording.stop, () => {
        entry.timers.forEach(clearTimeout)
        entry.timers = []
        entry.phase = 'stopped'
        notify(entry, { event: 'stopped', crashed: false })
    })
}

/** A line typed into the console: recorded commands answer as recorded. */
function runCommand(entry: DemoServer, command: string) {
    if (!command) return
    if (command === 'stop') return stop(entry)
    const reply = entry.recording.commands[command] ?? [
        {
            t: 15,
            line: '[{time}] [Server thread/INFO]: Unknown or incomplete command. The demo answers only the commands it recorded: help, list, seed, time query daytime, difficulty, stop',
        },
    ]
    replay(entry, reply)
}

/** Keystrokes from the terminal, echoed like a TTY until Enter sends the line. */
function type(entry: DemoServer, data: string) {
    if (entry.phase !== 'running') return
    for (const character of data) {
        if (character === '\r' || character === '\n') {
            write(entry, '\r\n')
            runCommand(entry, entry.input.trim())
            entry.input = ''
        } else if (character === '\x7f' || character === '\b') {
            if (entry.input) write(entry, '\b \b')
            entry.input = entry.input.slice(0, -1)
        } else if (character >= ' ') {
            entry.input += character
            write(entry, character)
        }
    }
}

function cursorFor(entry: DemoServer, cursor: unknown, length: number) {
    const previous = cursor as ServerLogCursor | undefined
    const reset = !previous || previous.epoch !== entry.epoch || previous.sequence > length
    return { reset, from: reset ? 0 : previous.sequence }
}

function base64(text: string) {
    let binary = ''
    for (const byte of new TextEncoder().encode(text)) binary += String.fromCharCode(byte)
    return btoa(binary)
}

/** The server folder a path lies in, and the path inside it (`''` for the folder itself). */
function locate(path: unknown) {
    const text = String(path)
    for (const entry of servers) {
        const root = `${SERVERS_DIR}/${entry.manifest.name}`
        if (text === root || text.startsWith(`${root}/`))
            return { entry, relative: text.slice(root.length + 1).replace(/\/$/, '') }
    }
    throw new Error(`No such file or directory: ${text}`)
}

function treeEntry(entry: DemoServer, relative: string) {
    return entry.recording.tree.find(
        (item) => item.path === relative || item.path === `${relative}/`,
    )
}

/** A file's text: the recorded files, the current log, or nothing for binaries. */
function fileText(entry: DemoServer, relative: string) {
    if (relative === 'logs/latest.log') return entry.lines.join('\n')
    const text = entry.files[relative]
    if (text === undefined) throw desktopOnly()
    return text
}

/** The demo's folders hold what the recorded run left behind; reading needs no disk. */
const fileHandlers: Handlers = {
    'plugin:fs|read_dir': ({ path }) => {
        const { entry, relative } = locate(path)
        const prefix = relative ? `${relative}/` : ''
        return entry.recording.tree.flatMap(({ path: item }) => {
            const rest = item.startsWith(prefix) ? item.slice(prefix.length) : null
            const name = rest?.replace(/\/$/, '')
            if (!rest || !name || name.includes('/')) return []
            const isDirectory = rest.endsWith('/')
            return [{ name, isDirectory, isFile: !isDirectory, isSymlink: false }]
        })
    },
    'plugin:fs|stat': ({ path }) => {
        const { entry, relative } = locate(path)
        const item = treeEntry(entry, relative)
        if (relative && !item) throw new Error(`No such file or directory: ${String(path)}`)
        const isDirectory = !relative || item!.path.endsWith('/')
        const modified = Date.parse(entry.manifest.lastStartedAt ?? entry.manifest.createdAt)
        return {
            isFile: !isDirectory,
            isDirectory,
            isSymlink: false,
            size:
                relative === 'logs/latest.log'
                    ? fileText(entry, relative).length
                    : (item?.size ?? 0),
            mtime: modified,
            atime: modified,
            birthtime: Date.parse(entry.manifest.createdAt),
            readonly: false,
            fileAttributes: null,
            dev: null,
            ino: null,
            mode: null,
            nlink: null,
            uid: null,
            gid: null,
            rdev: null,
            blksize: null,
            blocks: null,
        }
    },
    'plugin:fs|read_text_file': ({ path }) => {
        const { entry, relative } = locate(path)
        return Array.from(new TextEncoder().encode(fileText(entry, relative)))
    },
}

export const serverHandlers: Handlers = {
    ...fileHandlers,
    'plugin:servers|servers_list': () => servers.map(info),
    'plugin:servers|servers_get': ({ serverId }) => info(find(serverId)),
    'plugin:servers|servers_read_file': ({ serverId, file }) => {
        const contents = find(serverId).files[String(file)]
        if (contents === undefined) throw new Error(`No such file: ${String(file)}`)
        return contents
    },
    'plugin:servers|servers_write_file': ({ serverId, file, contents }) => {
        find(serverId).files[String(file)] = String(contents)
    },
    'plugin:servers|servers_is_local_address': ({ address }) =>
        /^(localhost|127\.|\[?::1\]?)/.test(String(address)),
    'plugin:servers|servers_wait_until_ready': ({ serverId }) => find(serverId).phase === 'running',
    'plugin:servers|servers_start': ({ serverId }) => start(find(serverId)),
    'plugin:servers|servers_stop': ({ serverId }) => stop(find(serverId)),
    'plugin:servers|servers_kill': ({ serverId }) => stop(find(serverId)),
    'plugin:servers|servers_send_console_input': ({ serverId, data }) =>
        type(
            find(serverId),
            new TextDecoder().decode(
                Uint8Array.from(atob(String(data)), (character) => character.charCodeAt(0)),
            ),
        ),
    'plugin:servers|servers_resize_console': () => {},
    'plugin:servers|servers_port_process': () => null,
    'plugin:servers|servers_get_log_buffer': ({ serverId }) => find(serverId).lines,
    'plugin:servers|servers_get_log_delta': ({ serverId, cursor }): ServerLogDelta => {
        const entry = find(serverId)
        const { reset, from } = cursorFor(entry, cursor, entry.lines.length)
        return {
            cursor: { epoch: entry.epoch, sequence: entry.lines.length },
            reset,
            lines: entry.lines.slice(from),
        }
    },
    'plugin:servers|servers_get_console_delta': ({ serverId, cursor }): ServerConsoleDelta => {
        const entry = find(serverId)
        const { reset, from } = cursorFor(entry, cursor, entry.console.length)
        return {
            cursor: { epoch: entry.epoch, sequence: entry.console.length },
            reset,
            data: base64(entry.console.slice(from)),
        }
    },
    'plugin:servers|servers_clear_log': ({ serverId }) => {
        const entry = find(serverId)
        entry.lines = []
        entry.console = ''
        entry.epoch += 1
    },
}
