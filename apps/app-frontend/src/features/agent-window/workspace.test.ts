import { readdir, readFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'

import { type Browser, chromium, type Page } from 'playwright'
import { afterAll, afterEach, beforeAll, beforeEach, expect, it } from 'vitest'

let server: Server
let browser: Browser
let page: Page
let baseUrl: string
const STORE = 'axolotl.agent-window.store.v2'
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
    const source = new URL('../../../../../packages/agent-window/src/', import.meta.url)
    const names = (await readdir(source)).filter((name) => /\.(html|js|css)$/.test(name))
    const assets = new Map<string, Buffer>(
        await Promise.all(
            names.map(async (name) => [`/${name}`, await readFile(new URL(name, source))] as const),
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
    page = await browser.newPage({ reducedMotion: 'reduce', acceptDownloads: true })
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
    await ready()
})

afterEach(async () => {
    await page?.context().close()
})

async function ready() {
    await page.waitForSelector('html[data-ready="true"]')
    await page.waitForFunction(() => Reflect.get(window, 'agentFixture').contextReads > 0)
}

async function send(value: string) {
    await page.locator('#message-input').fill(value)
    await page.locator('#message-input').press('Enter')
}

async function untilPlan() {
    const approve = page.getByTestId('approve-plan').last()
    for (let index = 0; index < 40 && !(await approve.isVisible()); index++)
        await page.clock.runFor(500)
    await approve.waitFor()
    return approve
}

async function untilSettled() {
    await page.clock.runFor(100)
    const status = page.locator('.run-status:not([hidden])')
    for (let index = 0; index < 40 && (await status.count()); index++) await page.clock.runFor(500)
    expect(await status.count()).toBe(0)
}

function useContext(context: Record<string, unknown>) {
    return page.evaluate((value) => {
        Reflect.get(window, 'AgentHost').getContext = async () => {
            Reflect.get(window, 'agentFixture').contextReads++
            return value
        }
        window.dispatchEvent(new Event('focus'))
    }, context)
}

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

it('walks a first-time user from a suggestion to an applied, labelled preview result', async () => {
    expect(await page.locator('#intro').isVisible()).toBe(true)
    expect(await page.locator('#preview-badge').isVisible()).toBe(true)
    expect(await page.locator('#composer-home #composer').count()).toBe(1)
    await page.locator('[data-prompt="promptCrash"]').click()
    expect(await page.locator('#message-input').inputValue()).toBe(messages.promptCrash)
    await page.locator('#send-button').click()

    await page.locator('.user-bubble').waitFor()
    expect(await page.locator('#composer-dock #composer').count()).toBe(1)
    expect(await page.locator('#message-input').inputValue()).toBe('')
    await page.clock.runFor(1_500)
    expect(await page.locator('.activity .step').count()).toBeGreaterThan(0)
    expect(await page.locator('#send-button').getAttribute('aria-label')).toBe(messages.stop)

    const approve = await untilPlan()
    expect(await page.locator('.plan-card').textContent()).toContain('Entity Culling')
    expect(await page.locator('.plan-card').textContent()).toContain(messages.approvalPromise)
    expect(await page.locator('#conversation-list').textContent()).toContain('Fix the launch crash')
    expect(await page.locator('.thread-row[data-state="awaiting"]').count()).toBe(1)
    await approve.click()
    await untilSettled()

    expect(await page.locator('.plan-card').getAttribute('data-state')).toBe('applied')
    const result = page.locator('.result-card.is-success')
    expect(await result.textContent()).toContain(messages.previewResult)
    await page.locator('#task-toggle').click()
    expect(await page.locator('.checklist li[data-state="done"]').count()).toBe(3)
    expect(await page.locator('.log-list code').allTextContents()).toContain(
        'resource_history_rollback',
    )

    await page.reload()
    await ready()
    expect(await page.locator('.result-card.is-success').count()).toBe(1)
    await page.getByTestId('new-chat').click()
    expect(await page.locator('#intro').isHidden()).toBe(true)
})

it('stops a run that is only reading and offers to run it again', async () => {
    await send('My game crashes on launch')
    await page.clock.runFor(1_500)
    await page.locator('#message-input').press('Escape')
    const notice = page.locator('.run-notice')
    await notice.waitFor()
    expect(await notice.textContent()).toContain(messages.noticeStopped)
    expect(await page.locator('.step[data-state="running"]').count()).toBe(0)
    await page.clock.runFor(10_000)
    expect(await page.locator('.plan-card').count()).toBe(0)

    await notice.locator('[data-action="retry"]').click()
    await untilPlan()
    expect(await page.locator('.turn').count()).toBe(1)
    expect(await page.locator('.run-notice:not([hidden])').count()).toBe(0)
})

it('treats a stop during writing as unknown until the status is checked', async () => {
    await send('My game crashes on launch')
    await (await untilPlan()).click()
    await page.clock.runFor(400)
    await page.locator('#send-button.is-stop').click()
    const notice = page.locator('.run-notice')
    await notice.waitFor()
    expect(await notice.textContent()).toContain(messages.noticeDetached)
    expect(await page.locator('.plan-card').getAttribute('data-state')).toBe('unknown')
    expect(await page.locator('.thread-row[data-state="attention"]').count()).toBe(1)

    await page.reload()
    await ready()
    await page.locator('.run-notice [data-action="check-status"]').click()
    await untilSettled()
    expect(await page.locator('.plan-card').getAttribute('data-state')).toBe('applied')
    expect(await page.locator('.result-card.is-success').count()).toBe(1)
})

it('shows a retryable failure and recovers on retry', async () => {
    await page.addInitScript(() => {
        Reflect.get(window, 'AgentHost').demo = { fail: 1 }
    })
    await page.reload()
    await ready()
    await send('The game stutters')
    await untilSettled()
    const error = page.locator('.result-card.is-error')
    expect(await error.textContent()).toContain('Couldn’t reach Modrinth')
    expect(await page.locator('.step[data-state="failed"]').count()).toBe(1)
    await error.locator('[data-action="retry"]').click()
    await untilPlan()
    expect(await page.locator('.result-card.is-error').count()).toBe(0)
})

it('queues follow-ups while working, lets them replace a waiting plan, and declines safely', async () => {
    await send('The game stutters')
    await page.clock.runFor(800)
    await send('Why those two mods?')
    const queued = page.locator('.queued')
    expect(await queued.textContent()).toContain('Why those two mods?')
    expect(await queued.textContent()).toContain(messages.queuedTag)

    await untilPlan()
    await page.clock.runFor(500)
    await untilPlan()
    expect(await page.locator('.turn').count()).toBe(2)
    expect(await queued.count()).toBe(0)
    expect(await page.locator('.plan-card').first().getAttribute('data-state')).toBe('superseded')
    expect(await page.locator('.plan-card').first().textContent()).toContain(
        messages.planSuperseded,
    )

    await page.locator('.plan-card').last().locator('[data-action="decline"]').click()
    expect(await page.locator('.plan-card').last().getAttribute('data-state')).toBe('declined')
    expect(await page.locator('.plan-card').last().textContent()).toContain(messages.planDeclined)
    expect(await page.locator('#thread-state').isHidden()).toBe(true)
})

it('attaches files by name, lets them be removed and passes them to the agent', async () => {
    const file = (name: string) => ({ name, mimeType: 'text/plain', buffer: Buffer.from('log') })
    await page.locator('#file-input').setInputFiles([file('latest.log'), file('crash.txt')])
    expect(await page.locator('#attachment-list .file-name').allTextContents()).toEqual([
        'latest.log',
        'crash.txt',
    ])
    await page.getByRole('button', { name: 'Remove crash.txt' }).click()
    expect(await page.locator('#attachment-list .file-name').allTextContents()).toEqual([
        'latest.log',
    ])
    await send('It crashes, see the log')
    expect(await page.locator('.user-bubble .file-chip').textContent()).toBe('latest.log')
    expect(await page.locator('#attachment-list').isHidden()).toBe(true)
    await page.clock.runFor(2_000)
    expect(await page.locator('.step-detail').first().textContent()).toBe('latest.log')
})

it('groups models by provider and explains setup while no model is available', async () => {
    expect(await page.locator('#model-label').textContent()).toBe(messages.setupModel)
    await page.locator('#model-button').click()
    expect(await page.locator('.picker-setup').textContent()).toContain(messages.setupModelHint)

    await useContext({
        ai_enabled: true,
        providers: [
            { provider_id: 'a', name: 'Provider A', models: [{ id: 'same', name: 'Same model' }] },
            {
                provider_id: 'b',
                name: 'Provider B',
                models: [
                    { id: 'same', name: 'Same model' },
                    { id: 'other', name: 'Other model' },
                ],
            },
        ],
        instances: [{ id: 'mist', name: 'Morning Mist', game_version: '1.21.1', loader: 'fabric' }],
    })
    await page.locator('.picker-group').first().waitFor()
    expect(await page.locator('.picker-group').allTextContents()).toEqual([
        'Provider A',
        'Provider B',
    ])
    expect(await page.locator('#model-label').textContent()).toBe('Same model')
    await page.locator('[data-provider="b"][data-model="same"]').click()
    expect(await page.locator('#model-button').getAttribute('title')).toBe(
        'Provider B · Same model',
    )

    await page.locator('#instance-button').click()
    await page.locator('[data-instance="mist"]').click()
    expect(await page.locator('#instance-label').textContent()).toBe('Morning Mist')
    await page.locator('#message-input').fill('draft')
    await page.getByTestId('new-chat').click()
    expect(await page.locator('#model-button').getAttribute('title')).toBe(
        'Provider B · Same model',
    )
    expect(await page.locator('#instance-label').textContent()).toBe('Morning Mist')
})

it('keeps permission confirmation open during the write and shows retryable failures inside it', async () => {
    await page.locator('#settings-button').click()
    await page.locator('#permission-full-access').click()
    const dialog = page.locator('#confirm-dialog')
    expect(await page.locator('#confirm-title').textContent()).toBe(messages.fullAccessWarningTitle)
    await page.locator('#confirm-accept').click()
    await page.waitForFunction(() => Reflect.get(window, 'agentFixture').write)
    expect(await page.locator('#confirm-cancel').isDisabled()).toBe(true)
    expect(await page.locator('#confirm-accept').isDisabled()).toBe(true)

    await page.mouse.click(2, 2)
    await page.keyboard.press('Escape')
    expect(await dialog.evaluate((node) => node.hasAttribute('open'))).toBe(true)

    await page.evaluate(() =>
        Reflect.get(window, 'agentFixture').write.reject(new Error('write failed')),
    )
    const error = dialog.locator('[data-dialog-error]')
    await error.waitFor({ state: 'visible' })
    expect(await error.textContent()).toBe(messages.permissionSaveError)
    expect(await page.locator('#confirm-cancel').isEnabled()).toBe(true)
    await page.locator('#confirm-accept').click()
    await page.evaluate(() =>
        Reflect.get(window, 'agentFixture').write.resolve({
            full_access: true,
            computer_use: false,
        }),
    )
    await dialog.waitFor({ state: 'hidden' })
    expect(await page.locator('#permission-full-access').isChecked()).toBe(true)
    expect(await page.locator('#permission-computer-use').isEnabled()).toBe(true)
    await page.locator('#settings-dialog [data-action="close-dialog"]').click()
    await page.locator('#settings-button').click()
    expect(await page.locator('#permission-full-access').isChecked()).toBe(true)
})

it('retains a failed rename or deletion and reports storage errors in the open dialog', async () => {
    await page.locator('[data-prompt="promptBackup"]').click()
    await page.locator('#conversation-more').click()
    await page.locator('[data-menu-rename]').click()
    await page.locator('#rename-input').fill('Renamed task')
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = true))
    await page.locator('#confirm-accept').click()
    const error = page.locator('#confirm-dialog [data-dialog-error]')
    await error.waitFor({ state: 'visible' })
    expect(await error.textContent()).toBe(messages.storageError)
    expect(await page.locator('#rename-input').inputValue()).toBe('Renamed task')
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = false))
    await page.keyboard.press('Control+n')
    await page.locator('#confirm-accept').click()
    await page.locator('#confirm-dialog').waitFor({ state: 'hidden' })
    expect(await page.locator('#conversation-title').textContent()).toBe('Renamed task')
    expect(await page.locator('#conversation-list [data-thread]').count()).toBe(1)
    expect(await page.locator('.banner').count()).toBe(0)

    await page.locator('#conversation-more').click()
    await page.locator('[data-menu-delete]').click()
    expect(await error.isVisible()).toBe(false)
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = true))
    await page.locator('#confirm-accept').click()
    await error.waitFor({ state: 'visible' })
    expect(await page.locator('#conversation-title').textContent()).toBe('Renamed task')
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = false))
    await page.locator('#confirm-accept').click()
    await page.locator('#confirm-dialog').waitFor({ state: 'hidden' })
    await page.reload()
    await ready()
    expect(await page.locator('#message-input').inputValue()).toBe('')
    expect(await page.locator('#conversation-list').textContent()).not.toContain('Renamed task')
})

