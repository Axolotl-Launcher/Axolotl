import { readFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'

import { type Browser, chromium, type Page } from 'playwright'
import { afterAll, afterEach, beforeAll, beforeEach, expect, it } from 'vitest'

let server: Server
let browser: Browser
let page: Page
let baseUrl: string
const englishSource = JSON.parse(
    await readFile(new URL('../../locales/en-US/index.json', import.meta.url), 'utf8'),
) as Record<string, { message: string }>
const messages = Object.fromEntries(
    Object.entries(englishSource)
        .filter(([id]) => id.startsWith('app.agent-window.chat.'))
        .map(([id, entry]) => [
            id
                .slice('app.agent-window.chat.'.length)
                .replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase()),
            entry.message,
        ]),
)

beforeAll(async () => {
    const assets = new Map(
        await Promise.all(
            ['index.html', 'app.js', 'state.js', 'messages.js', 'window.js', 'window.css'].map(
                async (name) =>
                    [
                        `/${name}`,
                        await readFile(
                            new URL(
                                `../../../../../packages/agent-window/src/${name}`,
                                import.meta.url,
                            ),
                        ),
                    ] as const,
            ),
        ),
    )
    server = createServer((request, response) => {
        const path = request.url === '/' ? '/index.html' : request.url!
        const content = assets.get(path)
        if (!content) {
            response.writeHead(404).end()
            return
        }
        response.setHeader(
            'Content-Type',
            path.endsWith('.html')
                ? 'text/html'
                : path.endsWith('.css')
                  ? 'text/css'
                  : 'text/javascript',
        )
        response.end(content)
    })
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
    const address = server.address()
    if (!address || typeof address === 'string') throw new Error('Missing fixture address')
    baseUrl = `http://127.0.0.1:${address.port}`
    browser = await chromium.launch()
})

afterAll(async () => {
    await browser?.close()
    if (server)
        await new Promise<void>((resolve, reject) =>
            server.close((error) => (error ? reject(error) : resolve())),
        )
})

beforeEach(async () => {
    page = await browser.newPage({ reducedMotion: 'reduce' })
    page.setDefaultTimeout(5_000)
    await page.clock.install()
    await page.addInitScript((catalog) => {
        const fixture = {
            contextReads: 0,
            storageFails: false,
            policy: { full_access: false, computer_use: false },
            write: null as null | {
                resolve: (value: { full_access: boolean; computer_use: boolean }) => void
                reject: (error: Error) => void
            },
        }
        Object.assign(window, {
            agentFixture: fixture,
            AgentHost: {
                appearance: {
                    locale: 'en-US',
                    labels: {
                        title: 'Agent',
                        minimize: 'Minimize',
                        maximize: 'Maximize',
                        restore: 'Restore',
                        close: 'Close',
                    },
                    messages: catalog,
                },
                getContext: async () => {
                    fixture.contextReads++
                    return { ai_enabled: false, providers: [], instances: [] }
                },
                getPermissions: async () => fixture.policy,
                setPermissions: () =>
                    new Promise((resolve, reject) => {
                        fixture.write = {
                            resolve: (policy) => {
                                fixture.policy = policy
                                resolve(policy)
                            },
                            reject,
                        }
                    }),
            },
        })
        const setItem = Storage.prototype.setItem
        Storage.prototype.setItem = function (key, value) {
            if (fixture.storageFails) throw new DOMException('Quota exceeded', 'QuotaExceededError')
            setItem.call(this, key, value)
        }
    }, messages)
    await page.goto(baseUrl)
    await page.waitForSelector('html[data-ready="true"]')
    await page.waitForFunction(() => Reflect.get(window, 'agentFixture').contextReads > 0)
})

afterEach(async () => {
    await page?.context().close()
})

it('leaves idle context alone and refreshes when the window regains focus or a picker opens', async () => {
    const reads = () => page.evaluate(() => Reflect.get(window, 'agentFixture').contextReads)
    const initial = await reads()
    await page.clock.fastForward(20_000)
    expect(await reads()).toBe(initial)

    await page.evaluate(() => window.dispatchEvent(new Event('focus')))
    await page.waitForFunction(
        (previous) => Reflect.get(window, 'agentFixture').contextReads > previous,
        initial,
    )
    const focused = await reads()
    await page.locator('#model-button').click()
    await page.waitForFunction(
        (previous) => Reflect.get(window, 'agentFixture').contextReads > previous,
        focused,
    )
})

