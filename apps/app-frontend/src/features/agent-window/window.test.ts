import { readFile } from 'node:fs/promises'
import { createRequire } from 'node:module'

import { expect, it, vi } from 'vitest'

const vm = createRequire(import.meta.url)('node:vm') as typeof import('node:vm')
const source = await readFile(
    new URL('../../../../../packages/agent-window/src/window.js', import.meta.url),
    'utf8',
)

function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (error: Error) => void
    const promise = new Promise<T>((done, fail) => {
        resolve = done
        reject = fail
    })
    return { promise, resolve, reject }
}

function appearance(locale: string) {
    return {
        locale,
        background: '#ffffff',
        surface: '#eeeeee',
        text: '#111111',
        accent: '#123456',
        labels: {
            title: locale,
            minimize: 'minimize',
            maximize: 'maximize',
            restore: 'restore',
            close: 'close',
        },
        messages: { windowActionError: 'Localized error' },
    }
}

function fixture() {
    const requests: ReturnType<typeof deferred<ReturnType<typeof appearance>>>[] = []
    const elements = new Map<
        string,
        EventTarget & {
            hidden: boolean
            textContent: string
            title: string
            setAttribute: ReturnType<typeof vi.fn>
        }
    >()
    const getElementById = (id: string) => {
        if (!elements.has(id)) {
            elements.set(
                id,
                Object.assign(new EventTarget(), {
                    hidden: true,
                    textContent: '',
                    title: '',
                    setAttribute: vi.fn(),
                }),
            )
        }
        return elements.get(id)!
    }
    const document = {
        documentElement: { lang: '', style: { setProperty: vi.fn() } },
        title: '',
        getElementById,
    }
    const window = Object.assign(new EventTarget(), {
        __TAURI_INTERNALS__: {
            invoke: (command: string) => {
                if (command.endsWith('|agent_window_get_appearance')) {
                    const request = deferred<ReturnType<typeof appearance>>()
                    requests.push(request)
                    return request.promise
                }
                return Promise.resolve(false)
            },
        },
    })
    const errors = vi.fn()
    vm.runInNewContext(source, {
        window,
        document,
        CSS: { supports: () => true },
        Event,
        console: { error: errors },
    })
    return {
        window,
        document,
        requests,
        errors,
        errorBox: getElementById('window-error'),
        ready: Reflect.get(window, 'AgentHost').ready as Promise<void>,
    }
}

it('keeps the latest appearance when an older native response arrives last', async () => {
    const view = fixture()
    view.window.dispatchEvent(new Event('agent-appearance-changed'))
    view.requests[1].resolve(appearance('fr-FR'))
    await vi.waitFor(() => expect(view.document.title).toBe('fr-FR'))
    view.requests[0].resolve(appearance('zh-CN'))
    await view.ready
    expect(view.document.documentElement.lang).toBe('fr-FR')
    expect(Reflect.get(view.window, 'AgentHost').appearance.locale).toBe('fr-FR')
})

it('waits for the latest startup appearance and ignores a superseded failure', async () => {
    const view = fixture()
    let ready = false
    void view.ready.then(() => {
        ready = true
    })
    view.window.dispatchEvent(new Event('agent-appearance-changed'))
    view.requests[0].reject(new Error('obsolete request'))
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(ready).toBe(false)
    view.requests[1].resolve(appearance('zh-TW'))
    await view.ready
    expect(view.document.documentElement.lang).toBe('zh-TW')
    expect(view.errorBox.hidden).toBe(true)
    expect(view.errors).not.toHaveBeenCalled()
})

it('uses the host error message without a hardcoded language branch', async () => {
    const view = fixture()
    view.requests[0].resolve(appearance('fr-FR'))
    await view.ready
    view.window.dispatchEvent(new Event('agent-appearance-changed'))
    view.requests[1].reject(new Error('current request'))
    await vi.waitFor(() => expect(view.errorBox.hidden).toBe(false))
    expect(view.errorBox.textContent).toBe('Localized error: Error: current request')
})
