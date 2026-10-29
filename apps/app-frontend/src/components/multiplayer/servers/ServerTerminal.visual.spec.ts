import '@modrinth/ui/directives/tooltip-styles.ts'

import { I18N_INJECTION_KEY } from '@modrinth/ui'
import { tooltipDirective } from '@modrinth/ui/directives/tooltip.ts'
import { flushPromises, type VueWrapper } from '@vue/test-utils'
import type { Terminal } from '@xterm/xterm'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { ref } from 'vue'

import type { ServerView } from '@/composables/useServers'
import type { ServerConsoleDelta } from '@/helpers/servers'
import { applyTheme, mountThemed, waitFor } from '@/test/visual-harness'

import ServerTerminal from './ServerTerminal.vue'

const backend = vi.hoisted(() => ({
    save: vi.fn<() => Promise<string | null>>(),
    write: vi.fn(async (_path: string, _text: string) => {}),
    delta: vi.fn<(id: string, cursor?: unknown) => Promise<ServerConsoleDelta>>(),
    input: vi.fn(async (_id: string, _bytes: Uint8Array) => {}),
    resize: vi.fn(async (_id: string, _cols: number, _rows: number) => {}),
    error: vi.fn(),
    unsubscribe: vi.fn(),
    notify: undefined as (() => void) | undefined,
    terminal: undefined as Terminal | undefined,
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: backend.save }))
vi.mock('@tauri-apps/plugin-fs', () => ({ writeTextFile: backend.write }))
vi.mock('@/helpers/servers', () => ({
    servers: {
        getConsoleDelta: backend.delta,
        sendConsoleInput: backend.input,
        resizeConsole: backend.resize,
    },
    base64ToBytes: (data: string) => Uint8Array.from(atob(data), (char) => char.charCodeAt(0)),
}))
vi.mock('@/composables/useServers', () => ({
    subscribeServerConsoleOutput: (_id: string, notify: () => void) => {
        backend.notify = notify
        return backend.unsubscribe
    },
}))
vi.mock('@xterm/xterm', async (importOriginal) => {
    const actual = await importOriginal<typeof import('@xterm/xterm')>()
    return {
        ...actual,
        Terminal: class extends actual.Terminal {
            constructor(options: ConstructorParameters<typeof actual.Terminal>[0]) {
                super(options)
                backend.terminal = this
            }
        },
    }
})
let wrapper: VueWrapper | undefined
const server = { id: 'server', running: true, serverType: 'vanilla' } as ServerView
const encoder = new TextEncoder()
const decoder = new TextDecoder()
const delta = (text = '', sequence = 0, reset = false, epoch = 1): ServerConsoleDelta => ({
    data: btoa(String.fromCharCode(...encoder.encode(text))),
    cursor: { epoch, sequence },
    reset,
})
function screen() {
    const buffer = backend.terminal!.buffer.active
    return Array.from({ length: buffer.length }, (_, index) =>
        buffer.getLine(index)?.translateToString(true),
    ).join('\n')
}
function inputText() {
    return backend.input.mock.calls.map(([, bytes]) => decoder.decode(bytes)).join('')
}
async function mountTerminal() {
    wrapper = await mountThemed(ServerTerminal, { server, view: 'terminal' as const }, 'dark', {
        global: {
            directives: { tooltip: tooltipDirective },
            provide: {
                [I18N_INJECTION_KEY as symbol]: {
                    locale: ref('en-US'),
                    t: (key: string) => key,
                    setLocale: () => {},
                },
                [Symbol.for('modrinth:notificationManager')]: { handleError: backend.error },
            },
        },
    })
    wrapper
        .get('div')
        .element.setAttribute(
            'style',
            'display:flex;width:min(800px,calc(100vw - 48px));height:420px;flex-direction:column',
        )
    await vi.waitFor(() => expect(backend.delta).toHaveBeenCalled())
    await flushPromises()
    return wrapper
}
async function output(text: string, sequence = 1, reset = false, epoch = 1) {
    backend.delta.mockResolvedValue(delta(text, sequence, reset, epoch))
    const calls = backend.delta.mock.calls.length
    backend.notify?.()
    await vi.waitFor(() => expect(backend.delta.mock.calls.length).toBeGreaterThan(calls))
    backend.delta.mockResolvedValue(delta('', sequence, false, epoch))
    await flushPromises()
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
}

beforeEach(() => {
    vi.clearAllMocks()
    backend.delta.mockReset().mockResolvedValue(delta())
    backend.save.mockReset().mockResolvedValue(null)
    backend.write.mockReset().mockResolvedValue(undefined)
})

it('exports terminal scrollback through the native save dialog and preserves wrapped spaces', async () => {
    await mountTerminal()
    await output('first line\r\nsecond line\r\n> ')
    backend.save.mockResolvedValue('E:/chosen/console.txt')
    await wrapper!.get('button[aria-label="Download"]').trigger('click')
    await vi.waitFor(() => expect(backend.write).toHaveBeenCalledOnce())
    expect(backend.save).toHaveBeenCalledWith(
        expect.objectContaining({ defaultPath: 'server-console.txt' }),
    )
    expect(backend.write.mock.calls[0][0]).toBe('E:/chosen/console.txt')
    expect(backend.write.mock.calls[0][1]).toContain('first line\nsecond line\n>')
})

