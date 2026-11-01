import { IntlMessageFormat } from 'intl-messageformat'
import { describe, expect, it, vi } from 'vitest'

import enMessages from '@/locales/en-US/index.json'
import zhMessages from '@/locales/zh-CN/index.json'

import type { InstallJobSnapshot } from './install'
import { installJobResourceMatches, installJobTitle } from './install-job-presentation'

vi.mock('@modrinth/ui', () => ({ defineMessages: (messages: unknown) => messages }))

function job(overrides: Partial<InstallJobSnapshot> = {}): InstallJobSnapshot {
    return {
        kind: 'install_content',
        display: { title: 'Install resources' },
        details: { type: 'empty' },
        items: [],
        ...overrides,
    } as unknown as InstallJobSnapshot
}

describe.each([
    ['en-US', enMessages],
    ['zh-CN', zhMessages],
] as const)('resource task titles (%s)', (locale, catalog) => {
    const format = (
        descriptor: { id?: string; defaultMessage?: string },
        values?: Record<string, string | number>,
    ) => {
        const translation = catalog[descriptor.id as keyof typeof catalog]
        return new IntlMessageFormat(
            translation?.message ?? descriptor.defaultMessage ?? '',
            locale,
        ).format(values) as string
    }
    const title = (snapshot: InstallJobSnapshot) => installJobTitle(snapshot, format, 'Fallback')

    it('localizes historical generic titles and includes full filenames without guessing names', () => {
        const filename = 'User-renamed-Sodium-0.5.8.jar.disabled'
        const snapshot = job({ items: [{ name: filename }] as InstallJobSnapshot['items'] })
        expect(title(snapshot)).toBe(
            locale === 'zh-CN' ? `安装内容：${filename}` : `Install content: ${filename}`,
        )
        expect(title(job())).toBe(locale === 'zh-CN' ? '安装内容' : 'Install content')
        expect(title(job({ kind: 'change_content', display: { title: 'Change resources' } }))).toBe(
            locale === 'zh-CN' ? '变更内容' : 'Change content',
        )
    })

    it('prefers persisted project titles and searches names beyond the batch summary', () => {
        const snapshot = job({
            resource_names: ['Sodium', 'Iris', 'AppleSkin'],
            items: [{ name: 'user-renamed-iris.jar' }] as InstallJobSnapshot['items'],
        })
        expect(title(snapshot)).toBe(
            locale === 'zh-CN' ? '安装内容：Sodium 等 3 项' : 'Install content: Sodium (3 items)',
        )
        expect(installJobResourceMatches(snapshot, 'appleskin')).toBe(true)
        expect(installJobResourceMatches(snapshot, 'user-renamed-iris.jar')).toBe(true)
        expect(installJobResourceMatches(snapshot, 'missing')).toBe(false)
        expect(title({ ...snapshot, kind: 'change_content' })).toBe(
            locale === 'zh-CN' ? '变更内容：Sodium 等 3 项' : 'Change content: Sodium (3 items)',
        )
        expect(title(job({ resource_names: ['Same title', 'Same title'] }))).toBe(
            locale === 'zh-CN'
                ? '安装内容：Same title 等 2 项'
                : 'Install content: Same title (2 items)',
        )
        expect(title(job({ kind: 'change_content', resource_names: ['Iris'] }))).toBe(
            locale === 'zh-CN' ? '变更内容：Iris' : 'Change content: Iris',
        )
    })

    it('preserves custom titles and non-content job fallbacks', () => {
        expect(title(job({ display: { title: 'My custom installation' } }))).toBe(
            'My custom installation',
        )
        expect(title(job({ kind: 'download_java', display: { title: 'Install resources' } }))).toBe(
            'Install resources',
        )
        expect(
            title(
                job({
                    kind: 'create_instance',
                    display: null,
                    details: { type: 'instance', name: 'My instance' },
                }),
            ),
        ).toBe('My instance')
    })
})