it('uses drawers on a narrow window and keeps dialogs inside it', async () => {
    await page.setViewportSize({ width: 680, height: 600 })
    await page.locator('[data-prompt="promptPack"]').click()
    expect(await page.locator('#sidebar').evaluate((node) => (node as HTMLElement).inert)).toBe(
        true,
    )
    await page.locator('#sidebar-toggle').click()
    expect(await page.locator('.drawer-backdrop').isVisible()).toBe(true)
    await page.keyboard.press('Escape')
    expect(await page.locator('.drawer-backdrop').isVisible()).toBe(false)
    // Only one drawer is open at a time.
    await page.locator('#task-toggle').click()
    expect(await page.locator('#task-panel').evaluate((node) => (node as HTMLElement).inert)).toBe(
        false,
    )
    await page.locator('#sidebar-toggle').click()
    expect(await page.locator('#task-panel').evaluate((node) => (node as HTMLElement).inert)).toBe(
        true,
    )
    await page.locator('.drawer-backdrop').click({ position: { x: 600, y: 300 } })
    expect(await page.locator('.drawer-backdrop').isVisible()).toBe(false)

    const help = page.locator('#help-dialog')
    await page.locator('#help-button').click()
    await help.waitFor()
    expect(await help.locator('.help-steps li').count()).toBe(3)
    const bounds = await help.boundingBox()
    expect(bounds!.x).toBeGreaterThanOrEqual(0)
    expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(680)
    expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(600)
    await page.keyboard.press('Escape')
    await help.waitFor({ state: 'hidden' })

    await page.setViewportSize({ width: 1280, height: 800 })
    // Back on a wide window the task list returns to its saved, open state.
    await page.waitForFunction(() => !document.getElementById('sidebar')!.inert)
})