it('keeps permission confirmation open during the write and shows retryable failures inside it', async () => {
    await page.locator('#settings-button').click()
    await page.locator('#permission-full-access').click()
    await page.locator('#permissions-confirm').click()
    await page.waitForFunction(() => Reflect.get(window, 'agentFixture').write)
    expect(await page.locator('#permissions-cancel').isDisabled()).toBe(true)
    expect(await page.locator('#permissions-confirm').isDisabled()).toBe(true)

    await page.mouse.click(2, 2)
    await page.keyboard.press('Escape')
    expect(
        await page
            .locator('#permissions-risk-dialog')
            .evaluate((dialog) => dialog.hasAttribute('open')),
    ).toBe(true)
    await page.evaluate(() => {
        document.querySelector<HTMLButtonElement>('[data-action="close-dialog"]')!.click()
    })
    expect(await page.locator('#permissions-risk-dialog').isVisible()).toBe(true)

    await page.evaluate(() =>
        Reflect.get(window, 'agentFixture').write.reject(new Error('write failed')),
    )
    const error = page.locator('#permissions-risk-dialog [data-dialog-error]')
    await error.waitFor({ state: 'visible' })
    expect(await error.textContent()).toBeTruthy()
    expect(await page.locator('#permissions-cancel').isEnabled()).toBe(true)
    await page.locator('#permissions-confirm').click()
    await page.evaluate(() =>
        Reflect.get(window, 'agentFixture').write.resolve({
            full_access: true,
            computer_use: false,
        }),
    )
    await page.locator('#permissions-risk-dialog').waitFor({ state: 'hidden' })
    expect(await page.locator('#permission-full-access').isChecked()).toBe(true)
    await page.locator('#settings-dialog [data-action="close-dialog"]').click()
    await page.locator('#settings-button').click()
    expect(await page.locator('#permission-full-access').isChecked()).toBe(true)
})

it('retains a failed rename or deletion and reports storage errors in the active dialog', async () => {
    await page.locator('[data-prompt="promptCheck"]').click()
    await page.locator('#conversation-more').click()
    await page.locator('[data-action="rename"]').click()
    await page.locator('#rename-input').fill('Renamed task')
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = true))
    await page.locator('#edit-confirm').click()
    const error = page.locator('#edit-dialog [data-dialog-error]')
    await error.waitFor({ state: 'visible' })
    expect(await error.textContent()).toBeTruthy()
    expect(await page.locator('#rename-input').inputValue()).toBe('Renamed task')
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = false))
    await page.keyboard.press('Control+n')
    await page.keyboard.press('Meta+n')
    await page.locator('#edit-confirm').click()
    await page.locator('#edit-dialog').waitFor({ state: 'hidden' })
    expect(await page.locator('#conversation-title').textContent()).toBe('Renamed task')
    expect(await page.locator('#conversation-list [data-thread]').count()).toBe(1)

    await page.locator('#conversation-more').click()
    await page.locator('[data-action="delete"]').click()
    expect(await error.isVisible()).toBe(false)
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = true))
    await page.locator('#edit-confirm').click()
    await error.waitFor({ state: 'visible' })
    expect(await page.locator('#conversation-title').textContent()).toBe('Renamed task')
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = false))
    await page.keyboard.press('Control+n')
    await page.locator('#edit-confirm').click()
    await page.locator('#edit-dialog').waitFor({ state: 'hidden' })
    await page.reload()
    await page.waitForSelector('html[data-ready="true"]')
    expect(await page.locator('#message-input').inputValue()).toBe('')
    expect(await page.locator('#conversation-list').textContent()).not.toContain('Renamed task')
})