it('does not write after cancelling a save dialog and reports export failures', async () => {
    await mountTerminal()
    await wrapper!.get('button[aria-label="Download"]').trigger('click')
    await flushPromises()
    expect(backend.write).not.toHaveBeenCalled()
    backend.save.mockResolvedValue('E:/chosen/console.txt')
    backend.write.mockRejectedValueOnce(new Error('write failed'))
    await wrapper!.get('button[aria-label="Download"]').trigger('click')
    await vi.waitFor(() => expect(backend.error).toHaveBeenCalledOnce())
})

it('does not write when the terminal unmounts while a save dialog is open', async () => {
    await mountTerminal()
    let finish!: (path: string) => void
    backend.save.mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                finish = resolve
            }),
    )
    await wrapper!.get('button[aria-label="Download"]').trigger('click')
    wrapper!.unmount()
    wrapper = undefined
    finish('E:/chosen/console.txt')
    await flushPromises()
    expect(backend.write).not.toHaveBeenCalled()
})

it('fits the terminal inside its padded host at short heights and after resizing', async () => {
    await mountTerminal()
    const root = wrapper!.get<HTMLElement>('div').element
    const terminal = wrapper!.get<HTMLElement>('.xterm').element
    const host = terminal.parentElement!.parentElement!
    for (const height of [300, 420, 240]) {
        root.style.height = `${height}px`
        await waitFor(() => {
            const bounds = host.getBoundingClientRect()
            const padding = getComputedStyle(host)
            const screen = terminal
                .querySelector<HTMLElement>('.xterm-screen')!
                .getBoundingClientRect()
            return (
                screen.bottom <= bounds.bottom - Number.parseFloat(padding.paddingBottom) + 1 &&
                screen.right <= bounds.right - Number.parseFloat(padding.paddingRight) + 1
            )
        })
        expect(getComputedStyle(host).paddingTop).toBe('12px')
        expect(terminal.getBoundingClientRect().top).toBeGreaterThanOrEqual(
            host.getBoundingClientRect().top + 12,
        )
    }
})
afterEach(() => {
    wrapper?.unmount()
    wrapper = undefined
    document.body.replaceChildren()
})

it('renders raw ANSI redraws, colors, Unicode and the alternate screen with real xterm', async () => {
    await mountTerminal()
    await output('\x1b[31mred\x1b[0m 中文\r\nprogress 10%\r\x1b[2Kprogress 90%')
    expect(screen()).toContain('red 中文')
    expect(screen()).toContain('progress 90%')
    expect(screen()).not.toContain('progress 10%')
    expect(backend.terminal!.buffer.active.getLine(0)!.getCell(0)!.getFgColor()).toBe(1)
    await output('\x1b[?1049h\x1b[Halternate', 2)
    expect(backend.terminal!.buffer.active.type).toBe('alternate')
    expect(screen()).toContain('alternate')
    await output('\x1b[?1049l', 3)
    expect(backend.terminal!.buffer.active.type).toBe('normal')
    expect(screen()).toContain('progress 90%')
})

it('forwards Enter, Tab, application cursor keys, Backspace and Ctrl+C without a custom command field', async () => {
    await mountTerminal()
    await output('\x1b[?1h> ')
    const field = wrapper!.get<HTMLTextAreaElement>('.xterm-helper-textarea')
    field.element.focus()
    for (const [key, keyCode, ctrlKey] of [
        ['Enter', 13, false],
        ['Tab', 9, false],
        ['ArrowUp', 38, false],
        ['Backspace', 8, false],
        ['c', 67, true],
    ] as const) {
        await field.trigger('keydown', { key, keyCode, ctrlKey })
        await field.trigger('keyup', { key, keyCode, ctrlKey })
    }
    await vi.waitFor(() => expect(inputText()).toBe('\r\t\x1bOA\x7f\x03'))
})

it('preserves bracketed multi-line Unicode paste and splits large input within the IPC limit', async () => {
    await mountTerminal()
    await output('\x1b[?2004h')
    const text = '中'.repeat(30000) + '\nsecond line'
    backend.terminal!.paste(text)
    await vi.waitFor(() => expect(backend.input).toHaveBeenCalledTimes(2))
    const bytes = new Uint8Array(
        backend.input.mock.calls.reduce((total, [, chunk]) => total + chunk.length, 0),
    )
    let offset = 0
    for (const [, chunk] of backend.input.mock.calls) {
        expect(chunk.length).toBeLessThanOrEqual(65536)
        bytes.set(chunk, offset)
        offset += chunk.length
    }
    expect(decoder.decode(bytes)).toBe(`\x1b[200~${text.replaceAll('\n', '\r')}\x1b[201~`)
})

