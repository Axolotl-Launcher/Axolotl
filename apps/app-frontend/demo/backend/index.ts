import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'

import { appHandlers } from './app'
import { authHandlers } from './auth'
import { homeHandlers } from './home'
import { instanceHandlers } from './instances'
import { runtimeHandlers } from './runtime'
import { settingsHandlers } from './settings'
import { tagHandlers } from './tags'
import type { Handlers } from './types'

const handlers: Handlers = {
    ...appHandlers,
    ...authHandlers,
    ...homeHandlers,
    ...instanceHandlers,
    ...runtimeHandlers,
    ...settingsHandlers,
    ...tagHandlers,
}

const reported = new Set<string>()

/** Commands without a demo answer resolve to null; each is reported once. */
function unhandled(cmd: string, args: unknown) {
    if (reported.has(cmd)) return
    reported.add(cmd)
    console.warn('[demo] unhandled command', cmd, args)
    ;((window as unknown as { __demoUnhandled?: string[] }).__demoUnhandled ??= []).push(cmd)
}

export function installDemoBackend() {
    // `@tauri-apps/plugin-os` reads these synchronously.
    Object.assign(window, {
        __TAURI_OS_PLUGIN_INTERNALS__: {
            platform: 'macos',
            os_type: 'macos',
            family: 'unix',
            arch: 'aarch64',
            version: '15.5.0',
            eol: '\n',
            exe_extension: '',
        },
    })
    mockIPC(
        async (cmd, args) => {
            // A Tauri reply arrives as a message, never in the same microtask checkpoint.
            await new Promise((resolve) => setTimeout(resolve, 0))
            const handler = handlers[cmd]
            if (!handler) {
                unhandled(cmd, args)
                return null
            }
            // Tauri hands the frontend JSON: dates arrive as strings, undefined fields vanish.
            const result = await handler((args ?? {}) as Record<string, unknown>)
            return result === undefined ? null : JSON.parse(JSON.stringify(result))
        },
        { shouldMockEvents: true },
    )
    mockWindows('main')
    // Demo files are bundled assets already served over HTTP, not paths on disk.
    const internals = (window as unknown as { __TAURI_INTERNALS__: Record<string, unknown> })
        .__TAURI_INTERNALS__
    internals.convertFileSrc = (path: string) => path
}