it('restores a plan that was waiting for approval after a restart', async () => {
    await send('Back up my world')
    await untilPlan()
    await page.reload()
    await ready()
    const approve = page.getByTestId('approve-plan')
    expect(await approve.isVisible()).toBe(true)
    await approve.click()
    await untilSettled()
    expect(await page.locator('.result-card.is-success').textContent()).toContain('Backup created')
})

it('preserves rejected storage until explicit recovery and can export it and the current draft', async () => {
    const raw = '{"version":999,"threads":["precious original"]}'
    await page.evaluate(({ key, value }) => localStorage.setItem(key, value), {
        key: STORE,
        value: raw,
    })
    await page.reload()
    await ready()
    await page.locator('#message-input').fill('Recovery draft')
    await page.clock.runFor(1_000)
    expect(await page.evaluate((key) => localStorage.getItem(key), STORE)).toBe(raw)
    for (const [action, expected] of [
        ['export-history', raw],
        ['export-draft', 'Recovery draft'],
    ]) {
        const downloaded = page.waitForEvent('download')
        await page.locator(`.banner [data-action="${action}"]`).click()
        const download = await downloaded
        expect(await readFile((await download.path())!, 'utf8')).toBe(expected)
    }
    await page.locator('.banner [data-action="reset-history"]').click()
    await page.locator('#confirm-accept').click()
    await page.locator('#confirm-dialog').waitFor({ state: 'hidden' })
    expect(await page.locator('#message-input').inputValue()).toBe('Recovery draft')
    expect(
        await page.evaluate(
            (key) => JSON.parse(localStorage.getItem(key)!).threads[0].draft,
            STORE,
        ),
    ).toBe('Recovery draft')
    expect(await page.locator('.banner').count()).toBe(0)
})

