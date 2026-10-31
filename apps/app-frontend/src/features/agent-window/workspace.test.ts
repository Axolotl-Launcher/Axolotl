import { readFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'

import { type Browser, chromium, type Page } from 'playwright'
import { afterAll, afterEach, beforeAll, beforeEach, expect, it } from 'vitest'

let server: Server
let browser: Browser
let page: Page
let baseUrl: string

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
    await page.addInitScript(() => {
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
    })
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

it('uses the English fallback for native window errors in Traditional Chinese', async () => {
    await page.addInitScript(() => {
        Reflect.set(window, '__TAURI_INTERNALS__', {
            invoke: async (command: string, args: { action?: string }) => {
                if (command.endsWith('|agent_window_get_appearance'))
                    return {
                        locale: 'zh-TW',
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
    })
    await page.reload()
    await page.waitForSelector('html[data-ready="true"]')
    await page.locator('#minimize').click()
    const error = page.locator('#window-error')
    await error.waitFor({ state: 'visible' })
    expect(await error.textContent()).toContain('Window action failed')
    expect(await page.locator('#guide-button').textContent()).toBe('Agent guide')
})

it('opens and replays the workflow guide in a narrow window', async () => {
    await page.setViewportSize({ width: 360, height: 640 })
    const guide = page.locator('#guide-dialog')
    await page.locator('#guide-button').click()
    await guide.waitFor({ state: 'visible' })
    const sections = guide.locator('section')
    expect(await sections.count()).toBe(4)
    for (const section of await sections.all()) {
        expect(await section.locator('h3').textContent()).toBeTruthy()
        expect(await section.locator('p').textContent()).toBeTruthy()
    }
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
