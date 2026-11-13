import {
    I18N_INJECTION_KEY,
    type ImportableLauncher,
    provideInstanceImport,
    type SymlinkMethodChoice,
} from '@modrinth/ui'
import { flushPromises, mount } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { createPinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { defineComponent, h, provide, ref } from 'vue'

import { notifyAccountSelectionChanged } from '@/helpers/account-state'
import { MINECRAFT_ROOTS_STORAGE_KEY } from '@/helpers/minecraft-roots'
import en from '@/locales/en-US/index.json'
import zh from '@/locales/zh-CN/index.json'
import { applyTheme, waitFor } from '@/test/visual-harness'

import OobeHost from './OobeHost.vue'
import { OOBE_STEP_IDS } from './step-ids'
import { OOBE_STEPS } from './steps'
import { steveHead } from './steps/account/accounts'
import { cardClock } from './steps/account/card-physics'
import { createOobeController } from './useOobe'

function headUrl(textureKey: string) {
    const hue = [...textureKey].reduce((sum, char) => sum + char.charCodeAt(0), 0) % 360
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><rect width="8" height="8" fill="hsl(${hue} 60% 45%)"/><rect x="1" y="3" width="2" height="1" fill="white"/><rect x="5" y="3" width="2" height="1" fill="white"/></svg>`
    return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

const backend = vi.hoisted(() => ({
    patches: [] as Record<string, unknown>[],
    restricted: true,
    accounts: [] as Array<{
        account_id: string
        account_type: 'yggdrasil' | 'microsoft'
        profile: { id: string; name: string; skins?: Array<{ state: string; url: string }> }
    }>,
    defaultAccount: null as string | null,
    failDefault: false,
    failSync: false,
    syncPending: null as Promise<void> | null,
    syncImported: 2,
    failImport: false,
    javas: [] as Array<{
        parsed_version: number
        version: string
        architecture: string
        path: string
        distribution: string | null
    }>,
    javaDefaults: [] as Array<{ parsed_version: number; path: string }>,
    externalJava: false,
    acceptExternalJava: false,
    invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', async (importOriginal) => ({
    ...(await importOriginal<object>()),
    invoke: backend.invoke,
}))
vi.mock('@tauri-apps/api/event', async (importOriginal) => ({
    ...(await importOriginal<object>()),
    listen: async () => () => {},
}))
// A head per skin texture, in a colour of its own, so the art shows which account it drew.
vi.mock('@/helpers/rendering/batch-skin-renderer.ts', () => ({
    getPlayerHeadUrl: async ({ texture_key }: { texture_key: string }) => headUrl(texture_key),
    retainPlayerHeadTextures: () => () => {},
}))
vi.mock('@tauri-apps/plugin-dialog', async (importOriginal) => ({
    ...(await importOriginal<object>()),
    open: async () => null,
    confirm: async () => backend.acceptExternalJava,
}))

backend.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    switch (command) {
        case 'plugin:settings|settings_get':
            return { locale: 'en-US', theme: 'dark', feature_flags: {} }
        case 'plugin:settings|settings_patch':
            backend.patches.push(args?.patch as Record<string, unknown>)
            return { locale: 'en-US', feature_flags: {}, ...(args?.patch as object) }
        case 'plugin:auth|get_users':
            return backend.accounts
        case 'plugin:auth|get_default_user':
            return backend.defaultAccount
        case 'plugin:auth|set_default_user':
            if (backend.failDefault) throw new Error('account update failed')
            backend.defaultAccount = args?.user as string
            return null
        case 'plugin:auth|get_anti_piracy_status':
            return { region: 'cn', restricted: backend.restricted, revision: 1 }
        case 'plugin:instance|instance_sync_direct_links':
            await backend.syncPending
            if (backend.failSync) throw new Error('sync failed')
            return {
                imported: backend.syncImported,
                updated: 0,
                removed: 0,
                missing: 0,
                errors: [],
            }
        case 'plugin:jre|jre_find_filtered_jres':
            return backend.javas
        case 'plugin:jre|get_java_default_versions':
            return backend.javaDefaults
        case 'plugin:jre|get_java_selection_info': {
            const runtime = backend.javas.find((entry) => entry.path === args?.path)
            if (!runtime) throw new Error('not a Java runtime')
            return {
                runtime,
                storage: { kind: backend.externalJava ? 'external' : 'local', volume_id: 'volume' },
                requires_confirmation: backend.externalJava,
            }
        }
        case 'plugin:jre|approve_java_selection':
            return backend.javas.find((entry) => entry.path === args?.path)
        case 'plugin:jre|set_java_default_version': {
            const java = backend.javas.find((entry) => entry.path === args?.path)
            if (!java) throw new Error('not a Java runtime')
            backend.javaDefaults = [
                ...backend.javaDefaults.filter(
                    (entry) => entry.parsed_version !== java.parsed_version,
                ),
                java,
            ]
            return java
        }
        case 'plugin:jre|jre_get_jre':
            return backend.javas.find((entry) => entry.path === args?.path) ?? null
        case 'plugin:install|install_import_instance':
            if (backend.failImport) throw new Error('import could not be queued')
            return { job_id: 'oobe-import' }
        default:
            return null
    }
})

const evidence = import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE as string | undefined
const mounted: ReturnType<typeof mount>[] = []

afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    localStorage.removeItem(MINECRAFT_ROOTS_STORAGE_KEY)
    backend.patches.length = 0
    backend.invoke.mockClear()
    backend.accounts = []
    backend.defaultAccount = null
    backend.failDefault = false
    backend.failSync = false
    backend.syncPending = null
    backend.syncImported = 2
    backend.failImport = false
    backend.javas = []
    backend.javaDefaults = []
    backend.externalJava = false
    backend.acceptExternalJava = false
})

function i18n(locale: 'en-US' | 'zh-CN') {
    const messages = (locale === 'zh-CN' ? zh : en) as Record<string, { message: string }>
    return {
        locale: ref(locale),
        t: (key: string, values?: Record<string, unknown>) => {
            const message = messages[key]?.message
            return message
                ? (new IntlMessageFormat(message, locale).format(values ?? {}) as string)
                : key
        },
        setLocale: () => {},
    }
}

async function render(
    locale: 'en-US' | 'zh-CN' = 'en-US',
    steps = OOBE_STEPS,
    detectedLaunchers?: ImportableLauncher[],
) {
    await page.viewport(1100, 700)
    const events = { complete: [] as string[], finished: 0 }
    const chooseImportMethod = vi.fn(async (): Promise<SymlinkMethodChoice[]> => [])
    const Harness = defineComponent({
        setup() {
            provideInstanceImport({
                getDetectedLaunchers: async (
                    onLauncher?: (launcher: ImportableLauncher) => void,
                ) => {
                    const launchers = detectedLaunchers ?? [
                        {
                            name: 'PCL2CE',
                            path: 'C:/Users/Steve/AppData/Roaming',
                            instances: [
                                {
                                    name: '.minecraft:1.21.4 Fabric',
                                    path: 'D:/Games/.minecraft/versions/1.21.4 Fabric',
                                },
                                {
                                    name: '.minecraft:1.20.1 Forge',
                                    path: 'D:/Games/.minecraft/versions/1.20.1 Forge',
                                },
                            ],
                        },
                        {
                            name: 'PrismLauncher',
                            path: 'C:/Users/Steve/AppData/Roaming/PrismLauncher',
                            instances: [
                                {
                                    name: 'Skyblock',
                                    path: 'C:/Users/Steve/AppData/Roaming/PrismLauncher/instances/Skyblock',
                                },
                            ],
                        },
                    ]
                    launchers.forEach((launcher) => onLauncher?.(launcher))
                    return launchers
                },
            } as never)
            provide('chooseImportMethod', chooseImportMethod)
            provide('showCreationModalWithOptions', () => {})
            return () =>
                h(OobeHost, {
                    steps,
                    onComplete: (id: string) => events.complete.push(id),
                    onFinish: () => events.finished++,
                })
        },
    })
    applyTheme('dark')
    const wrapper = mount(Harness, {
        attachTo: document.body,
        global: {
            plugins: [createPinia()],
            provide: {
                [I18N_INJECTION_KEY as symbol]: i18n(locale),
                [Symbol.for('modrinth:notificationManager')]: {
                    addNotification: vi.fn(),
                    handleError: vi.fn(),
                },
            },
            // Real transitions: the stub adds a wrapper element the layout never has.
            stubs: { WindowControls: true, transition: false },
            directives: { tooltip: () => {} },
        },
    })
    mounted.push(wrapper)
    return { wrapper, events, chooseImportMethod }
}

function heading() {
    return document.querySelector('.oobe-root h1')?.textContent?.trim()
}

function button(label: string) {
    return Array.from(document.querySelectorAll<HTMLButtonElement>('.oobe-root button')).find(
        (element) => element.textContent?.trim() === label,
    )
}

/** The title, the footer and the window itself stay put; only the step's own list scrolls. */
function expectInsideCard() {
    const card = document.querySelector<HTMLElement>('.oobe-card')!.getBoundingClientRect()
    const title = document.querySelector<HTMLElement>('.oobe-root h1')!.getBoundingClientRect()
    const next = document
        .querySelector<HTMLElement>('.oobe-root footer button:last-child')!
        .getBoundingClientRect()
    expect(document.querySelector<HTMLElement>('.oobe-root')!.scrollTop).toBe(0)
    expect(title.top).toBeGreaterThanOrEqual(card.top)
    expect(next.bottom).toBeLessThanOrEqual(card.bottom)
    expect(card.bottom).toBeLessThanOrEqual(window.innerHeight)
}

async function shot(name: string) {
    if (!evidence) return
    // Let the step's entrance finish first, so the evidence shows the layout people see.
    await waitFor(() => !document.querySelector('.oobe-root [class*="-enter-active"]'), {
        label: `${name} settled`,
    })
    await page.screenshot({ path: `${evidence}/${name}.png` })
}

it('lists every step once, in the order of the persisted ids', () => {
    expect(OOBE_STEPS.map((step) => step.id)).toEqual([...OOBE_STEP_IDS])
})

it('walks the whole flow and records each step as the user moves past it', async () => {
    const { events } = await render()
    await waitFor(() => heading() === 'Choose the launcher language.', { label: 'language step' })
    await waitFor(() => !!document.querySelector('[role="radio"][aria-checked="true"]'), {
        label: 'language selection',
    })
    expectInsideCard()
    await shot('oobe-language')

    button('Next')!.click()
    await waitFor(() => heading() === 'Who is using the launcher?', { label: 'account step' })
    await waitFor(() => document.body.textContent!.includes('Microsoft account'), {
        label: 'account options',
    })
    expect(events.complete).toEqual(['language_v1'])
    // No account changes yet; the single forward action skips the step.
    expect(button('Next')).toBeUndefined()
    expect(button('Skip')!.disabled).toBe(false)
    await waitFor(
        () =>
            document.body.textContent!.includes(
                'Sign in with a Microsoft account on this device first.',
            ),
        { label: 'offline restriction' },
    )
    await shot('oobe-account')

    button('Skip')!.click()
    await waitFor(() => heading() === 'Import data from other launchers?', {
        label: 'migration step',
    })
    await waitFor(() => document.body.textContent!.includes('manages its 2 versions directly'), {
        label: 'detected .minecraft folder',
    })
    expect(document.body.textContent).toContain('1 instance')
    await shot('oobe-migration')

    button('Skip')!.click()
    await waitFor(() => heading() === 'Choose or download Java.', { label: 'java step' })
    await waitFor(() => document.body.textContent!.includes('No Java installations were found'), {
        label: 'java scan',
    })
    expectInsideCard()
    await shot('oobe-java-empty')

    button('Skip')!.click()
    await waitFor(() => heading() === 'Choose the style you like.', { label: 'appearance step' })
    await waitFor(() => document.body.textContent!.includes('Match system'), {
        label: 'appearance options',
    })
    const light = Array.from(document.querySelectorAll<HTMLElement>('[role="radio"]')).find(
        (element) => element.textContent?.includes('Light'),
    )!
    light.click()
    await flushPromises()
    expect(document.documentElement.classList.contains('light-mode')).toBe(true)
    await waitFor(() => backend.patches.some((patch) => patch.theme === 'light'), {
        label: 'theme saved',
    })
    expectInsideCard()
    await shot('oobe-appearance-light')
    applyTheme('dark')

    button('Next')!.click()
    await waitFor(() => heading() === 'Welcome!', { label: 'welcome step' })
    await shot('oobe-welcome')
    button('Get started')!.click()
    expect(events.complete).toEqual([
        'language_v1',
        'account_v1',
        'migration_v1',
        'java_v1',
        'appearance_v1',
        'welcome_v1',
    ])
    expect(events.finished).toBe(1)
})

it('renders in Simplified Chinese without overflowing the card', async () => {
    await render('zh-CN')
    await waitFor(() => heading() === '选择启动器语言。', { label: 'zh language step' })
    await waitFor(() => document.querySelectorAll('.oobe-root [role="radio"]').length > 10, {
        label: 'zh language list',
    })
    const card = document.querySelector<HTMLElement>('.oobe-card')!
    expect(card.scrollWidth).toBeLessThanOrEqual(card.clientWidth)
    expect(card.getBoundingClientRect().bottom).toBeLessThanOrEqual(window.innerHeight)
    await shot('oobe-language-zh')
})

const migrationOnly = OOBE_STEPS.filter((step) => ['migration_v1', 'welcome_v1'].includes(step.id))

it('keeps Skip for existing accounts and failed edits, then keeps Next after a successful change and revisiting', async () => {
    backend.accounts = ['Alex', 'Steve'].map((name) => ({
        account_id: name,
        account_type: 'yggdrasil',
        profile: { id: name, name },
    }))
    const accountChoice = (name: string) =>
        Array.from(document.querySelectorAll<HTMLButtonElement>('.oobe-root [role="radio"]')).find(
            (entry) => entry.textContent?.includes(name),
        )
    backend.defaultAccount = 'Alex'
    await render(
        'en-US',
        OOBE_STEPS.filter((step) => ['account_v1', 'migration_v1'].includes(step.id)),
    )
    await waitFor(() => !!accountChoice('Steve'), { label: 'existing accounts loaded' })
    expect(button('Next')).toBeUndefined()
    expect(button('Skip')!.disabled).toBe(false)
    accountChoice('Alex')!.click()
    await flushPromises()
    expect(button('Next')).toBeUndefined()

    backend.failDefault = true
    accountChoice('Steve')!.click()
    await flushPromises()
    expect(button('Next')).toBeUndefined()
    backend.failDefault = false
    accountChoice('Steve')!.click()
    await waitFor(() => !!button('Next'), { label: 'default account changed' })
    expect(button('Skip')).toBeUndefined()
    expect(document.querySelectorAll('.oobe-root footer button')).toHaveLength(1)
    button('Next')!.click()
    await waitFor(() => heading() === 'Import data from other launchers?', { label: 'migration' })
    button('Back')!.click()
    await waitFor(() => heading() === 'Who is using the launcher?', { label: 'account revisited' })
    await waitFor(() => !!accountChoice('Steve'), { label: 'revisited accounts loaded' })
    expect(button('Skip')).toBeUndefined()
    expect(button('Next')!.disabled).toBe(false)
})

describe('account art', () => {
    // The cards move on springs driven by `cardClock`; these tests step that clock by hand.
    let clock = 0
    const realNow = cardClock.now
    beforeEach(() => {
        clock = 0
        cardClock.now = () => clock
    })
    afterEach(() => {
        cardClock.now = realNow
    })
    const nextFrame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    /** Moves the cards' clock on by `ms`, `step` at a time, letting a frame draw each step. */
    async function advance(ms: number, step = 16) {
        for (let elapsed = 0; elapsed < ms; elapsed += step) {
            clock += Math.min(step, ms - elapsed)
            await nextFrame()
        }
    }
    const settleCards = () => advance(1600, 100)

    const card = (slot: 'front' | 'left' | 'right' | 'hidden') =>
        document.querySelector<HTMLElement>(
            `[data-testid="oobe-account-card"][data-slot="${slot}"]`,
        )
    const cardHead = (slot: 'front' | 'left' | 'right' | 'hidden') =>
        card(slot)?.querySelector('[data-testid="oobe-account-head"]')?.getAttribute('href') ??
        undefined
    const cardOf = (name: string) =>
        document.querySelector<HTMLElement>(
            `[data-testid="oobe-account-card"][data-account="${name}"]`,
        )!
    const head = (name: string) => headUrl(`${name}:https://skins.test/${name}`)
    const accountChoice = (name: string) =>
        Array.from(document.querySelectorAll<HTMLButtonElement>('.oobe-root [role="radio"]')).find(
            (entry) => entry.textContent?.includes(name),
        )
    /** Where a card is drawn: offset from the front card's place, turn, size and stacking. */
    function sample(element: HTMLElement) {
        const style = getComputedStyle(element)
        const [a = 1, b = 0, , , x = 0, y = 0] = (
            style.transform.match(/-?[\d.]+(e-?\d+)?/g) ?? []
        ).map(Number)
        return {
            x,
            y,
            r: (Math.atan2(b, a) * 180) / Math.PI,
            s: Math.hypot(a, b),
            z: Number(style.zIndex),
        }
    }
    type Sample = ReturnType<typeof sample>
    const distance = (a: Sample, b: Sample) => Math.hypot(a.x - b.x, a.y - b.y)
    /** Follows two cards for `frames` frames of 16 ms. */
    async function follow(first: string, second: string, frames: number) {
        const samples: { first: Sample; second: Sample }[] = []
        for (let i = 0; i < frames; i++) {
            await advance(16)
            samples.push({ first: sample(cardOf(first)), second: sample(cardOf(second)) })
        }
        return samples
    }
    /** What a sign-in leaves behind: the stored accounts, one selected, and the announcement. */
    function signIn(names: string[], selected: string) {
        backend.accounts = names.map((name) => ({
            account_id: name,
            account_type: 'microsoft',
            profile: {
                id: name,
                name,
                skins: [{ state: 'ACTIVE', url: `https://skins.test/${name}` }],
            },
        }))
        backend.defaultAccount = selected
        notifyAccountSelectionChanged()
    }
    async function renderAccounts(names: string[], selected: string) {
        signIn(names, selected)
        await render('en-US', accountOnly)
        await waitFor(
            () =>
                names.every((name) => !!accountChoice(name)) &&
                cardHead('front') === head(selected),
            { label: 'accounts listed' },
        )
        await settleCards()
    }
    const accountOnly = OOBE_STEPS.filter((step) => ['account_v1', 'welcome_v1'].includes(step.id))

    it('shows Steve in front until an account signs in, and other accounts behind only from two', async () => {
        await render('en-US', accountOnly)
        await waitFor(() => !!cardHead('front'), { label: 'account art' })
        expect(cardHead('front')).toBe(steveHead)
        expect(cardHead('left')).toBeUndefined()
        expect(cardHead('right')).toBeUndefined()
        await shot('oobe-account-art-steve')

        signIn(['Alex'], 'Alex')
        await waitFor(() => cardHead('front') === head('Alex'), { label: 'signed-in head' })
        expect(cardHead('left')).toBeUndefined()

        signIn(['Alex', 'Notch', 'Zuri'], 'Alex')
        await waitFor(() => cardHead('right') === head('Zuri'), { label: 'back cards' })
        expect(cardHead('front')).toBe(head('Alex'))
        expect(cardHead('left')).toBe(head('Notch'))
        await settleCards()
        await shot('oobe-account-art-three')
        applyTheme('light')
        await shot('oobe-account-art-three-light')
        applyTheme('dark')
    })

    it('swings a picked back card up and around to the front without a pause', async () => {
        await renderAccounts(['Alex', 'Notch', 'Zuri'], 'Alex')
        accountChoice('Zuri')!.click()
        await waitFor(() => cardOf('Zuri').dataset.slot === 'front', { label: 'Zuri picked' })

        const samples = await follow('Zuri', 'Alex', 50)
        // Up and out from behind the front card first.
        expect(Math.min(...samples.map(({ first }) => first.y))).toBeLessThan(-40)
        // The two trade stacking order once, and only when Zuri's card is clear above Alex's.
        const crossing = samples.findIndex(({ first, second }) => first.z > second.z)
        expect(crossing).toBeGreaterThan(3)
        expect(samples[crossing]!.first.y).toBeLessThan(samples[crossing]!.second.y - 40)
        // No pause on the way: while it is still far from the front it keeps moving.
        const front = { x: 0, y: 0, r: 0, s: 1, z: 0 }
        for (let i = 1; i < samples.length; i++) {
            if (distance(samples[i]!.first, front) < 15) break
            expect(distance(samples[i]!.first, samples[i - 1]!.first)).toBeGreaterThan(1)
        }
        await settleCards()
        expect([cardHead('front'), cardHead('left'), cardHead('right')]).toEqual([
            head('Zuri'),
            head('Notch'),
            head('Alex'),
        ])
        await shot('oobe-account-art-swapped')
    })

    it('turns both cards back from where they are, at the speed they have, when a pick is taken back', async () => {
        await renderAccounts(['Alex', 'Notch', 'Zuri'], 'Alex')
        accountChoice('Zuri')!.click()
        await waitFor(() => cardOf('Zuri').dataset.slot === 'front', { label: 'Zuri picked' })
        const going = await follow('Zuri', 'Alex', 8)
        await shot('oobe-account-art-reverse-before')

        // Back to Alex while Zuri's card is still swinging out behind it.
        accountChoice('Alex')!.click()
        await waitFor(() => cardOf('Alex').dataset.slot === 'front', { label: 'Alex picked back' })
        const back = await follow('Zuri', 'Alex', 40)
        // No jump and no stop: the first step back is about as long as the last step out.
        const lastOut = going[going.length - 1]!
        const stepOut = distance(lastOut.first, going[going.length - 2]!.first)
        const stepBack = distance(back[0]!.first, lastOut.first)
        expect(stepBack).toBeGreaterThan(stepOut * 0.35)
        expect(stepBack).toBeLessThan(stepOut * 1.6)
        // Zuri's card never got in front, so it stays behind Alex's all the way back.
        expect(back.every(({ first, second }) => first.z < second.z)).toBe(true)
        await shot('oobe-account-art-reverse-after')
        await settleCards()
        expect([cardHead('front'), cardHead('left'), cardHead('right')]).toEqual([
            head('Alex'),
            head('Notch'),
            head('Zuri'),
        ])
    })

    it('bends a card still in flight on to its new place when another account is picked', async () => {
        await renderAccounts(['Alex', 'Notch', 'Zuri'], 'Alex')
        accountChoice('Zuri')!.click()
        await waitFor(() => cardOf('Zuri').dataset.slot === 'front', { label: 'Zuri picked' })
        const going = await follow('Zuri', 'Alex', 8)

        // Notch before Zuri lands: Zuri turns off toward the left, Alex keeps sinking right.
        accountChoice('Notch')!.click()
        await waitFor(() => cardOf('Notch').dataset.slot === 'front', { label: 'Notch picked' })
        expect(cardOf('Zuri').dataset.slot).toBe('left')
        expect(cardOf('Alex').dataset.slot).toBe('right')
        const after = await follow('Zuri', 'Alex', 30)
        const lastOut = going[going.length - 1]!
        for (const which of ['first', 'second'] as const) {
            const stepOut = distance(lastOut[which], going[going.length - 2]![which])
            const stepOn = distance(after[0]![which], lastOut[which])
            expect(stepOn).toBeGreaterThan(stepOut * 0.35)
            expect(stepOn).toBeLessThan(stepOut * 1.6)
        }
        await shot('oobe-account-art-chain')
        await settleCards()
        expect([cardHead('front'), cardHead('left'), cardHead('right')]).toEqual([
            head('Notch'),
            head('Zuri'),
            head('Alex'),
        ])
        await shot('oobe-account-art-chained')
    })

    it('hides a card from four accounts and draws accounts without a card out of it', async () => {
        // Sorted, these are Alex, Bo, Ida, Kai, Notch, Zuri: Kai is on the hidden card.
        await renderAccounts(['Alex', 'Bo', 'Ida', 'Kai', 'Notch', 'Zuri'], 'Alex')
        expect([cardHead('left'), cardHead('right'), cardHead('hidden')]).toEqual([
            head('Bo'),
            head('Ida'),
            head('Kai'),
        ])
        // Only the hidden card's top edge shows; its head lies within the front card.
        const covered = card('front')!.getBoundingClientRect()
        const hiddenHead = card('hidden')!
            .querySelector('[data-testid="oobe-account-head"]')!
            .getBoundingClientRect()
        expect(hiddenHead.top).toBeGreaterThan(covered.top)
        expect(hiddenHead.left).toBeGreaterThan(covered.left)
        expect(hiddenHead.right).toBeLessThan(covered.right)
        expect(card('hidden')!.getBoundingClientRect().top).toBeLessThan(covered.top)
        await shot('oobe-account-art-deck')

        // Zuri has no card: it takes over the hidden card out of sight and is drawn out of it.
        accountChoice('Zuri')!.click()
        await waitFor(
            () =>
                document.querySelector<HTMLElement>('[data-account="Zuri"]')?.dataset.slot ===
                'front',
            { label: 'Zuri drawn' },
        )
        expect(cardHead('hidden')).toBe(head('Alex'))
        const start = sample(cardOf('Zuri'))
        expect(start.x).toBeCloseTo(0, 0)
        expect(start.y).toBeCloseTo(-46, 0)
        expect(start.s).toBeCloseTo(0.8, 2)
        expect(start.z).toBeLessThan(sample(cardOf('Alex')).z)

        const samples = await follow('Zuri', 'Alex', 50)
        expect(Math.min(...samples.map(({ first }) => first.y))).toBeLessThan(-60)
        const crossing = samples.findIndex(({ first, second }) => first.z > second.z)
        expect(crossing).toBeGreaterThan(3)
        await settleCards()
        expect([
            cardHead('front'),
            cardHead('left'),
            cardHead('right'),
            cardHead('hidden'),
        ]).toEqual([head('Zuri'), head('Bo'), head('Ida'), head('Alex')])
        await shot('oobe-account-art-drawn')
    })
})

