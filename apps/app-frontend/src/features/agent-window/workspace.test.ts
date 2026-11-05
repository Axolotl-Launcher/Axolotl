import { readdir, readFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'

import { type Browser, chromium, type Page } from 'playwright'
import { afterAll, afterEach, beforeAll, beforeEach, expect, it } from 'vitest'

let server: Server
let browser: Browser
let page: Page
let baseUrl: string
const STORE = 'axolotl.agent-window.store.v2'
const START = Date.parse('2026-11-02T12:00:00Z')
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
    // A clock that is only installed still follows the wall clock, so a slow
    // machine would move the preview's timers between two steps of a test.
    // Paused, the page's time, timers and frames move only when a test runs
    // the clock. The pause lands a second after the install, which a running
    // clock cannot have passed yet.
    await page.clock.install({ time: START })
    await page.clock.pauseAt(START + 1_000)
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
                preview: true,
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

function isFocused(locator: ReturnType<Page['locator']>) {
    return locator.evaluate((node) => node === document.activeElement)
}

// Presses Tab (or Shift+Tab) until the locator has the focus, as a keyboard
// user would, and fails if it is never reached.
async function tabTo(locator: ReturnType<Page['locator']>, key = 'Tab', limit = 40) {
    for (let index = 0; index < limit && !(await isFocused(locator)); index++)
        await page.keyboard.press(key)
    expect(await isFocused(locator)).toBe(true)
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

it('leaves a plan not carried out when the run fails after proposing it', async () => {
    await page.addInitScript(() => {
        Reflect.get(window, 'AgentHost').demo = { fail: 'plan' }
    })
    await page.reload()
    await ready()
    await send('The game stutters')
    await untilSettled()
    const plan = page.locator('.plan-card[data-state="stopped"]')
    expect(await plan.count()).toBe(1)
    expect(await plan.textContent()).toContain(messages.planStopped)
    expect(await page.getByTestId('approve-plan').count()).toBe(0)
    expect(await page.locator('[data-action="approve"]:visible').count()).toBe(0)
    const error = page.locator('.result-card.is-error')
    expect(await error.textContent()).toContain('Lost the connection to the agent')

    // The failure happens once, so trying again proposes a plan that can be approved.
    await error.locator('[data-action="retry"]').click()
    await untilPlan()
    expect(await page.locator('.plan-card').getAttribute('data-state')).toBe('pending')
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
    expect(await page.locator('.user-bubble .file-name').textContent()).toBe('latest.log')
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

it('closes a picker on a second click of its button and switches between the two', async () => {
    const picker = page.locator('#picker')
    const instance = page.locator('#instance-button')
    const model = page.locator('#model-button')
    const state = async () => ({
        open: await picker.evaluate((node) => node.matches(':popover-open')),
        instance: await instance.getAttribute('aria-expanded'),
        model: await model.getAttribute('aria-expanded'),
    })
    await instance.click()
    expect(await state()).toEqual({ open: true, instance: 'true', model: 'false' })
    // The press itself moves the focus out of the picker, which closes it
    // before the click; the click must not open it again.
    await instance.click()
    expect(await state()).toEqual({ open: false, instance: 'false', model: 'false' })
    expect(await isFocused(instance)).toBe(true)
    await instance.click()
    expect(await state()).toEqual({ open: true, instance: 'true', model: 'false' })
    // The other button switches pickers.
    await model.click()
    expect(await state()).toEqual({ open: true, instance: 'false', model: 'true' })
    await model.click()
    expect(await state()).toEqual({ open: false, instance: 'false', model: 'false' })
    expect(await isFocused(model)).toBe(true)
    // From the keyboard the button opens it, and Escape closes it onto the button.
    await page.keyboard.press('Enter')
    expect(await state()).toEqual({ open: true, instance: 'false', model: 'true' })
    await page.keyboard.press('Escape')
    expect(await state()).toEqual({ open: false, instance: 'false', model: 'false' })
    await expect.poll(() => isFocused(model)).toBe(true)
    // Pressed and released elsewhere, a button does not remember the press.
    await instance.click()
    await instance.dispatchEvent('pointerdown')
    await page.keyboard.press('Escape')
    await expect.poll(() => isFocused(instance)).toBe(true)
    await page.keyboard.press('Enter')
    expect(await state()).toEqual({ open: true, instance: 'true', model: 'false' })
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
    // Gives anything the late result scheduled its timers and frames.
    await page.clock.runFor(100)
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
    // Finished step groups build their rows when opened.
    await page.locator('.activity-summary').first().click()
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

it('streams an answer into the same markup a full render builds, without rebuilding its log', async () => {
    const source = [
        '# Crash',
        'The game **stopped** at `render` — see the [wiki](https://example.com/w).',
        '',
        '3. Close the game',
        '4. Open *the* log',
        '',
        '> A quote',
        '> that goes on',
        '',
        '```',
        'java.lang.IllegalStateException: Buffer not bound',
        '\tat net.minecraft.client.Render.draw(Render.java:200)',
        '```',
        '',
        '```log',
        ...Array.from({ length: 40 }, (_, index) =>
            index % 3
                ? `[12:00:${index}] [Render thread/INFO]: line ${index}`
                : `[WARN] slow ${index}`,
        ),
        '```',
        '---',
        '- one',
        '- two **bold**',
    ].join('\n')
    // A string, so the test's own bundler leaves the page's import alone.
    const dom = await page.evaluateHandle<{
        renderMarkdown: (container: Element, source: string) => void
    }>("import('/dom.js')")
    const report = await page.evaluate(
        ({ dom: { renderMarkdown }, text }) => {
            const streamed = document.createElement('div')
            const labelled = () =>
                [...streamed.querySelectorAll('.code-block')].find(
                    (block) => block.querySelector('.code-head span')?.textContent === 'log',
                )
            const mismatches: number[] = []
            let firstLine: Element | null | undefined = null
            for (let at = 0, step = 0; at < text.length; step++) {
                at = Math.min(text.length, at + 1 + (step % 9))
                renderMarkdown(streamed, text.slice(0, at))
                const fresh = document.createElement('div')
                renderMarkdown(fresh, text.slice(0, at))
                if (streamed.innerHTML !== fresh.innerHTML) mismatches.push(at)
                firstLine ||= labelled()?.querySelector('.log-line')
            }
            return {
                mismatches,
                blocks: streamed.children.length,
                lines: labelled()?.querySelectorAll('.log-line').length,
                // The log's first line kept its node while the others streamed in.
                keptFirstLine: !!firstLine && labelled()?.querySelector('.log-line') === firstLine,
            }
        },
        { dom, text: source },
    )
    expect(report).toEqual({ mismatches: [], blocks: 8, lines: 40, keptFirstLine: true })
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

it('marks answers and walks requests from the message navigator', async () => {
    await send('Hello there')
    await untilSettled()
    await send('What can you do?')
    await untilSettled()
    const ticks = page.locator('.rail-item')
    expect(await ticks.count()).toBe(2)
    // The ticks are a single tab stop.
    expect(await page.locator('.rail-item[tabindex="0"]').count()).toBe(1)

    const mark = page.locator('.turn').first().locator('.mark-turn')
    await mark.click()
    expect(await mark.getAttribute('aria-pressed')).toBe('true')
    expect(await ticks.first().getAttribute('data-marked')).toBe('')

    // Hovering a tick previews its request; clicking it scrolls there.
    await ticks.first().hover()
    await page.clock.runFor(50)
    const peek = page.locator('#rail-peek')
    expect(await peek.getAttribute('class')).toContain('is-shown')
    expect(await peek.textContent()).toContain('Hello there')
    expect(await peek.textContent()).toContain(messages.marked)
    await ticks.first().click()

    // Arrow keys walk the ticks and the card follows the focus.
    await ticks.first().focus()
    await page.keyboard.press('ArrowDown')
    expect(await ticks.nth(1).evaluate((node) => node === document.activeElement)).toBe(true)
    expect(await peek.textContent()).toContain('What can you do?')

    await page.reload()
    await ready()
    expect(
        await page.locator('.turn').first().locator('.mark-turn').getAttribute('aria-pressed'),
    ).toBe('true')
})

// Opens a stored task of `count` finished turns, each a short answer.
async function openLongTask(count: number) {
    await page.addInitScript(
        ({ key, total }) => {
            const turns = Array.from({ length: total }, (_, index) => ({
                id: `turn-${index}`,
                text: `Question ${index + 1}`,
                run: {
                    status: 'done',
                    items: [{ id: `answer-${index}`, type: 'text', text: `Answer ${index + 1}` }],
                },
            }))
            localStorage.setItem(
                key,
                JSON.stringify({
                    version: 2,
                    activeId: 'long',
                    threads: [{ id: 'long', title: 'Long task', turns }],
                }),
            )
        },
        { key: STORE, total: count },
    )
    await page.reload()
    await ready()
    // The view moves to the latest turn in the next frame.
    await page.clock.runFor(100)
}

const turnIds = () =>
    page
        .locator('#messages > .turn')
        .evaluateAll((nodes) => nodes.map((node) => (node as HTMLElement).dataset.turn))

it('opens a long task at its latest turns and builds older ones as the reader scrolls up', async () => {
    await openLongTask(120)
    const ids = await turnIds()
    expect(ids).toHaveLength(40)
    expect(ids[0]).toBe('turn-80')
    expect(ids.at(-1)).toBe('turn-119')
    const scroll = page.locator('#message-scroll')
    expect(
        await scroll.evaluate((node) => node.scrollHeight - node.scrollTop - node.clientHeight),
    ).toBeLessThan(2)
    // The navigator still lists every turn.
    expect(await page.locator('.rail-item').count()).toBe(120)

    const top = () =>
        page.evaluate(
            () => document.querySelector('[data-turn="turn-80"]')!.getBoundingClientRect().top,
        )
    const before = await scroll.evaluate((node) => {
        node.scrollTop = 0
        return document.querySelector('[data-turn="turn-80"]')!.getBoundingClientRect().top
    })
    await expect.poll(() => page.locator('#messages > .turn').count()).toBe(80)
    expect((await turnIds())[0]).toBe('turn-40')
    expect(Math.abs((await top()) - before)).toBeLessThan(3)
    // Once their size is known the new turns may skip layout again, without moving.
    await expect
        .poll(async () => {
            await page.clock.runFor(50)
            return page.locator('.turn.is-placing').count()
        })
        .toBe(0)
    expect(Math.abs((await top()) - before)).toBeLessThan(3)
})

it('builds down to an older turn picked in the navigator and shows it', async () => {
    await openLongTask(120)
    await page.locator('.rail-item').first().click()
    expect(await page.locator('#messages > .turn').count()).toBe(120)
    await page.clock.runFor(100)
    const first = page.locator('[data-turn="turn-0"]')
    const view = await page.locator('#message-scroll').boundingBox()
    const turn = await first.boundingBox()
    expect(turn!.y).toBeGreaterThanOrEqual(view!.y)
    expect(turn!.y).toBeLessThan(view!.y + view!.height / 2)
    expect(await page.locator('.rail-item[data-current]').getAttribute('data-jump')).toBe('turn-0')
})

it('adds only the new turn at the bottom of a long task', async () => {
    await openLongTask(120)
    await send('Hello there')
    await page.clock.runFor(100)
    const ids = await turnIds()
    expect(ids).toHaveLength(41)
    expect(ids.slice(0, 40)).toEqual(Array.from({ length: 40 }, (_, index) => `turn-${80 + index}`))
    expect(
        await page.locator('#messages > .turn').last().locator('.user-bubble').textContent(),
    ).toBe('Hello there')
    expect(await page.locator('.rail-item').count()).toBe(121)
})

it('attaches a long pasted crash report, can put it back as text and sends it alone', async () => {
    const report = [
        '---- Minecraft Crash Report ----',
        ...Array.from(
            { length: 30 },
            (_, index) => `\tat net.minecraft.Example.call(Example.java:${index})`,
        ),
    ].join('\n')
    const paste = () =>
        page.evaluate((value) => {
            const data = new DataTransfer()
            data.setData('text/plain', value)
            document.getElementById('message-input')!.dispatchEvent(
                new ClipboardEvent('paste', {
                    clipboardData: data,
                    bubbles: true,
                    cancelable: true,
                }),
            )
        }, report)
    await page.locator('#message-input').focus()
    await paste()
    const chip = page.locator('#attachment-list .file-chip')
    expect(await chip.getAttribute('data-kind')).toBe('crash')
    expect(await chip.textContent()).toContain(messages.pasteCrashName)
    expect(await chip.textContent()).toContain('31')
    expect(await page.locator('#message-input').inputValue()).toBe('')

    await page.locator('.toast .text-button').click()
    expect(await chip.count()).toBe(0)
    expect(await page.locator('#message-input').inputValue()).toBe(report)

    await page.locator('#message-input').fill('')
    await paste()
    expect(await page.locator('#send-button').isEnabled()).toBe(true)
    await page.locator('#send-button').click()
    await untilPlan()
    expect(await page.locator('.user-bubble .file-chip').textContent()).toContain(
        messages.pasteCrashName,
    )
    expect(await page.locator('#conversation-title').textContent()).not.toBe('')
})

it('pins a task from its context menu and names icon buttons with their shortcut', async () => {
    await send('Hello there')
    await untilSettled()
    await page.getByTestId('new-chat').click()
    await send('What can you do?')
    await untilSettled()
    const names = () => page.locator('.thread-name').allTextContents()
    const [newest, older] = await names()

    await page.locator('.thread-row').nth(1).click({ button: 'right' })
    await page.locator('[data-menu-pin]').click()
    expect(await page.locator('.list-group').first().textContent()).toBe(messages.pinned)
    expect(await names()).toEqual([older, newest])

    await page.locator('#sidebar-toggle').hover()
    await page.clock.runFor(600)
    expect(await page.locator('#tip').textContent()).toBe(`${messages.toggleSidebar}CtrlB`)
    await page.mouse.move(600, 600)
    expect(await page.locator('#tip').getAttribute('class')).not.toContain('is-shown')
})

it('keeps an unsent new task draft out of the list, and New task returns to it', async () => {
    await send('Hello there')
    await untilSettled()
    await page.getByTestId('new-chat').click()
    await page.locator('#message-input').fill('Back up my world later')
    expect(await page.locator('.thread-name').count()).toBe(1)

    await page.locator('.thread-link').first().click()
    await page.clock.runFor(400)
    expect(await page.locator('.new-task').getAttribute('data-draft')).toBe(messages.draftShort)
    await page.getByTestId('new-chat').click()
    await page.clock.runFor(400)
    expect(await page.locator('#message-input').inputValue()).toBe('Back up my world later')
    expect(await page.locator('.new-task').getAttribute('data-draft')).toBeNull()
    expect(await page.locator('.thread-name').count()).toBe(1)
})

it('pins a task dragged to the top and feeds tasks to the axolotl, which asks about backups', async () => {
    await send('My game crashes on launch')
    await (await untilPlan()).click()
    await untilSettled()
    await page.getByTestId('new-chat').click()
    await send('Hello there')
    await untilSettled()
    const names = () => page.locator('.thread-name').allTextContents()
    const [newest, crash] = await names()
    const carry = async (row: number, x: number, y: number) => {
        const box = (await page.locator('.thread-link').nth(row).boundingBox())!
        await page.mouse.move(box.x + 40, box.y + box.height / 2)
        await page.mouse.down()
        await page.mouse.move(x, y, { steps: 4 })
        await page.mouse.up()
        await page.clock.runFor(50)
    }

    // Dropped at the top, the older task is pinned there and is not opened.
    const top = (await page.locator('.thread-link').first().boundingBox())!
    await carry(1, top.x + 40, top.y + 2)
    expect(await page.locator('.list-group').first().textContent()).toBe(messages.pinned)
    expect(await names()).toEqual([crash, newest])
    expect(await page.locator('.thread-link[aria-current="page"] .thread-name').textContent()).toBe(
        newest,
    )

    // Fed to the axolotl, the crash fix asks about the version its rollback kept.
    const main = (await page.locator('#conversation').boundingBox())!
    const middle = [main.x + main.width / 2, main.y + main.height / 2] as const
    await carry(0, ...middle)
    expect(await page.locator('#feed-zone').getAttribute('data-state')).toBe('asking')
    expect(await page.locator('.feed-leftovers li').textContent()).toContain('Entity Culling 1.7.4')
    expect(await names()).toEqual([newest])
    await page.keyboard.press('Escape')
    await page.clock.runFor(50)
    expect(await page.locator('#feed-zone').isHidden()).toBe(true)
    await page.locator('.toast .text-button').last().click()
    expect(await names()).toEqual([crash, newest])

    // A task that left nothing behind goes without a question.
    await carry(1, ...middle)
    expect(await page.locator('.feed-leftovers li').count()).toBe(0)
    expect(await page.locator('#feed-zone').isHidden()).toBe(true)
    expect(await names()).toEqual([crash])
})

it('puts what the open task offers first in the command menu', async () => {
    await send('Hello there')
    await untilSettled()
    const title = await page.locator('#conversation-title').textContent()
    await page.keyboard.press('Control+k')
    await page.locator('#palette').waitFor()
    expect(await page.locator('.palette-group').first().textContent()).toBe(title)
    expect(await page.locator('.palette-row').first().textContent()).toContain(messages.pinChat)
    await page.keyboard.press('Enter')
    expect(await page.locator('.list-group').first().textContent()).toBe(messages.pinned)
})

it('keeps step groups open or folded by the chosen detail and remembers standing instructions', async () => {
    await send('My game crashes on launch')
    await (await untilPlan()).click()
    await untilSettled()
    const groups = page.locator('.activity')
    expect(
        await groups.evaluateAll((nodes) => nodes.some((node) => node.hasAttribute('data-open'))),
    ).toBe(false)

    await page.locator('#settings-button').click()
    await page.locator('#detail-choice [data-detail="detailed"]').click()
    expect(
        await page.locator('#detail-choice [data-detail="detailed"]').getAttribute('aria-checked'),
    ).toBe('true')
    expect(
        await groups.evaluateAll((nodes) => nodes.every((node) => node.hasAttribute('data-open'))),
    ).toBe(true)
    // Detailed names every step's tool, not only the one under the pointer.
    expect(
        await page
            .locator('.step-tool:not([hidden])')
            .evaluateAll((nodes) => nodes.every((node) => getComputedStyle(node).opacity === '1')),
    ).toBe(true)
    await page.locator('#instructions-input').fill('Answer in English and prefer Fabric.')
    await page.clock.runFor(1_000)

    await page.reload()
    await ready()
    expect(await page.locator('html').getAttribute('data-steps')).toBe('detailed')
    await page.locator('#settings-button').click()
    expect(await page.locator('#instructions-input').inputValue()).toBe(
        'Answer in English and prefer Fabric.',
    )
})

it('folds the earlier steps of a long run behind one row', async () => {
    await page.evaluate((key) => {
        const steps = Array.from({ length: 12 }, (_, index) => ({
            id: `s${index}`,
            title: `Step ${index + 1}`,
            tool: 'instance_get',
            status: 'done',
        }))
        localStorage.setItem(
            key,
            JSON.stringify({
                version: 2,
                activeId: 't',
                threads: [
                    {
                        id: 't',
                        title: 'Long check',
                        turns: [
                            {
                                id: 'u',
                                text: 'Check everything',
                                run: {
                                    status: 'done',
                                    startedAt: 1,
                                    endedAt: 2,
                                    items: [
                                        {
                                            id: 'a',
                                            type: 'activity',
                                            steps,
                                            startedAt: 1,
                                            endedAt: 2,
                                        },
                                    ],
                                },
                            },
                        ],
                    },
                ],
            }),
        )
    }, STORE)
    await page.reload()
    await ready()
    await page.locator('.activity-summary').click()
    expect(await page.locator('.step:not([hidden])').count()).toBe(6)
    expect(await page.locator('.step-more').textContent()).toBe('Show 6 earlier steps')
    await page.locator('.step-more button').click()
    expect(await page.locator('.step:not([hidden])').count()).toBe(12)
    expect(await page.locator('.step-more').count()).toBe(0)
})

it('explains every tool in plain words on its step and in the activity log', async () => {
    await send('My game crashes on launch')
    await (await untilPlan()).click()
    await untilSettled()
    // Finished groups fold away and build their rows once opened.
    for (const summary of await page.locator('.activity-summary').all()) await summary.click()
    // Until the catalog is regenerated the explanations are their keys, so this
    // checks shape: each tool has one, it is not the bare name, and no two
    // tools share one.
    const steps = await page.locator('.activity .step').evaluateAll((rows) =>
        rows.map((row) => {
            const chip = row.querySelector<HTMLElement>('.step-tool')!
            const described = row.getAttribute('aria-describedby')
            return {
                tool: chip.textContent,
                tip: chip.dataset.tip,
                description: described && document.getElementById(described)?.textContent,
                tabStop: chip.tabIndex >= 0,
            }
        }),
    )
    expect(steps.map((step) => step.tool)).toContain('resource_history_rollback')
    for (const step of steps) {
        expect(step.tip, step.tool).toMatch(/\S/)
        expect(step.tip).not.toBe(step.tool)
        // Screen readers get the same words, without a tab stop per step.
        expect(step.description).toBe(step.tip)
        expect(step.tabStop).toBe(false)
    }
    const tips = new Map(steps.map((step) => [step.tool, step.tip]))
    expect(new Set(tips.values()).size).toBe(tips.size)

    // A scroll hides the tooltip, so the chip is scrolled into view, and that
    // scroll has landed, before the pointer rests on it.
    const chip = page.locator('.step-tool', { hasText: 'logs_read' })
    await chip.evaluate(async (node) => {
        const scroller = document.getElementById('message-scroll')!
        const landed = new Promise((resolve) =>
            scroller.addEventListener('scroll', resolve, { once: true }),
        )
        const before = scroller.scrollTop
        node.scrollIntoView({ block: 'center' })
        if (scroller.scrollTop !== before) await landed
    })
    await chip.hover()
    await page.clock.runFor(600)
    expect(await page.locator('#tip.is-shown').textContent()).toBe(tips.get('logs_read'))

    await page.locator('#task-toggle').click()
    const log = await page.locator('.log-list li').evaluateAll((entries) =>
        entries.map((entry) => {
            const described = entry.getAttribute('aria-describedby')
            return {
                tool: entry.querySelector('code')?.textContent,
                tip: entry.querySelector('code')?.dataset.tip,
                description: described && document.getElementById(described)?.textContent,
            }
        }),
    )
    expect(log.length).toBe(steps.length)
    for (const entry of log) {
        expect(entry.tip, entry.tool).toBe(tips.get(entry.tool!))
        expect(entry.description).toBe(entry.tip)
    }
})

it('offers follow-ups under the latest result and sends the one picked', async () => {
    await send('My game crashes on launch')
    await (await untilPlan()).click()
    await untilSettled()
    const chips = page.locator('.result-card .next-chip')
    expect(await chips.allTextContents()).toEqual(['Launch the game to check', 'Undo this change'])
    await chips.nth(1).click()
    await page.locator('.turn').nth(1).waitFor()
    expect(await page.locator('.turn').nth(1).locator('.user-bubble').textContent()).toBe(
        'Undo this change',
    )
    // Only the latest result offers them.
    expect(await page.locator('.next-chip').count()).toBe(0)
})

it('marks a task that finishes in the background as unread until it is opened', async () => {
    await send('Hello there')
    await page.getByTestId('new-chat').click()
    for (
        let index = 0;
        index < 20 && !(await page.locator('.thread-row.is-unread').count());
        index++
    )
        await page.clock.runFor(500)
    const row = page.locator('.thread-row.is-unread')
    expect(await row.count()).toBe(1)
    expect(await row.locator('.thread-dot').count()).toBe(1)
    await row.locator('.thread-link').click()
    expect(await page.locator('.thread-row.is-unread').count()).toBe(0)
})

it('previews a pasted log with its errors, warnings and stack frames told apart', async () => {
    const log = [
        '[12:00:01] [main/INFO]: Loading Minecraft 1.21.1',
        '[12:00:02] [main/WARN]: Mod tweaks uses an old format',
        '[12:00:03] [Render thread/ERROR]: Mixin apply failed',
        'java.lang.RuntimeException: boom',
        ...Array.from(
            { length: 20 },
            (_, index) => `\tat net.minecraft.Example.call(Example.java:${index})`,
        ),
    ].join('\n')
    await page.locator('#message-input').focus()
    await page.evaluate((value) => {
        const data = new DataTransfer()
        data.setData('text/plain', value)
        document.getElementById('message-input')!.dispatchEvent(
            new ClipboardEvent('paste', {
                clipboardData: data,
                bubbles: true,
                cancelable: true,
            }),
        )
    }, log)
    expect(await page.locator('#attachment-list .file-chip').getAttribute('data-kind')).toBe('log')
    await page.locator('#attachment-list .file-open').click()
    const peek = page.locator('#file-peek')
    await peek.waitFor()
    expect(await peek.locator('.log-line.is-error').count()).toBe(2)
    expect(await peek.locator('.log-line.is-warn').count()).toBe(1)
    expect(await peek.locator('.log-line.is-trace').count()).toBe(20)
    expect(await peek.locator('code').textContent()).toBe(log)
    expect(await peek.textContent()).not.toContain('null')
})

it('lists what the task changed and asks the agent to undo it', async () => {
    await send('My game crashes on launch')
    await (await untilPlan()).click()
    await untilSettled()
    const pill = page.locator('#changes-pill')
    expect(await pill.isVisible()).toBe(true)
    expect(await pill.locator('.changes-count').textContent()).toBe('1')
    await pill.click()
    const list = page.locator('#changes-peek')
    await list.waitFor()
    expect(await list.locator('li').count()).toBe(1)
    expect(await list.locator('li').textContent()).toContain('Entity Culling')
    expect(await list.locator('.changes-diff').textContent()).toBe('1.7.4 → 1.7.2')
    await list.locator('[data-followup]').click()
    await page.locator('.turn').nth(1).waitFor()
    expect(await page.locator('.turn').nth(1).locator('.user-bubble').textContent()).toBe(
        messages.undoAllPrompt,
    )
    expect(await pill.isHidden()).toBe(true)
})

it('picks the task instance with an @ mention', async () => {
    await useContext({
        ai_enabled: false,
        providers: [],
        instances: [
            { id: 'mist', name: 'Morning Mist', game_version: '1.21.1', loader: 'fabric' },
            { id: 'stone', name: 'Stone Age', game_version: '1.20.1', loader: 'forge' },
        ],
    })
    await page.locator('#message-input').fill('fix the lag in @mo')
    const menu = page.locator('#slash-menu')
    expect(await menu.isVisible()).toBe(true)
    expect(await menu.locator('.slash-name').allTextContents()).toEqual(['@Morning Mist'])
    await page.locator('#message-input').press('Enter')
    expect(await page.locator('#message-input').inputValue()).toBe('fix the lag in ')
    expect(await page.locator('#instance-button').textContent()).toContain('Morning Mist')
    expect(await menu.isHidden()).toBe(true)
    // An @ inside a word does not open it.
    await page.locator('#message-input').fill('mail me@home')
    expect(await menu.isHidden()).toBe(true)
})

it('marks search matches and shows the message that matched', async () => {
    await send('My game crashes on launch')
    await untilPlan()
    await page.locator('#conversation-search').fill('game')
    const row = page.locator('.thread-row')
    expect(await row.count()).toBe(1)
    expect(await row.locator('.thread-snippet').textContent()).toBe('My game crashes on launch')
    expect(await row.locator('.thread-snippet mark').textContent()).toBe('game')
    await page.locator('#conversation-search').fill('crash')
    expect(await row.locator('.thread-name mark').textContent()).toBe('crash')
    expect(await row.locator('.thread-snippet').count()).toBe(0)
})

it('steps through tasks with Ctrl [ and Ctrl ]', async () => {
    await send('Hello there')
    await untilSettled()
    await page.getByTestId('new-chat').click()
    await send('What can you do?')
    await untilSettled()
    const [newest, older] = await page.locator('.thread-name').allTextContents()
    expect(await page.locator('#conversation-title').textContent()).toBe(newest)
    await page.keyboard.press('Control+]')
    expect(await page.locator('#conversation-title').textContent()).toBe(older)
    await page.keyboard.press('Control+[')
    expect(await page.locator('#conversation-title').textContent()).toBe(newest)
})

it('switches between tasks in place, without fading the window or replaying their history', async () => {
    await send('My game crashes on launch')
    await (await untilPlan()).click()
    await untilSettled()
    await page.getByTestId('new-chat').click()
    await send('Hello there')
    await untilSettled()
    const [newest, older] = await page
        .locator('.thread-link')
        .evaluateAll((links) => links.map((link) => (link as HTMLElement).dataset.thread))
    // With motion on, every view transition is counted.
    await page.emulateMedia({ reducedMotion: 'no-preference' })
    await page.evaluate(() => {
        const start = document.startViewTransition.bind(document)
        Reflect.set(window, 'transitions', 0)
        document.startViewTransition = (update) => {
            Reflect.set(window, 'transitions', Reflect.get(window, 'transitions') + 1)
            return start(update)
        }
    })
    // Entrances CSS started under a node that are still to finish; the
    // axolotl's own moods and endless loops do not count.
    const entrances = (selector: string) =>
        page
            .locator(selector)
            .evaluate(
                (node) =>
                    node
                        .getAnimations({ subtree: true })
                        .filter(
                            (animation) =>
                                animation instanceof CSSAnimation &&
                                animation.playState === 'running' &&
                                animation.effect?.getComputedTiming().endTime !== Infinity &&
                                !(animation.effect as KeyframeEffect).target?.closest('.axo'),
                        ).length,
            )

    // Between two conversations the new one is in place at once: no
    // transition of the whole window, and its plan, steps and result show
    // as they are rather than rising, drawing and popping in again.
    await page.locator(`.thread-link[data-thread="${older}"]`).click()
    expect(await page.locator('#conversation-title').textContent()).toBe(
        await page.locator(`#thread-name-${older}`).textContent(),
    )
    expect(await page.locator('.plan-card .change').count()).toBeGreaterThan(0)
    expect(await entrances('#messages')).toBe(0)
    expect(await entrances('#conversation')).toBe(0)
    expect(await page.evaluate(() => Reflect.get(window, 'transitions'))).toBe(0)
    await page.locator(`.thread-link[data-thread="${newest}"]`).click()
    expect(await entrances('#messages')).toBe(0)
    expect(await page.evaluate(() => Reflect.get(window, 'transitions'))).toBe(0)

    // To or from the new task page the composer changes place, so that one
    // still morphs and the composer glides.
    await page.getByTestId('new-chat').click()
    await page.locator('#composer-home #composer').waitFor()
    await page.locator('#message-input').fill('A draft')
    const before = await page.evaluate(() => Reflect.get(window, 'transitions'))
    await page.locator(`.thread-link[data-thread="${older}"]`).click()
    await page.locator('#composer-dock #composer').waitFor()
    expect(await page.evaluate(() => Reflect.get(window, 'transitions'))).toBe(before + 1)
})

it('closes the status line smoothly after its run, and opens it again for a new run', async () => {
    await page.emulateMedia({ reducedMotion: 'no-preference' })
    await send('Hello there')
    const status = page.locator('#messages > .turn:last-child .run-status')
    const hidden = () => status.evaluate((node) => (node as HTMLElement).hidden)
    const untilLeaving = async () => {
        for (let index = 0; index < 40 && !(await status.getAttribute('data-leaving')); index++)
            await page.clock.runFor(250)
        expect(await status.getAttribute('data-leaving')).toBeTruthy()
    }
    // The axolotl reacts first, then the line closes over a few frames
    // rather than vanishing, and only then is hidden.
    await untilLeaving()
    await page.clock.runFor(1_400)
    expect(await status.getAttribute('data-moving')).toBe('out')
    expect(await hidden()).toBe(false)
    await expect.poll(hidden).toBe(true)
    expect(await status.getAttribute('data-moving')).toBeNull()

    // Answered again, the same turn opens its line in place.
    const again = page.locator('#messages > .turn:last-child [data-action="retry"]')
    await again.evaluate((node) => (node as HTMLElement).click())
    expect(await hidden()).toBe(false)
    expect(await status.getAttribute('data-moving')).toBe('in')
    await expect.poll(() => status.getAttribute('data-moving')).toBeNull()

    // A run that starts while the line is closing takes it back open.
    await untilLeaving()
    await page.clock.runFor(1_400)
    expect(await status.getAttribute('data-moving')).toBe('out')
    await again.evaluate((node) => (node as HTMLElement).click())
    await expect.poll(() => status.getAttribute('data-moving')).toBeNull()
    expect(await hidden()).toBe(false)
    expect(await status.evaluate((node) => node.getBoundingClientRect().height)).toBeGreaterThan(24)
})

it('renames a task in place by double-clicking its title', async () => {
    await send('Hello there')
    await untilSettled()
    const title = page.locator('#conversation-title')
    const before = await title.textContent()
    await title.dblclick()
    const input = page.locator('.title-input')
    expect(await input.inputValue()).toBe(before)
    await input.fill('Ignored')
    await input.press('Escape')
    expect(await title.textContent()).toBe(before)
    await title.dblclick()
    await page.locator('.title-input').fill('My first task')
    await page.locator('.title-input').press('Enter')
    expect(await title.textContent()).toBe('My first task')
    expect(await page.locator('.thread-name').first().textContent()).toBe('My first task')
})

it('asks the latest answer again, but never one that wrote changes', async () => {
    await send('Hello there')
    await untilSettled()
    const again = page.locator(`.turn-actions [aria-label="${messages.answerAgain}"]`)
    expect(await again.count()).toBe(1)
    await again.click()
    await untilSettled()
    expect(await page.locator('.turn').count()).toBe(1)
    expect(await page.locator('.agent-text').count()).toBeGreaterThan(0)

    await send('My game crashes on launch')
    await (await untilPlan()).click()
    await untilSettled()
    expect(await again.count()).toBe(0)
})

it('runs a whole task from the keyboard: a starting point, a plan with one change left out, approval', async () => {
    // The starting points follow the composer in the tab order.
    await page.locator('#message-input').focus()
    const pack = page.locator('[data-prompt="promptPack"]')
    await tabTo(pack)
    await page.keyboard.press('Enter')
    const input = page.locator('#message-input')
    expect(await isFocused(input)).toBe(true)
    expect(await input.inputValue()).toBe(messages.promptPack)
    await page.keyboard.press('Enter')
    await untilPlan()

    // Shift+Tab walks back from the composer to the plan's ticks; Space unticks.
    const lithium = page.locator(
        `.change-toggle[aria-label="${messages.includeChange.replace('{name}', 'Lithium')}"]`,
    )
    await tabTo(lithium, 'Shift+Tab')
    await page.keyboard.press('Space')
    expect(await lithium.getAttribute('aria-checked')).toBe('false')
    expect(await isFocused(lithium)).toBe(true)
    expect(await page.locator('.plan-skipping').textContent()).toContain('Lithium')
    const included = await page.locator('.change-toggle[aria-checked="true"]').count()

    // Approving redraws the plan's buttons; the focus waits in the composer.
    await tabTo(page.getByTestId('approve-plan'))
    await page.keyboard.press('Enter')
    expect(await isFocused(input)).toBe(true)
    await untilSettled()
    expect(await page.locator('.plan-card').getAttribute('data-state')).toBe('applied')
    expect(await page.locator('.plan-state.is-applied').textContent()).toBe(
        messages.planAppliedPartial.replace('{done}', String(included)).replace('{skipped}', '1'),
    )
    // A decided plan's ticks leave the tab order and read as unavailable.
    expect(await lithium.getAttribute('tabindex')).toBe('-1')
    expect(await lithium.getAttribute('aria-disabled')).toBe('true')
})

it('walks the steps-shown choice with arrow keys as a single tab stop', async () => {
    await page.locator('#settings-button').click()
    const checked = page.locator('#detail-choice [aria-checked="true"]')
    const focusedChoice = () =>
        page.evaluate(() => (document.activeElement as HTMLElement).dataset.detail)
    expect(await page.locator('#detail-choice [tabindex="0"]').count()).toBe(1)
    await checked.focus()
    expect(await focusedChoice()).toBe('balanced')

    await page.keyboard.press('ArrowRight')
    expect(await focusedChoice()).toBe('detailed')
    expect(await checked.getAttribute('data-detail')).toBe('detailed')
    expect(await page.locator('html').getAttribute('data-steps')).toBe('detailed')
    // The keys wrap round and jump to the ends.
    await page.keyboard.press('ArrowDown')
    expect(await focusedChoice()).toBe('concise')
    await page.keyboard.press('ArrowLeft')
    expect(await focusedChoice()).toBe('detailed')
    await page.keyboard.press('Home')
    expect(await focusedChoice()).toBe('concise')
    await page.keyboard.press('End')
    expect(await focusedChoice()).toBe('detailed')
    expect(await page.locator('#detail-choice [tabindex="0"]').getAttribute('data-detail')).toBe(
        'detailed',
    )

    // Tab leaves the group in one step, and Shift+Tab comes back to the choice.
    await page.keyboard.press('Tab')
    expect(await isFocused(page.locator('#instructions-input'))).toBe(true)
    await page.keyboard.press('Shift+Tab')
    expect(await focusedChoice()).toBe('detailed')
})

it('previews each steps-shown choice under it, in one picture that only changes level', async () => {
    await page.locator('#settings-button').click()
    const preview = page.locator('#detail-preview')
    // Decorative: the radio group carries the choice for screen readers.
    expect(await preview.getAttribute('aria-hidden')).toBe('true')
    expect(await preview.getAttribute('data-level')).toBe('balanced')
    expect(await preview.locator('.dp-tool').allTextContents()).toEqual([
        'java_list',
        'system_get_memory',
        'instance_list',
        'instance_get',
        'logs_read',
    ])
    const well = await preview.locator('.dp-well').elementHandle()
    // What shows at each level: which groups are open (done, working), whether
    // tool names show, which of the working group's two labels shows, and the
    // one caption that does.
    const shown = () =>
        preview.evaluate((node) => {
            const visible = (element: Element) => getComputedStyle(element).opacity === '1'
            const groups = [...node.querySelectorAll('.dp-group')]
            return {
                open: groups.map(
                    (group) =>
                        getComputedStyle(group.querySelector('.dp-body')!).gridTemplateRows !==
                        '0px',
                ),
                tools: [...node.querySelectorAll('.dp-tool')].every(visible),
                label: [...groups[1].querySelectorAll('.dp-label > span')].findIndex(visible),
                captions: [...node.querySelectorAll('.dp-caption > span')]
                    .filter(visible)
                    .map((caption) => (caption as HTMLElement).dataset.for),
            }
        })
    expect(await shown()).toEqual({
        open: [false, true],
        tools: false,
        label: 1,
        captions: ['balanced'],
    })
    await page.locator('#detail-choice [data-detail="concise"]').click()
    expect(await preview.getAttribute('data-level')).toBe('concise')
    expect(await shown()).toEqual({
        open: [false, false],
        tools: false,
        label: 0,
        captions: ['concise'],
    })
    await page.locator('#detail-choice [data-detail="detailed"]').click()
    expect(await shown()).toEqual({
        open: [true, true],
        tools: true,
        label: 1,
        captions: ['detailed'],
    })
    // The picture is built once, so a new level can move rather than redraw.
    expect(await well!.evaluate((node) => node.isConnected)).toBe(true)
})

it('gives the focus back to what opened a popover or menu, even after a redraw', async () => {
    await send('My game crashes on launch')
    await (await untilPlan()).click()
    await untilSettled()

    // The changes popover takes the focus and returns it to the pill.
    const pill = page.locator('#changes-pill')
    const peek = page.locator('#changes-peek')
    await pill.focus()
    await page.keyboard.press('Enter')
    await peek.waitFor()
    expect(await pill.getAttribute('aria-expanded')).toBe('true')
    expect(await isFocused(peek)).toBe(true)
    await page.keyboard.press('Tab')
    expect(await isFocused(peek.locator('[data-followup]'))).toBe(true)
    await page.keyboard.press('Escape')
    await peek.waitFor({ state: 'hidden' })
    expect(await isFocused(pill)).toBe(true)
    expect(await pill.getAttribute('aria-expanded')).toBe('false')

    // Arrow keys walk the task menu and Escape closes it onto its button.
    const more = page.locator('#conversation-more')
    const focusedItem = () =>
        page.evaluate(() => Object.keys((document.activeElement as HTMLElement).dataset)[0])
    await more.focus()
    await page.keyboard.press('Enter')
    expect(await more.getAttribute('aria-expanded')).toBe('true')
    expect(await focusedItem()).toBe('menuPin')
    await page.keyboard.press('ArrowDown')
    expect(await focusedItem()).toBe('menuRename')
    await page.keyboard.press('End')
    expect(await focusedItem()).toBe('menuDelete')
    await page.keyboard.press('ArrowDown')
    expect(await focusedItem()).toBe('menuPin')
    await page.keyboard.press('Escape')
    expect(await isFocused(more)).toBe(true)
    expect(await more.getAttribute('aria-expanded')).toBe('false')
    // Tab leaves the menu and closes it on the way.
    await page.keyboard.press('Enter')
    await page.keyboard.press('Tab')
    expect(await page.locator('#menu').evaluate((node) => node.matches(':popover-open'))).toBe(
        false,
    )
    expect(await isFocused(page.locator('#task-toggle'))).toBe(true)

    // The task list redraws while a row's menu is open; Escape still lands on
    // that row's ⋯ button, now a new node.
    const rowMore = page.locator('.thread-more').first()
    await rowMore.focus()
    await page.keyboard.press('Enter')
    const original = await rowMore.elementHandle()
    await page.evaluate(() =>
        document.getElementById('conversation-search')!.dispatchEvent(new Event('input')),
    )
    expect(await original!.evaluate((node) => node.isConnected)).toBe(false)
    await page.keyboard.press('Escape')
    // The browser fires the popover's toggle event with the next frame.
    await expect.poll(() => isFocused(rowMore)).toBe(true)
})

it('renames and steps through tasks from the keyboard and keeps the focus on redrawn controls', async () => {
    await send('Hello there')
    await untilSettled()
    // Marking redraws the answer's actions; the focus stays on the toggle,
    // which keeps one name and reports its state.
    const mark = page.locator('.mark-turn').first()
    await mark.focus()
    await page.keyboard.press('Enter')
    expect(await mark.getAttribute('aria-pressed')).toBe('true')
    expect(await mark.getAttribute('aria-label')).toBe(messages.markTurn)
    expect(await isFocused(mark)).toBe(true)

    await page.keyboard.press('Control+n')
    await send('What can you do?')
    await untilSettled()
    const links = page.locator('.thread-link')
    const older = (await page.locator('.thread-name').allTextContents())[1]

    // F2 on the open task renames it in place and Enter puts the focus back.
    await links.first().focus()
    await page.keyboard.press('F2')
    const title = page.locator('.title-input')
    expect(await isFocused(title)).toBe(true)
    await page.keyboard.type('Named by keyboard')
    await page.keyboard.press('Enter')
    expect(await page.locator('#conversation-title').textContent()).toBe('Named by keyboard')
    expect(await isFocused(links.first())).toBe(true)

    // Arrow keys walk the list; F2 on another task asks for its name instead.
    await page.keyboard.press('ArrowDown')
    expect(await isFocused(links.nth(1))).toBe(true)
    expect(await links.nth(1).textContent()).toBe(older)
    await page.keyboard.press('F2')
    expect(await isFocused(page.locator('#rename-input'))).toBe(true)
    expect(await page.locator('#rename-input').inputValue()).toBe(older)
    await page.keyboard.press('Escape')
    expect(await isFocused(links.nth(1))).toBe(true)
    expect(await page.locator('#conversation-title').textContent()).toBe('Named by keyboard')

    // Hiding the task list with the focus in it hands the focus to its toggle.
    await page.keyboard.press('Control+b')
    expect(await page.locator('#sidebar').evaluate((node) => (node as HTMLElement).inert)).toBe(
        true,
    )
    expect(await isFocused(page.locator('#sidebar-toggle'))).toBe(true)
})

it('tells screen readers once per change what the agent is doing, never token by token', async () => {
    await page.evaluate(() => {
        const said: string[] = []
        Reflect.set(window, 'said', said)
        const node = document.getElementById('announcer')!
        new MutationObserver(() =>
            said.push([...node.children].map((line) => line.textContent).join(' | ')),
        ).observe(node, { childList: true })
    })
    const said = () => page.evaluate(() => Reflect.get(window, 'said') as string[])
    await send('My game crashes on launch')
    await untilPlan()
    // The agent's side of the transcript is not a live region of its own.
    expect(await page.locator('.turn .agent').getAttribute('aria-live')).toBe('off')
    expect(await page.locator('#messages').getAttribute('aria-live')).toBe('polite')
    const title = await page.locator('.plan-card h3').textContent()
    expect(await said()).toEqual([messages.thinking, `${messages.stateAwaiting} | ${title}`])

    await page.getByTestId('approve-plan').click()
    await untilSettled()
    const result = page.locator('.result-card .result-copy')
    expect(await said()).toEqual([
        messages.thinking,
        `${messages.stateAwaiting} | ${title}`,
        messages.applying,
        `${await result.locator('strong').textContent()} | ${await result.locator('p').first().textContent()}`,
    ])
    expect(await page.locator('#messages').getAttribute('aria-busy')).toBeNull()
})

const TOUR_TARGETS = ['#composer', '#composer .composer-toolbar', '#send-button']

const storedTourDone = () =>
    page.evaluate((key) => JSON.parse(localStorage.getItem(key) || '{}').tourDone, STORE)

// The card sits beside the stop's target, inside the window, never over it.
async function expectTourStop(step: number) {
    const card = page.locator('.tour-card')
    expect(await card.getAttribute('data-step')).toBe(String(step))
    const box = (await card.boundingBox())!
    const target = (await page.locator(TOUR_TARGETS[step]).boundingBox())!
    const viewport = page.viewportSize()!
    expect(box.y >= target.y + target.height || box.y + box.height <= target.y).toBe(true)
    expect(box.x).toBeGreaterThanOrEqual(0)
    expect(box.x + box.width).toBeLessThanOrEqual(viewport.width)
    expect(box.y + box.height).toBeLessThanOrEqual(viewport.height)
}

it('offers a first-run tour once the new task page is quiet and walks it by keyboard', async () => {
    const card = page.locator('.tour-card')
    await page.clock.runFor(1_500)
    await card.waitFor()
    // A named, non-modal dialog that holds the focus and lets the page be used.
    expect(await card.getAttribute('role')).toBe('dialog')
    expect(await card.getAttribute('aria-label')).toBeTruthy()
    expect(await card.getAttribute('aria-modal')).toBeNull()
    expect(await isFocused(card)).toBe(true)
    expect(
        await page.locator('.tour').evaluate((node) => getComputedStyle(node).pointerEvents),
    ).toBe('none')
    await expectTourStop(0)
    expect(await card.locator('[data-tour="back"]').isHidden()).toBe(true)

    // Arrow keys and Enter go on, and each new stop is read out.
    await page.keyboard.press('ArrowRight')
    await expectTourStop(1)
    const said = () => page.locator('#announcer p').allTextContents()
    expect(await said()).toContain(await card.locator('.tour-title').textContent())
    await page.keyboard.press('Enter')
    await expectTourStop(2)
    expect(await said()).toContain(await card.locator('.tour-title').textContent())
    expect(await card.locator('[data-tour="skip"]').isHidden()).toBe(true)
    await page.keyboard.press('ArrowRight')
    await expectTourStop(2)
    await page.keyboard.press('ArrowLeft')
    await expectTourStop(1)
    await page.keyboard.press('ArrowRight')

    // The last stop's button finishes, and the focus goes to the composer.
    await tabTo(card.locator('[data-tour="next"]'))
    await page.keyboard.press('Enter')
    expect(await page.locator('.tour').count()).toBe(0)
    expect(await isFocused(page.locator('#message-input'))).toBe(true)

    await page.reload()
    await ready()
    expect(await storedTourDone()).toBe(true)
    await page.clock.runFor(3_000)
    expect(await page.locator('.tour').count()).toBe(0)
})

it('skips the tour with Escape, replays it from settings and gives way to the page', async () => {
    const card = page.locator('.tour-card')
    await page.clock.runFor(1_500)
    await card.waitFor()
    await page.keyboard.press('ArrowRight')
    // Escape belongs to the tour: it skips, and the focus goes to the composer.
    await page.keyboard.press('Escape')
    expect(await page.locator('.tour').count()).toBe(0)
    expect(await isFocused(page.locator('#message-input'))).toBe(true)
    await page.clock.runFor(500)
    expect(await storedTourDone()).toBe(true)

    // Settings replays it from the first stop on a new task page, also on a
    // narrow window.
    await send('Hello there')
    await untilSettled()
    await page.setViewportSize({ width: 420, height: 760 })
    await page.locator('#settings-button').click()
    await page.locator('[data-action="replay-tour"]').click()
    await card.waitFor()
    expect(await page.locator('#settings-dialog').evaluate((node) => node.open)).toBe(false)
    expect(await page.locator('#composer-home #composer').count()).toBe(1)
    expect(await isFocused(card)).toBe(true)
    await expectTourStop(0)
    await card.locator('[data-tour="next"]').click()
    await card.locator('[data-tour="next"]').click()
    await expectTourStop(2)
    await card.locator('[data-tour="back"]').click()
    await expectTourStop(1)

    // Using the page instead ends it, and the click still lands.
    await page.locator('#message-input').click()
    expect(await page.locator('.tour').count()).toBe(0)
    await page.keyboard.type('Hi')
    expect(await page.locator('#message-input').inputValue()).toBe('Hi')
})

it('never offers the tour over typing or while the open task has turns', async () => {
    await page.locator('#message-input').fill('My game')
    await page.clock.runFor(3_000)
    expect(await page.locator('.tour').count()).toBe(0)

    // A first run that already sent something opens on that task.
    await send('Hello there')
    await untilSettled()
    await page.reload()
    await ready()
    expect(await storedTourDone()).not.toBe(true)
    await page.clock.runFor(3_000)
    expect(await page.locator('.tour').count()).toBe(0)
})

it('asks why a planned change is needed before deciding', async () => {
    await send('My game crashes on launch')
    await untilPlan()
    await page.locator('.plan-card .change-why').first().click()
    await page.locator('.turn').nth(1).waitFor()
    expect(await page.locator('.turn').nth(1).locator('.user-bubble').textContent()).toBe(
        messages.askWhy.replace('{name}', 'Entity Culling'),
    )
    await untilPlan()
    expect(
        await page.locator('.plan-card[data-state="superseded"] .change-why').first().isVisible(),
    ).toBe(false)
})

it('reads a dropped crash report so it previews and starts the crash task', async () => {
    const report = [
        '---- Minecraft Crash Report ----',
        'java.lang.RuntimeException: boom',
        '\tat a.b(C.java:1)',
    ].join('\n')
    await page.locator('#file-input').setInputFiles({
        name: 'crash-2026-11-01_client.txt',
        mimeType: 'text/plain',
        buffer: Buffer.from(report),
    })
    const chip = page.locator('#attachment-list .file-chip')
    await page.locator('#attachment-list .file-chip[data-kind="crash"]').waitFor()
    expect(await chip.locator('.file-name').textContent()).toBe('crash-2026-11-01_client.txt')
    expect(await chip.locator('.file-size').textContent()).toContain('3')
    await chip.locator('.file-open').click()
    expect(await page.locator('#file-peek .log-line.is-error').count()).toBe(1)
    await page.keyboard.press('Escape')
    await page.locator('#send-button').click()
    await untilPlan()
    expect(await page.locator('.plan-card').textContent()).toContain('Entity Culling')
})

it('leaves a new task empty when its first request cannot be saved', async () => {
    await page.locator('#message-input').fill('Back up my world')
    await page.evaluate(() => (Reflect.get(window, 'agentFixture').storageFails = true))
    await page.locator('#message-input').press('Enter')
    await page.locator('.banner [data-action="retry-storage"]').waitFor()
    // Nothing was launched that could still arrive.
    await page.clock.runFor(2_000)
    expect(await page.locator('.turn').count()).toBe(0)
    expect(await page.locator('#empty-state').isVisible()).toBe(true)
    expect(await page.locator('#composer-home #composer').count()).toBe(1)
    expect(await page.locator('#message-input').inputValue()).toBe('Back up my world')
    expect(await page.locator('.run-status').count()).toBe(0)
    expect(await page.locator('#thread-state').isHidden()).toBe(true)
    expect(await page.locator('#announcer').textContent()).not.toContain(messages.thinking)
})

it('leaves the task list alone while the first request is typed and erased', async () => {
    await page.evaluate(() => {
        const seen: string[] = []
        Reflect.set(window, 'listChanges', seen)
        const input = document.getElementById('message-input') as HTMLTextAreaElement
        new MutationObserver(() => seen.push(input.value)).observe(
            document.getElementById('conversation-list')!,
            { childList: true, subtree: true, characterData: true, attributes: true },
        )
    })
    const changes = () => page.evaluate(() => Reflect.get(window, 'listChanges') as string[])
    const rows = page.locator('#conversation-list [data-thread]')
    expect(await rows.count()).toBe(0)
    await page.locator('#message-input').focus()
    await page.keyboard.type('Hello')
    expect(await rows.count()).toBe(0)
    for (let index = 0; index < 5; index++) await page.keyboard.press('Backspace')
    expect(await changes()).toEqual([])
    expect(await rows.count()).toBe(0)
})

const archive = (name: string) => ({
    name,
    mimeType: 'application/zip',
    buffer: Buffer.from('zip'),
})

it('keeps attachment chips and queued requests in place while typing', async () => {
    await send('The game stutters')
    await send('Why those two mods?')
    await page.locator('#file-input').setInputFiles(archive('world.zip'))
    const chip = await page.locator('#attachment-list .file-chip').first().elementHandle()
    const queued = await page.locator('#queue-list .queued').first().elementHandle()
    await page.locator('#message-input').focus()
    await page.keyboard.type('abc')
    expect(await page.locator('#message-input').inputValue()).toBe('abc')
    expect(
        await page.evaluate(
            ([first, row]) => [
                first === document.querySelector('#attachment-list .file-chip'),
                row === document.querySelector('#queue-list .queued'),
            ],
            [chip, queued],
        ),
    ).toEqual([true, true])
})

it('hands the focus to the next attachment, the one before, then the composer as they go', async () => {
    await page
        .locator('#file-input')
        .setInputFiles(['first.zip', 'second.zip', 'third.zip'].map(archive))
    const remove = (name: string) =>
        page.getByRole('button', {
            name: messages.removeAttachment.replace('{name}', name),
            exact: true,
        })
    await remove('second.zip').focus()
    await page.keyboard.press('Enter')
    expect(await isFocused(remove('third.zip'))).toBe(true)
    await page.keyboard.press('Enter')
    expect(await isFocused(remove('first.zip'))).toBe(true)
    await page.keyboard.press('Enter')
    expect(await page.locator('#attachment-list .file-chip').count()).toBe(0)
    expect(await isFocused(page.locator('#message-input'))).toBe(true)
})

it('hands the focus along the queue as requests are removed, and keeps it in the composer on Send now', async () => {
    await send('The game stutters')
    for (const value of ['One', 'Two', 'Three']) await send(value)
    const rows = page.locator('#queue-list .queued')
    const texts = () => rows.locator('.queued-text').allTextContents()
    const remove = (index: number) => rows.nth(index).locator('[data-remove-queued]')
    expect(await texts()).toEqual(['One', 'Two', 'Three'])
    await remove(1).focus()
    await page.keyboard.press('Enter')
    expect(await texts()).toEqual(['One', 'Three'])
    expect(await isFocused(remove(1))).toBe(true)
    await page.keyboard.press('Enter')
    expect(await texts()).toEqual(['One'])
    expect(await isFocused(remove(0))).toBe(true)
    await page.keyboard.press('Enter')
    expect(await rows.count()).toBe(0)
    expect(await isFocused(page.locator('#message-input'))).toBe(true)

    // A stopped run leaves the queue waiting with a Send now button.
    await send('Later')
    await page.locator('#message-input').press('Escape')
    const sendNow = rows.first().getByRole('button', { name: messages.sendNow, exact: true })
    await sendNow.focus()
    await page.keyboard.press('Enter')
    expect(await rows.count()).toBe(0)
    expect(await page.locator('.turn').count()).toBe(2)
    expect(await isFocused(page.locator('#message-input'))).toBe(true)
})

it('tells a native runtime whether a task works in the launcher, an instance or one that is gone', async () => {
    // A runtime that records what it is asked and never finishes; it has no
    // `preview` field, so the window must treat it as native.
    await page.evaluate(() => {
        const requests: { mode: string; scope: string; instance: string | null }[] = []
        Reflect.set(window, 'runtimeRequests', requests)
        const record =
            (mode: string) => (request: { scope: string; instance: { id: string } | null }) => {
                requests.push({
                    mode,
                    scope: request.scope,
                    instance: request.instance?.id ?? null,
                })
                return { stop() {} }
            }
        Reflect.get(window, 'AgentHost').runtime = {
            start: record('start'),
            approve: record('approve'),
            check: record('check'),
        }
    })
    const context = {
        ai_enabled: true,
        execution_available: true,
        providers: [{ provider_id: 'p', name: 'Provider', models: [{ id: 'm', name: 'Model' }] }],
        instances: [{ id: 'mist', name: 'Morning Mist', game_version: '1.21.1', loader: 'fabric' }],
    }
    await useContext(context)
    await page.locator('#preview-badge').waitFor({ state: 'hidden' })
    const ask = async (value: string, count: number) => {
        await send(value)
        await page.waitForFunction(
            (expected) => Reflect.get(window, 'runtimeRequests').length === expected,
            count,
        )
        // Stopping the unfinished run frees the composer for the next request.
        await page.locator('#message-input').press('Escape')
    }
    await ask('Check my launcher', 1)
    await page.locator('#instance-button').click()
    await page.locator('[data-instance="mist"]').click()
    await ask('Check this instance', 2)
    await useContext({ ...context, instances: [] })
    await page.locator('#instance-label').filter({ hasText: messages.contextRemoved }).waitFor()
    await ask('Check it again', 3)
    expect(await page.evaluate(() => Reflect.get(window, 'runtimeRequests'))).toEqual([
        { mode: 'start', scope: 'launcher', instance: null },
        { mode: 'start', scope: 'instance', instance: 'mist' },
        { mode: 'start', scope: 'missing', instance: null },
    ])

    // Without execution the window goes back to the preview and says so.
    await useContext({ ...context, execution_available: false })
    await page.locator('#preview-badge').waitFor({ state: 'visible' })
})

it('tells you when a task in the background is waiting for approval', async () => {
    await send('My game crashes on launch')
    await page.getByTestId('new-chat').click()
    for (let index = 0; index < 40 && !(await page.locator('.toast').count()); index++)
        await page.clock.runFor(500)
    expect(await page.locator('.toast').first().textContent()).toContain(
        messages.toastAwaiting.split('{title}')[1].trim().slice(0, 4),
    )
})

it('waits for attachment contents before sending', async () => {
    await page.evaluate(() => {
        const file = new File(['log'], 'latest.log', { type: 'text/plain' })
        file.text = () =>
            new Promise((resolve) => Reflect.set(window, 'finishAttachmentRead', resolve))
        const data = new DataTransfer()
        data.items.add(file)
        document
            .getElementById('conversation')!
            .dispatchEvent(new DragEvent('drop', { bubbles: true, dataTransfer: data }))
    })
    await page.locator('#message-input').fill('Read this log')
    await page.locator('#send-button').click()
    expect(await page.locator('.turn').count()).toBe(0)
    await page.evaluate(() => Reflect.get(window, 'finishAttachmentRead')('Read this complete log'))
    await page.locator('.turn').waitFor()
    const stored = await page.evaluate((key) => JSON.parse(localStorage.getItem(key)!), STORE)
    expect(stored.threads[0].turns[0].attachments[0].text).toBe('Read this complete log')
})