it.each(['zh-TW', 'fr-FR'])(
    'uses the supplied English fallback catalog while retaining native locale %s',
    async (locale) => {
        await page.addInitScript(
            ({ nativeLocale, catalog }) => {
                Reflect.set(window, '__TAURI_INTERNALS__', {
                    invoke: async (command: string, args: { action?: string }) => {
                        if (command.endsWith('|agent_window_get_appearance'))
                            return {
                                locale: nativeLocale,
                                messages: catalog,
                                labels: {
                                    title: 'Axolotl Agent',
                                    minimize: 'Minimize',
                                    maximize: 'Maximize',
                                    restore: 'Restore',
                                    close: 'Close',
                                },
                            }
                        if (args?.action === 'minimize') throw new Error('Window unavailable')
                        return false
                    },
                })
            },
            { nativeLocale: locale, catalog: messages },
        )
        await page.reload()
        await page.waitForSelector('html[data-ready="true"]')
        await page.locator('#minimize').click()
        const error = page.locator('#window-error')
        await error.waitFor({ state: 'visible' })
        expect(await error.textContent()).toContain(messages.windowActionError)
        expect(await page.locator('#guide-button').textContent()).toBe(messages.guide)
        expect(await page.locator('html').getAttribute('lang')).toBe(locale)
    },
)

it('opens and replays the workflow guide in a narrow window', async () => {
    await page.setViewportSize({ width: 360, height: 640 })
    const guide = page.locator('#guide-dialog')
    await page.locator('#guide-button').click()
    await guide.waitFor({ state: 'visible' })
    const sections = guide.locator('section')
    expect(await sections.count()).toBe(4)
    for (const section of await sections.all()) {
        for (const content of [section.locator('h3'), section.locator('p')]) {
            const key = await content.getAttribute('data-text')
            expect(await content.textContent()).toBe(messages[key!])
        }
    }
    expect(await guide.locator('[data-text="historyRetention"]').textContent()).toBe(
        messages.historyRetention,
    )
    const bounds = await guide.boundingBox()
    expect(bounds!.x).toBeGreaterThanOrEqual(0)
    expect(bounds!.y).toBeGreaterThanOrEqual(0)
    expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(360)
    expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(640)
    await guide.locator('[data-action="close-dialog"]').click()
    await guide.waitFor({ state: 'hidden' })
    await page.locator('#guide-button').click()
    await guide.waitFor({ state: 'visible' })
    expect(await sections.count()).toBe(4)
    await page.keyboard.press('Escape')
    await guide.waitFor({ state: 'hidden' })
})

async function chooseModel() {
    await page.evaluate(() => {
        Reflect.get(window, 'AgentHost').getContext = async () => ({
            ai_enabled: true,
            providers: [
                {
                    provider_id: 'provider',
                    name: 'Provider',
                    models: [{ id: 'model', name: 'Model' }],
                },
            ],
            instances: [],
        })
        window.dispatchEvent(new Event('focus'))
    })
    await page.locator('#model-button').click()
    await page.locator('[data-model="model"]').click()
}

it('preserves rejected storage until explicit recovery and can export it and the current draft', async () => {
    const raw = '{"version":999,"threads":["precious original"]}'
    await page.evaluate(
        (value) => localStorage.setItem('axolotl.agent-window.conversations.v1', value),
        raw,
    )
    await page.reload()
    await page.waitForSelector('html[data-ready="true"]')
    await page.locator('#message-input').fill('Recovery draft')
    expect(
        await page.evaluate(() => localStorage.getItem('axolotl.agent-window.conversations.v1')),
    ).toBe(raw)
    for (const [action, expected] of [
        ['export-history', raw],
        ['export-draft', 'Recovery draft'],
    ]) {
        const downloaded = page.waitForEvent('download')
        await page.locator(`[data-action="${action}"]`).click()
        const download = await downloaded
        expect(await readFile((await download.path())!, 'utf8')).toBe(expected)
    }
    await page.locator('[data-action="reset-history"]').click()
    await page.locator('[data-action="confirm-reset-history"]').click()
    await page.locator('#storage-reset-dialog').waitFor({ state: 'hidden' })
    expect(await page.locator('#message-input').inputValue()).toBe('Recovery draft')
    expect(
        await page.evaluate(
            () =>
                JSON.parse(localStorage.getItem('axolotl.agent-window.conversations.v1')!)
                    .threads[0].draft,
        ),
    ).toBe('Recovery draft')
})