it('keeps the draft and existing turns when sending cannot be saved, then retries', async () => {
    await send('First request')
    await page.locator('#message-input').press('Escape')
    expect(await page.locator('.turn').count()).toBe(1)
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = true))
    await send('Unsaved second request')
    expect(await page.locator('#message-input').inputValue()).toBe('Unsaved second request')
    expect(await page.locator('.turn').count()).toBe(1)
    expect(await page.locator('.user-bubble').textContent()).toBe('First request')
    const downloaded = page.waitForEvent('download')
    await page.locator('.banner [data-action="export-draft"]').click()
    expect(await readFile((await (await downloaded).path())!, 'utf8')).toBe(
        'Unsaved second request',
    )
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = false))
    await page.locator('.banner [data-action="retry-storage"]').click()
    expect(await page.locator('.banner').count()).toBe(0)
    await page.locator('#send-button').click()
    expect(await page.locator('.turn').count()).toBe(2)
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
    await page.locator('.banner [data-action="refresh-context"]').waitFor({ state: 'visible' })
    await useContext({
        ai_enabled: true,
        providers: [
            { provider_id: 'provider', name: 'Provider', models: [{ id: 'model', name: 'Model' }] },
        ],
        instances: [],
    })
    await page.locator('.banner').waitFor({ state: 'detached' })
    expect(await page.locator('#model-label').textContent()).toBe('Model')
    await page.evaluate(() =>
        Reflect.get(
            window,
            'resolveStaleContext',
        )({ ai_enabled: false, providers: [], instances: [] }),
    )
    await page.waitForTimeout(50)
    expect(await page.locator('#model-label').textContent()).toBe('Model')
    expect(await page.locator('.banner').count()).toBe(0)
})