it('keeps Skip after a folder sync fails and changes to Next only when retry succeeds', async () => {
    await render('en-US', migrationOnly)
    await waitFor(() => !!button('Use this folder'), { label: 'found folder' })
    backend.failSync = true
    button('Use this folder')!.click()
    await waitFor(() => document.body.textContent!.includes('sync failed'), {
        label: 'failed sync',
    })
    expect(button('Next')).toBeUndefined()
    expect(button('Skip')!.disabled).toBe(false)
    backend.failSync = false
    button('Use this folder')!.click()
    await waitFor(() => !!button('Next') && !button('Next')!.disabled, { label: 'sync retried' })
    expect(button('Skip')).toBeUndefined()
})

it('disables the merged action during work and keeps Skip when an existing folder needs no changes', async () => {
    localStorage.setItem(
        MINECRAFT_ROOTS_STORAGE_KEY,
        JSON.stringify([{ path: 'D:/Games/.minecraft', mode: 'automatic' }]),
    )
    let finish!: () => void
    backend.syncPending = new Promise<void>((resolve) => {
        finish = resolve
    })
    backend.syncImported = 0
    await render('en-US', migrationOnly)
    await waitFor(() => !!button('Use this folder'), { label: 'existing folder' })
    button('Use this folder')!.click()
    await flushPromises()
    expect(button('Next')).toBeUndefined()
    expect(button('Skip')!.disabled).toBe(true)
    finish()
    await waitFor(() => !button('Skip')!.disabled, { label: 'unchanged folder sync completed' })
    expect(button('Next')).toBeUndefined()
})

