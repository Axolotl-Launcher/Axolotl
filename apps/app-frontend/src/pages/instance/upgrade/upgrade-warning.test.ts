import { readFileSync } from 'node:fs'

import { expect, it } from 'vitest'

import type { InstanceUpgradeResult } from '@/helpers/instance-upgrade'

import { shouldExpandUpgradeWarningsByDefault } from '../../../helpers/post-upgrade-notice.ts'
import {
    filterUpgradeWarnings,
    paginateUpgradeWarnings,
    summarizeUpgradeWarnings,
    UPGRADE_WARNING_PAGE_SIZE,
    upgradeResultWarningRows,
    upgradeWarningDisplayName,
    upgradeWarningMessageId,
} from './upgrade-warning.ts'

const base = {
    planId: 'plan',
    sourceInstanceId: 'source',
    targetInstanceId: 'target',
    backupInstanceId: null,
    solution: { kind: 'custom', selections: [], dependencyChanges: [], warnings: [] },
    externalChanges: [],
    skippedDueToExternalConflict: [],
} as InstanceUpgradeResult

it('structured warning maps by stable code', () => {
    const rows = upgradeResultWarningRows({
        ...base,
        compatibilityWarnings: [],
        compatibilityWarningDetails: [
            {
                code: 'keep_incompatible',
                relativePath: 'mods/a.jar',
                contentId: 'a',
                provider: 'modrinth',
                projectId: 'project',
                conflictingProjectId: null,
            },
        ],
    })
    expect(upgradeWarningMessageId(rows[0].code!)).toBe(
        'instance.upgrade.warning.keep-incompatible',
    )
    const zhCn = JSON.parse(
        readFileSync(new URL('../../../locales/zh-CN/index.json', import.meta.url), 'utf8'),
    ) as Record<string, { message: string }>
    const localized = zhCn[upgradeWarningMessageId(rows[0].code!)]?.message
    expect(localized).toBe('{path} 已原样保留，可能与升级后的实例不兼容。')
    expect(localized).not.toMatch(/will be preserved/i)
})

it('legacy persisted warning falls back to raw message', () => {
    const rows = upgradeResultWarningRows({
        ...base,
        compatibilityWarnings: [
            {
                code: 'unidentified',
                message: 'Legacy backend text',
                contentId: null,
                provider: null,
                projectId: null,
                conflictingProjectId: null,
                dependencyRequirements: [],
            },
        ],
    })
    expect(rows[0].legacyMessage).toBe('Legacy backend text')
})

it('300 structured warnings stay summarized with path as secondary data', () => {
    const rows = upgradeResultWarningRows({
        ...base,
        compatibilityWarnings: [],
        compatibilityWarningDetails: Array.from({ length: 300 }, (_, index) => ({
            code:
                index < 200
                    ? 'unidentified'
                    : index < 275
                      ? 'no_compatible_release'
                      : 'prerelease_only',
            relativePath: `resourcepacks/example-${index}.zip`,
            contentId: `content-${index}`,
            provider: null,
            projectId: null,
            conflictingProjectId: null,
        })),
    })
    expect(summarizeUpgradeWarnings(rows)).toEqual({ local: 200, kept: 75, fallback: 25 })
    expect(upgradeWarningDisplayName(rows[0])).toBe('example-0.zip')
    expect(shouldExpandUpgradeWarningsByDefault(rows.length)).toBe(false)
    const zhCn = JSON.parse(
        readFileSync(new URL('../../../locales/zh-CN/index.json', import.meta.url), 'utf8'),
    ) as Record<string, { message: string }>
    expect(zhCn['instance.upgrade.result.warning-unidentified-headline']?.message).toBe(
        '此内容在升级时被原样保留',
    )

    const page1 = paginateUpgradeWarnings(rows, 1)
    const page2 = paginateUpgradeWarnings(rows, 2)
    const lastPage = paginateUpgradeWarnings(rows, 30)
    expect(UPGRADE_WARNING_PAGE_SIZE).toBe(10)
    expect(page1.items.length).toBe(10)
    expect(page2.items.map((row) => row.contentId)).toEqual(
        Array.from({ length: 10 }, (_, index) => `content-${index + 10}`),
    )
    expect(lastPage.items.map((row) => row.contentId)).toEqual(
        Array.from({ length: 10 }, (_, index) => `content-${index + 290}`),
    )
    const remainderPage = paginateUpgradeWarnings(rows.slice(0, 293), 30)
    expect(remainderPage.items.map((row) => row.contentId)).toEqual([
        'content-290',
        'content-291',
        'content-292',
    ])
    const searched = filterUpgradeWarnings(rows, 'all', 'example-287')
    expect(searched.map((row) => row.contentId)).toEqual(['content-287'])
    expect(paginateUpgradeWarnings(searched, 1).items.length).toBe(1)
    expect(filterUpgradeWarnings(rows, 'all', '').length).toBe(300)
    expect(filterUpgradeWarnings(rows, 'local', '').length).toBe(200)
    expect(filterUpgradeWarnings(rows, 'kept', '').length).toBe(75)
    expect(filterUpgradeWarnings(rows, 'fallback', '').length).toBe(25)

    const source = readFileSync(new URL('./UpgradeResultCollections.vue', import.meta.url), 'utf8')
    expect(source).toMatch(/v-if="warningsOpen"/)
    expect(source).toMatch(/v-for="warning in warningPage\.items"/)
    expect(source).not.toMatch(/v-for="warning in warnings"/)
    expect(source).toMatch(/technicalDetails/)
    expect(source).toMatch(/warningHeadline\(warning\)/)
    expect(source).toMatch(
        /watch\(\[warningSearch, warningFilter\],[\s\S]*?warningPageNumber\.value = 1/,
    )

    const buttonSlotStart = source.indexOf('<template #button="{ open }">')
    const buttonSlotEnd = source.indexOf('</template>', buttonSlotStart)
    const summaryPosition = source.indexOf('warningSummary.local', buttonSlotStart)
    expect(
        buttonSlotStart >= 0 &&
            summaryPosition > buttonSlotStart &&
            summaryPosition < buttonSlotEnd,
    ).toBeTruthy()
    expect(source).toMatch(/class="block w-full"/)
    expect(source).toMatch(/button-class="[^"]*w-full[^"]*focus-visible:ring-4/)

    const accordionSource = readFileSync(
        new URL('../../../../../../packages/ui/src/components/base/Accordion.vue', import.meta.url),
        'utf8',
    )
    const accordionButtonSlot = accordionSource.lastIndexOf('<slot name="button"')
    const accordionButtonStart = accordionSource.lastIndexOf('<button', accordionButtonSlot)
    const accordionButtonEnd = accordionSource.indexOf('</button>', accordionButtonSlot)
    expect(
        accordionButtonStart >= 0 &&
            accordionButtonSlot > accordionButtonStart &&
            accordionButtonSlot < accordionButtonEnd,
    ).toBeTruthy()
})
