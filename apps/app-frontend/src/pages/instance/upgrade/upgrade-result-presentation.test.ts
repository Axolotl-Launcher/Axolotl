import { readFileSync } from 'node:fs'

import { expect, it } from 'vitest'

import type { InstanceUpgradeSolution } from '@/helpers/instance-upgrade'

import {
    filterUpgradeDetailItems,
    paginateUpgradeDetailItems,
    UPGRADE_RESULT_PAGE_SIZE,
    upgradeDetailItems,
    upgradeDetailProjectIdentities,
    upgradeDetailReleaseIdentities,
} from './upgrade-result-presentation.ts'

function largeSolution(): InstanceUpgradeSolution {
    return {
        kind: 'custom',
        warnings: [],
        selections: Array.from({ length: 500 }, (_, index) => ({
            contentId: `example-${index}`,
            provider: 'modrinth',
            projectId: `project-${index}`,
            currentReleaseId: `old-${index}`,
            targetReleaseId: `new-${index}`,
            action: index % 3 === 0 ? 'keep' : index % 3 === 1 ? 'disable' : 'upgrade',
            enabled: index % 3 !== 1,
        })),
        dependencyChanges: [],
    }
}

it('500-item result paginates to 25 real visible rows per page', () => {
    const all = upgradeDetailItems(largeSolution())
    const first = paginateUpgradeDetailItems(all, 1)
    const second = paginateUpgradeDetailItems(all, 2)
    expect(UPGRADE_RESULT_PAGE_SIZE).toBe(25)
    expect(first.items.length).toBe(25)
    expect(first.items.map((item) => item.contentId)).toEqual(
        Array.from({ length: 25 }, (_, index) => `example-${index}`),
    )
    expect(second.items.map((item) => item.contentId)).toEqual(
        Array.from({ length: 25 }, (_, index) => `example-${index + 25}`),
    )
})

it('search and status filters happen before pagination', () => {
    const all = upgradeDetailItems(largeSolution())
    const match = filterUpgradeDetailItems(all, 'all', 'example-487')
    expect(match.map((item) => item.contentId)).toEqual(['example-487'])
    const updated = filterUpgradeDetailItems(all, 'updated', '')
    expect(updated.every((item) => item.action === 'upgrade')).toBeTruthy()
    expect(
        paginateUpgradeDetailItems(updated, 99).page <=
            paginateUpgradeDetailItems(updated, 99).pageCount,
    ).toBe(true)
})

it('metadata scope contains only identities from the current visible page', () => {
    const visible = paginateUpgradeDetailItems(upgradeDetailItems(largeSolution()), 1).items
    expect(upgradeDetailProjectIdentities(visible).length).toBe(25)
    expect(upgradeDetailReleaseIdentities(visible).length).toBe(50)
})

it('component resets page on filter/search and lazily mounts paginated rows', () => {
    const source = readFileSync(new URL('./UpgradeResultCollections.vue', import.meta.url), 'utf8')
    expect(source).toMatch(/watch\(\[search, filter\],[\s\S]*?page\.value = 1[\s\S]*?\)/)
    expect(source).toMatch(/v-if="detailsOpen"/)
    expect(source).toMatch(/v-for="item in visibleRows"/)
    expect(source).not.toMatch(/v-for="item in allItems"/)
    expect(source).toMatch(/upgradeDetailProjectIdentities\(pageData\.value\.items\)/)
})