it('renders stored, provider and agent strings as text without HTML parsing sinks', async () => {
    const hostile = '<img src=x onerror="window.agentXss=true"> & "quoted"'
    await page.evaluate(
        ({ key, value }) => {
            const run = {
                status: 'awaiting',
                items: [
                    {
                        id: 'a',
                        type: 'activity',
                        steps: [
                            { id: 's', title: value, detail: value, tool: value, status: 'done' },
                        ],
                    },
                    { id: 't', type: 'text', text: `**${value}** \`${value}\`` },
                    {
                        id: 'p',
                        type: 'plan',
                        state: 'pending',
                        plan: {
                            title: value,
                            summary: value,
                            changes: [{ name: value, note: value, from: value, to: value }],
                            notes: [value],
                        },
                    },
                ],
                progress: [{ title: value, detail: value, status: 'done' }],
            }
            localStorage.setItem(
                key,
                JSON.stringify({
                    version: 2,
                    activeId: 'x',
                    panelOpen: true,
                    threads: [
                        {
                            id: 'x',
                            title: value,
                            selection: { provider_id: value, model_id: value },
                            instanceId: value,
                            turns: [
                                { id: value, text: value, attachments: [{ name: value }], run },
                            ],
                        },
                    ],
                }),
            )
        },
        { key: STORE, value: hostile },
    )
    await page.addInitScript((value) => {
        Reflect.get(window, 'AgentHost').getContext = async () => {
            Reflect.get(window, 'agentFixture').contextReads++
            return {
                ai_enabled: true,
                providers: [
                    { provider_id: value, name: value, models: [{ id: value, name: value }] },
                ],
                instances: [{ id: value, name: value, loader: value, game_version: value }],
            }
        }
        for (const property of ['innerHTML', 'outerHTML'])
            Object.defineProperty(Element.prototype, property, {
                set() {
                    throw new Error('HTML parsing sink used')
                },
            })
        Element.prototype.insertAdjacentHTML = () => {
            throw new Error('HTML parsing sink used')
        }
    }, hostile)
    const errors: string[] = []
    page.on('pageerror', (error) => errors.push(error.message))
    await page.reload()
    await ready()
    expect(await page.locator('.thread-name').textContent()).toBe(hostile)
    expect(await page.locator('.user-bubble p').textContent()).toBe(hostile)
    expect(await page.locator('.agent-text strong').textContent()).toBe(hostile)
    expect(await page.locator('.agent-text code').textContent()).toBe(hostile)
    expect(await page.locator('.plan-card h3').textContent()).toBe(hostile)
    expect(await page.locator('.step-title').textContent()).toBe(hostile)
    expect(await page.locator('.checklist strong').textContent()).toBe(hostile)
    await page.locator('#model-button').click()
    expect(await page.locator('.picker-option strong').textContent()).toBe(hostile)
    await page.locator('#picker-search').press('Escape')
    await page.locator('#instance-button').click()
    expect(await page.locator('.picker-option strong').allTextContents()).toContain(hostile)
    await page.locator('#picker-search').press('Escape')
    expect(await page.locator('img[src="x"]').count()).toBe(0)
    expect(await page.evaluate(() => Reflect.get(window, 'agentXss'))).toBeUndefined()
    expect(errors).toEqual([])
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
        await ready()
        await page.locator('#minimize').click()
        const error = page.locator('#window-error')
        await error.waitFor({ state: 'visible' })
        expect(await error.textContent()).toContain(messages.windowActionError)
        expect(await page.locator('.new-task [data-text="newChat"]').textContent()).toBe(
            messages.newChat,
        )
        expect(await page.locator('html').getAttribute('lang')).toBe(locale)
    },
)

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
        await ready()
        await page.waitForFunction(() => Reflect.get(window, 'revealCalls') === 1)
        for (const calls of [2, 3]) {
            await page.clock.fastForward(501)
            await page.waitForFunction(
                (expected) => Reflect.get(window, 'revealCalls') === expected,
                calls,
            )
        }
        if (permanent) {
            const toast = page.locator('.toast')
            await toast.waitFor({ state: 'visible' })
            expect(await toast.textContent()).toBe(messages.windowRevealError)
        }
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
    expect(await page.locator('#help-button').isHidden()).toBe(true)
    expect(await page.locator('#settings-button').isHidden()).toBe(true)
    expect(await page.locator('.new-task [data-text="newChat"]').textContent()).toBe('')
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
    expect(await page.locator('#help-button').isVisible()).toBe(true)
    expect(await page.locator('.new-task [data-text="newChat"]').textContent()).toBe(
        messages.newChat,
    )
    expect(await page.locator('#window-error').isHidden()).toBe(true)
})

it('opens a command menu that runs actions and jumps to tasks', async () => {
    await send('Back up my world')
    await page.locator('#message-input').press('Escape')
    await page.getByTestId('new-chat').click()
    await page.keyboard.press('Control+k')
    await page.locator('#palette').waitFor()
    expect(await page.locator('.palette-row').count()).toBeGreaterThan(5)
    await page.locator('#palette-input').fill('settings')
    expect(await page.locator('.palette-row').count()).toBe(1)
    await page.keyboard.press('Enter')
    await page.locator('#settings-dialog').waitFor()
    await page.keyboard.press('Escape')
    await page.locator('#settings-dialog').waitFor({ state: 'hidden' })

    await page.keyboard.press('Control+k')
    await page.locator('#palette-input').fill('back up')
    expect(await page.locator('.palette-row').count()).toBe(1)
    await page.keyboard.press('Enter')
    await page.locator('#palette').waitFor({ state: 'hidden' })
    expect(await page.locator('#conversation-title').textContent()).toContain('Back up')
})
