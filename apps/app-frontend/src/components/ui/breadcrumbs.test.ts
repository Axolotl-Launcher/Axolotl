import { readFileSync } from 'node:fs'

import { expect, it } from 'vitest'

import { resolveBreadcrumbLabel } from '../../helpers/breadcrumb-label.ts'

const localeFiles = {
    'en-US': '../../locales/en-US/index.json',
    'zh-CN': '../../locales/zh-CN/index.json',
    'zh-TW': '../../locales/zh-TW/index.json',
} as const

const localeMessages = Object.fromEntries(
    Object.entries(localeFiles).map(([locale, path]) => [
        locale,
        JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8')) as Record<
            string,
            { message: string }
        >,
    ]),
) as Record<keyof typeof localeFiles, Record<string, { message: string }>>

it('upgrade breadcrumb reuses localized Upgrade instance message at runtime', () => {
    let locale: keyof typeof localeFiles = 'en-US'
    const labels = { Upgrade: 'app.instance.upgrade-instance' }
    const resolve = () =>
        resolveBreadcrumbLabel(
            'Upgrade',
            () => '',
            labels,
            (messageId) => localeMessages[locale][messageId].message,
        )

    expect(resolve()).toBe('Upgrade instance')
    locale = 'zh-CN'
    expect(resolve()).toBe('升级实例')
    locale = 'zh-TW'
    expect(resolve()).toBe('升級實例')
})

it('screenshots breadcrumb resolves through the active locale', () => {
    let locale: 'en-US' | 'zh-CN' = 'en-US'
    const labels = { Screenshots: 'app.navigation.screenshots' }
    const resolve = () =>
        resolveBreadcrumbLabel(
            'Screenshots',
            () => '',
            labels,
            (messageId) => localeMessages[locale][messageId].message,
        )

    expect(resolve()).toBe('Screenshots')
    locale = 'zh-CN'
    expect(resolve()).toBe('截图中心')
})

it('upgrade route paths, internal names, and breadcrumb depth stay unchanged', () => {
    const routes = readFileSync(new URL('../../routes/instance.ts', import.meta.url), 'utf8')
    expect(routes).toMatch(/useRootContext: true,[\s\S]*?breadcrumb: \[\{ name: 'Upgrade' \}\]/)
    expect(routes).not.toMatch(/breadcrumb: \[\{ name: '\?Instance'[^\]]*\{ name: 'Upgrade' \}\]/)
    for (const [path, name] of [
        ['', 'InstanceUpgrade'],
        ['compatibility', 'InstanceUpgradeCompatibility'],
        ['customize', 'InstanceUpgradeCustomize'],
        ['confirm', 'InstanceUpgradeConfirm'],
        ['progress', 'InstanceUpgradeProgress'],
        ['result', 'InstanceUpgradeResult'],
    ] as const) {
        expect(routes).toMatch(new RegExp(`path: '${path}',\\s+name: '${name}'`))
    }
})

it('breadcrumb component resolves Upgrade through formatMessage on each render', () => {
    const source = readFileSync(new URL('./Breadcrumbs.vue', import.meta.url), 'utf8')
    expect(source).toMatch(/Upgrade: messages\.upgradeInstance/)
    expect(source).toMatch(/Screenshots: messages\.screenshots/)
    expect(source).toMatch(/Upgrade: ArrowBigUpDashIcon/)
    expect(source).toMatch(/screenshots: \{ id: 'app\.navigation\.screenshots'/)
    expect(source).toMatch(/id: 'app\.instance\.upgrade-instance'/)
    expect(source).toMatch(
        /resolveBreadcrumbLabel\([\s\S]*?\(message\) => formatMessage\(message\)/,
    )
})

it('breadcrumb separators render only between visible breadcrumb items', () => {
    const source = readFileSync(new URL('./Breadcrumbs.vue', import.meta.url), 'utf8')
    expect(source).toMatch(/v-for="\(breadcrumb, index\) in breadcrumbs"/)
    expect(source).toMatch(/v-if="index < breadcrumbs\.length - 1"/)
    expect(source).not.toMatch(/ChevronRightIcon v-if="breadcrumb\.link"/)
})