it('keeps existing message DOM while editing the next draft', async () => {
    await chooseModel()
    await page.locator('#message-input').fill('Existing request')
    await page.locator('#send-button').click()
    const message = await page.locator('[data-message]').elementHandle()
    await page.locator('#message-input').fill('The next draft')
    expect(await message!.evaluate((node) => node.isConnected)).toBe(true)
    expect(await page.locator('#message-input').inputValue()).toBe('The next draft')
    expect(await page.locator('.user-message').textContent()).toBe('Existing request')
})

it('keeps the actual draft and existing messages when sending cannot commit, then retries', async () => {
    await chooseModel()
    await page.locator('#message-input').fill('First request')
    await page.locator('#send-button').click()
    expect(await page.locator('[data-message]').count()).toBe(1)
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = true))
    await page.locator('#message-input').fill('Unsaved second request')
    await page.locator('#send-button').click()
    expect(await page.locator('#message-input').inputValue()).toBe('Unsaved second request')
    expect(await page.locator('[data-message]').count()).toBe(1)
    expect(await page.locator('.user-message').textContent()).toBe('First request')
    const downloaded = page.waitForEvent('download')
    await page.locator('[data-action="export-draft"]').click()
    expect(await readFile((await (await downloaded).path())!, 'utf8')).toBe(
        'Unsaved second request',
    )
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = false))
    await page.locator('[data-action="retry-storage"]').click()
    await page.locator('#send-button').click()
    expect(await page.locator('[data-message]').count()).toBe(2)
    expect(await page.locator('#message-input').inputValue()).toBe('')
})

it('times out stuck context requests, permits retry, and ignores their late result', async () => {
    await page.evaluate(() => {
        Reflect.get(window, 'AgentHost').getContext = () =>
            new Promise((resolve) => {
                Reflect.set(window, 'resolveStaleContext', resolve)
            })
        window.dispatchEvent(new Event('focus'))
    })
    await page.waitForFunction(() => Reflect.get(window, 'resolveStaleContext'))
    await page.clock.fastForward(15_001)
    await page.locator('[data-action="refresh-context"]').waitFor({ state: 'visible' })
    await chooseModel()
    expect(await page.locator('#model-label').textContent()).toBe('Model')
    await page.evaluate(() =>
        Reflect.get(
            window,
            'resolveStaleContext',
        )({
            ai_enabled: false,
            providers: [],
            instances: [],
        }),
    )
    await page.waitForTimeout(50)
    expect(await page.locator('#model-label').textContent()).toBe('Model')
    expect(await page.locator('#context-banner').isHidden()).toBe(true)
})

it('renders stored and provider strings as text without using HTML parsing sinks', async () => {
    const hostile = '<img src=x onerror="window.agentXss=true"> & "quoted"'
    await page.evaluate((value) => {
        const key = 'axolotl.agent-window.conversations.v1'
        const store = JSON.parse(
            localStorage.getItem(key) || '{"version":1,"activeId":"x","threads":[{"id":"x"}]}',
        )
        const thread = store.threads[0]
        thread.title = value
        thread.messages = [{ id: value, text: value, status: 'stopped' }]
        thread.selection = { provider_id: value, model_id: value }
        thread.instanceId = value
        localStorage.setItem(key, JSON.stringify(store))
    }, hostile)
    await page.addInitScript((value) => {
        Reflect.get(window, 'AgentHost').getContext = async () => ({
            ai_enabled: true,
            providers: [{ provider_id: value, name: value, models: [{ id: value, name: value }] }],
            instances: [{ id: value, name: value, loader: value, game_version: value }],
        })
        Object.defineProperty(Element.prototype, 'innerHTML', {
            set() {
                throw new Error('HTML parsing sink used')
            },
        })
    }, hostile)
    await page.reload()
    await page.waitForSelector('html[data-ready="true"]')
    expect(await page.locator('.thread-name').textContent()).toBe(hostile)
    expect(await page.locator('.user-message').textContent()).toBe(hostile)
    await page.locator('#model-button').click()
    expect(await page.locator('.picker-option strong').textContent()).toBe(hostile)
    await page.locator('#picker-search').press('Escape')
    await page.locator('#instance-button').click()
    expect(await page.locator('.picker-option strong').allTextContents()).toContain(hostile)
    await page.locator('#picker-search').press('Escape')
    await page.locator('#task-toggle').click()
    expect(await page.locator('.instance-summary strong').textContent()).toBe(hostile)
    expect(await page.locator('img[src="x"]').count()).toBe(0)
    expect(await page.evaluate(() => Reflect.get(window, 'agentXss'))).toBeUndefined()
})