it('keeps Skip when importing fails to start and shows Next once an import is queued', async () => {
    const { chooseImportMethod } = await render('en-US', migrationOnly)
    await waitFor(() => !!button('Use this folder'), { label: 'found folder' })
    const importMethod = Array.from(
        document.querySelectorAll<HTMLElement>('.oobe-root [role="radio"]'),
    ).find((method) => method.textContent?.trim() === 'Import')!
    importMethod.click()
    await flushPromises()
    document.querySelector<HTMLButtonElement>('.oobe-root [role="checkbox"]')!.click()
    await flushPromises()
    expect(button('Next')).toBeUndefined()
    chooseImportMethod.mockResolvedValue([
        {
            instanceName: '.minecraft:1.21.4 Fabric',
            instancePath: 'D:/Games/.minecraft/versions/1.21.4 Fabric',
            symlink: false,
        },
    ])
    backend.failImport = true
    button('Import 1 instance')!.click()
    await flushPromises()
    expect(button('Next')).toBeUndefined()
    expect(button('Skip')!.disabled).toBe(false)
    backend.failImport = false
    button('Import 1 instance')!.click()
    await waitFor(() => !!button('Next'), { label: 'import queued' })
    expect(button('Skip')).toBeUndefined()
})

it('puts the .minecraft folders first and uses a found one in place by default', async () => {
    await render('en-US', migrationOnly)
    await waitFor(() => !!button('Use this folder'), { label: 'found .minecraft folder' })
    expect(
        Array.from(document.querySelectorAll('.oobe-root h2')).map((title) =>
            title.textContent?.trim(),
        ),
    ).toEqual(['Use a .minecraft folder in place', 'Found on this device'])
    expect(button('Next')).toBeUndefined()
    expect(button('Skip')!.disabled).toBe(false)

    button('Use this folder')!.click()
    await waitFor(() => document.body.textContent!.includes('Added, 2 versions found'), {
        label: 'folder in use',
    })
    const roots = [{ path: 'D:/Games/.minecraft', mode: 'automatic' }]
    expect(backend.invoke).toHaveBeenCalledWith('plugin:instance|instance_sync_direct_links', {
        roots,
    })
    expect(JSON.parse(localStorage.getItem(MINECRAFT_ROOTS_STORAGE_KEY)!)).toEqual(roots)
    // Nothing was copied, and the folder can no longer switch to being imported.
    expect(backend.invoke.mock.calls.some(([command]) => String(command).includes('import'))).toBe(
        false,
    )
    const methods = document.querySelectorAll<HTMLButtonElement>('.oobe-root [role="radio"]')
    expect(Array.from(methods).every((method) => method.disabled)).toBe(true)
    await waitFor(() => !!button('Next') && !button('Next')!.disabled, { label: 'step ready' })
    expect(button('Skip')).toBeUndefined()
    expectInsideCard()
    await shot('oobe-migration-direct')
})

