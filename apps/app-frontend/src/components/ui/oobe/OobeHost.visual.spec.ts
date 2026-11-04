import { I18N_INJECTION_KEY, type ImportableLauncher, provideInstanceImport } from '@modrinth/ui'
import { flushPromises, mount } from '@vue/test-utils'
import IntlMessageFormat from 'intl-messageformat'
import { createPinia } from 'pinia'
import { afterEach, expect, it, vi } from 'vitest'
import { page } from 'vitest/browser'
import { defineComponent, h, provide, ref } from 'vue'

import en from '@/locales/en-US/index.json'
import zh from '@/locales/zh-CN/index.json'
import { applyTheme, waitFor } from '@/test/visual-harness'

import OobeHost from './OobeHost.vue'
import { OOBE_STEP_IDS } from './step-ids'
import { OOBE_STEPS } from './steps'
import { createOobeController } from './useOobe'

const backend = vi.hoisted(() => ({
    patches: [] as Record<string, unknown>[],
    restricted: true,
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
vi.mock('@tauri-apps/plugin-dialog', async (importOriginal) => ({
    ...(await importOriginal<object>()),
    open: async () => null,
}))

backend.invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    switch (command) {
        case 'plugin:settings|settings_get':
            return { locale: 'en-US', theme: 'dark', feature_flags: {} }
        case 'plugin:settings|settings_patch':
            backend.patches.push(args?.patch as Record<string, unknown>)
            return { locale: 'en-US', feature_flags: {}, ...(args?.patch as object) }
        case 'plugin:auth|get_users':
            return []
        case 'plugin:auth|get_default_user':
            return null
        case 'plugin:auth|get_anti_piracy_status':
            return { region: 'cn', restricted: backend.restricted, revision: 1 }
        default:
            return null
    }
})

const evidence = import.meta.env.VITE_AXOLOTL_VISUAL_EVIDENCE as string | undefined
const mounted: ReturnType<typeof mount>[] = []

afterEach(() => {
    mounted.splice(0).forEach((wrapper) => wrapper.unmount())
    backend.patches.length = 0
    backend.invoke.mockClear()
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

async function render(locale: 'en-US' | 'zh-CN' = 'en-US') {
    await page.viewport(1100, 700)
    const events = { complete: [] as string[], finished: 0 }
    const Harness = defineComponent({
        setup() {
            provideInstanceImport({
                getDetectedLaunchers: async (
                    onLauncher?: (launcher: ImportableLauncher) => void,
                ) => {
                    const launcher = {
                        name: 'HMCL',
                        path: 'D:/Games/.minecraft',
                        instances: [
                            { name: '1.21.4 Fabric', path: 'D:/Games/.minecraft/versions/a' },
                            { name: '1.20.1 Forge', path: 'D:/Games/.minecraft/versions/b' },
                        ],
                    }
                    onLauncher?.(launcher)
                    return [launcher]
                },
            } as never)
            provide('chooseImportMethod', async () => [])
            provide('showCreationModalWithOptions', () => {})
            return () =>
                h(OobeHost, {
                    steps: OOBE_STEPS,
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
    return { wrapper, events }
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
    const next = button('Next')!.getBoundingClientRect()
    expect(document.querySelector<HTMLElement>('.oobe-root')!.scrollTop).toBe(0)
    expect(title.top).toBeGreaterThanOrEqual(card.top)
    expect(next.bottom).toBeLessThanOrEqual(card.bottom)
    expect(card.bottom).toBeLessThanOrEqual(window.innerHeight)
}

async function shot(name: string) {
    if (evidence) await page.screenshot({ path: `${evidence}/${name}.png` })
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
    // Nothing to continue with yet, and offline accounts stay locked by the anti-piracy policy.
    expect(button('Next')!.disabled).toBe(true)
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
    await waitFor(() => document.body.textContent!.includes('2 instances'), {
        label: 'detected launcher',
    })
    await shot('oobe-migration')

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
    expect(controller.prepare(['language_v1', 'account_v1', 'migration_v1', 'welcome_v1'])).toBe(
        true,
    )
    expect(controller.steps.value.map((step) => step.id)).toEqual(['appearance_v1'])

    void controller.complete('appearance_v1')
    void controller.complete('appearance_v1')
    await flushPromises()
    release()
    await controller.finish()
    expect(saved).toEqual([
        ['language_v1', 'account_v1', 'migration_v1', 'welcome_v1', 'appearance_v1'],
    ])
    expect(controller.prepare(saved[0])).toBe(false)
})