it.each([false, true])(
    'retries native reveal without blocking context initialization (permanent failure: %s)',
    async (permanent) => {
        await page.addInitScript(
            ({ alwaysFail, catalog }) => {
                Reflect.set(window, 'revealCalls', 0)
                Reflect.set(window, '__TAURI_INTERNALS__', {
                    invoke: async (command: string, args: { action?: string }) => {
                        if (command.endsWith('|agent_window_get_appearance'))
                            return {
                                locale: 'en-US',
                                messages: catalog,
                                labels: {
                                    title: 'Agent',
                                    minimize: 'Minimize',
                                    maximize: 'Maximize',
                                    restore: 'Restore',
                                    close: 'Close',
                                },
                            }
                        if (args?.action === 'show') {
                            const calls = Reflect.get(window, 'revealCalls') + 1
                            Reflect.set(window, 'revealCalls', calls)
                            if (alwaysFail || calls < 3)
                                throw new Error('Native window unavailable')
                        }
                        return false
                    },
                })
            },
            { alwaysFail: permanent, catalog: messages },
        )
        const errors: string[] = []
        page.on('pageerror', (error) => errors.push(error.message))
        await page.reload()
        await page.waitForSelector('html[data-ready="true"]')
        await page.waitForFunction(() => Reflect.get(window, 'agentFixture').contextReads > 0)
        await page.waitForFunction(() => Reflect.get(window, 'revealCalls') === 1)
        for (const calls of [2, 3]) {
            await page.clock.fastForward(501)
            await page.waitForFunction(
                (expected) => Reflect.get(window, 'revealCalls') === expected,
                calls,
            )
        }
        if (permanent) await page.locator('#notification').waitFor({ state: 'visible' })
        expect(errors).toEqual([])
    },
)

it('reveals a native startup diagnostic without raw message keys and recovers after the catalog arrives', async () => {
    await page.addInitScript((catalog) => {
        Reflect.set(window, 'nativeCatalogReady', false)
        Reflect.set(window, 'revealCalls', 0)
        Reflect.set(window, '__TAURI_INTERNALS__', {
            invoke: async (command: string, args: { action?: string }) => {
                if (command.endsWith('|agent_window_get_appearance'))
                    return {
                        locale: 'fr-FR',
                        labels: {
                            title: 'Agent',
                            minimize: 'Minimize',
                            maximize: 'Maximize',
                            restore: 'Restore',
                            close: 'Close',
                        },
                        messages: Reflect.get(window, 'nativeCatalogReady') ? catalog : undefined,
                    }
                if (args?.action === 'show')
                    Reflect.set(window, 'revealCalls', Reflect.get(window, 'revealCalls') + 1)
                return false
            },
        })
    }, messages)
    await page.reload()
    await page.waitForSelector('html[data-ready="true"]')
    await page.waitForFunction(() => Reflect.get(window, 'revealCalls') > 0)
    expect(await page.locator('#workspace').isHidden()).toBe(true)
    expect(await page.locator('#guide-button').isHidden()).toBe(true)
    expect(await page.locator('#settings-button').isHidden()).toBe(true)
    expect(await page.locator('[data-text="newChat"]').textContent()).toBe('')
    expect(await page.locator('#window-error').textContent()).toContain(
        'message catalog is unavailable',
    )
    expect(await page.evaluate(() => Reflect.get(window, 'agentFixture').contextReads)).toBe(0)
    await page.evaluate(() => {
        Reflect.set(window, 'nativeCatalogReady', true)
        window.dispatchEvent(new Event('agent-appearance-changed'))
    })
    await page.locator('#workspace').waitFor({ state: 'visible' })
    await page.waitForFunction(() => Reflect.get(window, 'agentFixture').contextReads > 0)
    expect(await page.locator('#guide-button').textContent()).toBe(messages.guide)
    expect(await page.locator('#window-error').isHidden()).toBe(true)
})