it('lists the versions of a found folder once the user switches it to import', async () => {
    await render('en-US', migrationOnly)
    await waitFor(() => !!button('Use this folder'), { label: 'found .minecraft folder' })
    const importMethod = Array.from(
        document.querySelectorAll<HTMLElement>('.oobe-root [role="radio"]'),
    ).find((method) => method.textContent?.trim() === 'Import')!
    importMethod.click()
    await flushPromises()
    expect(button('Use this folder')).toBeUndefined()
    expect(document.body.textContent).toContain('1.21.4 Fabric')
    expect(document.body.textContent).toContain('1.20.1 Forge')
    expect(document.body.textContent).not.toContain('.minecraft:1.21.4 Fabric')
    expect(button('Skip')!.disabled).toBe(false)
    expect(button('Next')).toBeUndefined()
    expect(backend.invoke).not.toHaveBeenCalledWith(
        'plugin:instance|instance_sync_direct_links',
        expect.anything(),
    )
    await shot('oobe-migration-import')
})

it('uses shared game folders directly and imports only the selected version with its metadata path', async () => {
    const root = 'C:/Users/Steve/AppData/Roaming/.minecraft'
    const instances = ['1.20.1', '1.21.1'].map((name) => ({
        name,
        path: root,
        version_path: `${root}/versions/${name}`,
        compatible_mode: true,
    }))
    const { chooseImportMethod } = await render('zh-CN', migrationOnly, [
        { name: 'PCL2CE', path: 'C:/Users/Steve/AppData/Roaming', instances },
        { name: 'MinecraftLauncher', launcherType: 'Generic', path: root, instances },
    ])
    await waitFor(() => !!button('使用此文件夹'), { label: 'shared folder direct action' })
    expect(document.body.textContent).toContain('官方启动器')
    expect(document.querySelectorAll('.oobe-root [role="radio"]')).toHaveLength(2)
    const importMethod = Array.from(
        document.querySelectorAll<HTMLElement>('.oobe-root [role="radio"]'),
    ).find((method) => method.textContent?.trim() === '导入')!
    importMethod.click()
    await flushPromises()
    const checkboxes = Array.from(
        document.querySelectorAll<HTMLButtonElement>('.oobe-root [role="checkbox"]'),
    )
    expect(checkboxes).toHaveLength(2)
    checkboxes[0]!.click()
    await flushPromises()
    expect(checkboxes[0]!.getAttribute('aria-checked')).toBe('true')
    expect(checkboxes[1]!.getAttribute('aria-checked')).toBe('false')
    button('导入 1 个实例')!.click()
    await vi.waitFor(() =>
        expect(chooseImportMethod).toHaveBeenCalledWith({
            instances: [
                {
                    name: '1.20.1',
                    path: `${root}/versions/1.20.1`,
                    launcherType: 'Generic',
                    basePath: root,
                },
            ],
        }),
    )
    checkboxes[1]!.click()
    await flushPromises()
    expect(checkboxes.every((checkbox) => checkbox.getAttribute('aria-checked') === 'true')).toBe(
        true,
    )
    checkboxes[0]!.click()
    await flushPromises()
    expect(checkboxes[0]!.getAttribute('aria-checked')).toBe('false')
    expect(checkboxes[1]!.getAttribute('aria-checked')).toBe('true')
    await shot('oobe-migration-shared-zh')
})