it('responds to terminal device queries so interactive programs do not stall', async () => {
    await mountTerminal()
    await output('\x1b[6n')
    await vi.waitFor(() => expect(inputText()).toBe('\x1b[1;1R'))
})

it('keeps the last cursor after failure and retries missing output without clearing the display', async () => {
    await mountTerminal()
    await output('old', 10)
    backend.delta.mockRejectedValueOnce(new Error('temporarily unavailable'))
    backend.notify?.()
    await vi.waitFor(() => expect(backend.error).toHaveBeenCalledOnce())
    expect(screen()).toContain('old')
    await output('new', 11)
    expect(screen()).toContain('oldnew')
    expect(backend.delta).toHaveBeenLastCalledWith('server', { epoch: 1, sequence: 10 })
})

it('resizes the actual PTY, preserves the terminal when changing views and disables stopped input', async () => {
    await mountTerminal()
    await output('keep me', 1)
    await vi.waitFor(() => expect(backend.resize).toHaveBeenCalled())
    const instance = backend.terminal
    await wrapper!.setProps({ view: 'logs' })
    await wrapper!.setProps({ view: 'terminal' })
    expect(backend.terminal).toBe(instance)
    expect(screen()).toContain('keep me')
    await wrapper!.setProps({ server: { ...server, running: false } })
    backend.terminal!.input('blocked', true)
    await flushPromises()
    expect(backend.input).not.toHaveBeenCalled()
})

it('discards a previous server response and fetches the new session without its cursor', async () => {
    await mountTerminal()
    let finish!: (value: ServerConsoleDelta) => void
    backend.delta.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    const calls = backend.delta.mock.calls.length
    backend.notify?.()
    await vi.waitFor(() => expect(backend.delta.mock.calls.length).toBeGreaterThan(calls))
    await wrapper!.setProps({ server: { ...server, id: 'other' } })
    backend.delta.mockResolvedValue(delta('other output', 1, true, 2))
    finish(delta('stale output', 99))
    await vi.waitFor(() => expect(screen()).toContain('other output'))
    expect(screen()).not.toContain('stale output')
    expect(backend.delta).toHaveBeenLastCalledWith('other', undefined)
})

it('starts a fresh polling session for preparation even when no output event is delivered', async () => {
    await mountTerminal()
    await wrapper!.setProps({ server: { ...server, running: false } })
    await output('previous run', 9)
    backend.delta.mockResolvedValue(delta('Preparing Fabric server files', 1, true, 2))
    const calls = backend.delta.mock.calls.length
    await wrapper!.setProps({ server: { ...server, running: false, starting: true } })
    await vi.waitFor(() => expect(backend.delta.mock.calls.length).toBeGreaterThan(calls))
    await vi.waitFor(() => expect(screen()).toContain('Preparing Fabric server files'))
    expect(screen()).not.toContain('previous run')
    expect(backend.delta).toHaveBeenLastCalledWith('server', undefined)
    expect(backend.terminal!.options.disableStdin).toBe(true)
})

it('unsubscribes and ignores a response that arrives after unmount', async () => {
    await mountTerminal()
    let finish!: (value: ServerConsoleDelta) => void
    backend.delta.mockReturnValueOnce(
        new Promise((resolve) => {
            finish = resolve
        }),
    )
    const calls = backend.delta.mock.calls.length
    backend.notify?.()
    await vi.waitFor(() => expect(backend.delta.mock.calls.length).toBeGreaterThan(calls))
    wrapper!.unmount()
    wrapper = undefined
    finish(delta('late response', 1))
    await flushPromises()
    expect(backend.unsubscribe).toHaveBeenCalledOnce()
    expect(backend.error).not.toHaveBeenCalled()
})

it('keeps terminal tools usable at narrow widths and preserves collapse after switching to logs', async () => {
    await mountTerminal()
    await output(
        '[20:17:33] [Server thread/INFO]: Starting minecraft server\r\n\x1b[32m[20:17:35] Done (2.0s)! For help, type "help"\x1b[0m\r\n> ',
    )
    for (const [width, theme] of [
        [1100, 'dark'],
        [720, 'dark'],
        [420, 'light'],
    ] as const) {
        await page.viewport(width, 640)
        applyTheme(theme)
        await flushPromises()
        const root = wrapper!.get('div').element
        await waitFor(() => root.getAnimations({ subtree: true }).length === 0)
        expect(root.scrollWidth).toBeLessThanOrEqual(root.clientWidth + 1)
        expect(wrapper!.get('button[aria-label="Download"]').isVisible()).toBe(true)
        if (import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE)
            await page.screenshot({
                element: root,
                path: `${import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE}/terminal-${width}-${theme}.png`,
            })
    }
    await wrapper!.get('button[aria-label="Expand"]').trigger('click')
    await wrapper!.setProps({ view: 'logs' })
    const collapse = document.querySelector<HTMLButtonElement>('button[aria-label="Collapse"]')!
    expect(collapse).not.toBeNull()
    collapse.click()
    await flushPromises()
    expect(document.querySelector('button[aria-label="Expand"]')).not.toBeNull()
})