it('uses the official shared game folder in place in the light theme', async () => {
    const root = 'C:/Users/Steve/AppData/Roaming/.minecraft'
    await render('en-US', migrationOnly, [
        {
            name: 'MinecraftLauncher',
            launcherType: 'Generic',
            path: root,
            instances: ['1.20.1', '1.21.1'].map((name) => ({
                name,
                path: root,
                version_path: `${root}/versions/${name}`,
                compatible_mode: true,
            })),
        },
    ])
    applyTheme('light')
    await waitFor(() => !!button('Use this folder'), { label: 'official shared folder' })
    expect(document.body.textContent).toContain('Official launcher')
    button('Use this folder')!.click()
    await waitFor(() => !!button('Next') && !button('Next')!.disabled, {
        label: 'shared folder in use',
    })
    expect(backend.invoke).toHaveBeenCalledWith('plugin:instance|instance_sync_direct_links', {
        roots: [{ path: root, mode: 'automatic' }],
    })
    expectInsideCard()
    await shot('oobe-migration-shared-light')
})

const javaOnly = OOBE_STEPS.filter((step) => ['java_v1', 'welcome_v1'].includes(step.id))

function javaRuntime(parsed: number, version: string, path: string) {
    return {
        parsed_version: parsed,
        version,
        architecture: 'x86_64',
        path,
        distribution: 'Temurin',
    }
}

it('groups the Java found on this device and changes Skip to Next once one is chosen', async () => {
    backend.javas = [
        javaRuntime(17, '17.0.12', 'C:/Program Files/Eclipse Adoptium/jdk-17/bin/javaw.exe'),
        javaRuntime(21, '21.0.5', 'C:/Program Files/Eclipse Adoptium/jdk-21/bin/javaw.exe'),
        javaRuntime(21, '21.0.3', 'D:/Java/jdk-21/bin/javaw.exe'),
    ]
    backend.javaDefaults = [backend.javas[1]!]
    const { events } = await render('en-US', javaOnly)
    const choice = (path: string) =>
        Array.from(document.querySelectorAll<HTMLElement>('.oobe-root [role="radio"]')).find(
            (entry) => entry.textContent?.includes(path),
        )
    await waitFor(() => !!choice('D:/Java/jdk-21/bin/javaw.exe'), { label: 'found Java' })
    expect(document.querySelectorAll('.oobe-root [role="radiogroup"]')).toHaveLength(2)
    expect(
        choice('C:/Program Files/Eclipse Adoptium/jdk-21/bin/javaw.exe')!.getAttribute(
            'aria-checked',
        ),
    ).toBe('true')
    expect(button('Skip')!.disabled).toBe(false)
    expectInsideCard()
    await shot('oobe-java-found')

    choice('C:/Program Files/Eclipse Adoptium/jdk-21/bin/javaw.exe')!.click()
    await flushPromises()
    expect(button('Next')).toBeUndefined()

    choice('D:/Java/jdk-21/bin/javaw.exe')!.click()
    await waitFor(() => !!button('Next'), { label: 'Java chosen' })
    expect(backend.invoke).toHaveBeenCalledWith('plugin:jre|set_java_default_version', {
        majorVersion: 21,
        path: 'D:/Java/jdk-21/bin/javaw.exe',
    })
    expect(choice('D:/Java/jdk-21/bin/javaw.exe')!.getAttribute('aria-checked')).toBe('true')
    expect(events.complete).toEqual(['java_v1'])
    applyTheme('light')
    await shot('oobe-java-found-light')
})

it('requires approval before persisting an external Java choice during setup', async () => {
    backend.externalJava = true
    backend.javas = [javaRuntime(17, '17.0.12', 'E:/portable/bin/javaw.exe')]
    const { events } = await render('en-US', javaOnly)
    const choice = () =>
        Array.from(document.querySelectorAll<HTMLElement>('.oobe-root [role="radio"]')).find(
            (entry) => entry.textContent?.includes(backend.javas[0]!.path),
        )
    await waitFor(() => !!choice(), { label: 'external Java choice' })
    choice()!.click()
    await flushPromises()
    expect(backend.javaDefaults).toEqual([])
    expect(events.complete).toEqual([])
    expect(backend.invoke).not.toHaveBeenCalledWith(
        'plugin:jre|approve_java_selection',
        expect.anything(),
    )
    backend.acceptExternalJava = true
    choice()!.click()
    await waitFor(() => backend.javaDefaults.length === 1, { label: 'approved Java default' })
    expect(backend.invoke).toHaveBeenCalledWith('plugin:jre|approve_java_selection', {
        path: 'E:/portable/bin/javaw.exe',
        volumeId: 'volume',
    })
    expect(events.complete).toEqual(['java_v1'])
})

it('keeps Skip when a typed Java path cannot be started', async () => {
    await render('zh-CN', javaOnly)
    await waitFor(() => heading() === '选择或下载 Java。', { label: 'zh java step' })
    const input = document.querySelector<HTMLInputElement>('.oobe-root input')!
    input.value = 'C:/nothing/java.exe'
    input.dispatchEvent(new Event('input'))
    await flushPromises()
    button('添加')!.click()
    await waitFor(() => document.body.textContent!.includes('无法从这个路径启动 Java。'), {
        label: 'invalid Java path',
    })
    expect(button('跳过')!.disabled).toBe(false)
    const card = document.querySelector<HTMLElement>('.oobe-card')!
    expect(card.scrollWidth).toBeLessThanOrEqual(card.clientWidth)
    await shot('oobe-java-invalid-zh')
})

it('shows only the steps a release added, and writes the record in order', async () => {
    const saved: string[][] = []
    let release!: () => void
    const controller = createOobeController(
        (record) =>
            new Promise<void>((resolve) => {
                saved.push(record)
                release = resolve
            }),
    )
    expect(
        controller.prepare(['language_v1', 'account_v1', 'migration_v1', 'java_v1', 'welcome_v1']),
    ).toBe(true)
    expect(controller.steps.value.map((step) => step.id)).toEqual(['appearance_v1'])

    void controller.complete('appearance_v1')
    void controller.complete('appearance_v1')
    await flushPromises()
    release()
    await controller.finish()
    expect(saved).toEqual([
        ['language_v1', 'account_v1', 'migration_v1', 'java_v1', 'welcome_v1', 'appearance_v1'],
    ])
    expect(controller.prepare(saved[0])).toBe(false)
})

it('leaves a step whose write failed out of the next record, so it returns at the next start', async () => {
    const saved: string[][] = []
    const controller = createOobeController(async (record) => {
        if (record[record.length - 1] === 'account_v1') throw new Error('database unavailable')
        saved.push(record)
    })
    controller.prepare(['language_v1'])
    void controller.complete('account_v1')
    void controller.complete('migration_v1')
    await controller.finish()
    expect(saved).toEqual([['language_v1', 'migration_v1']])
    expect(controller.prepare(saved[0])).toBe(true)
    expect(controller.steps.value.map((step) => step.id)).toContain('account_v1')
})
