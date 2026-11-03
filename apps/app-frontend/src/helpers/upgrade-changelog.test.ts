import { expect, it } from 'vitest'

import {
    getUpgradeChangelogTranslation,
    setUpgradeChangelogTranslation,
    shouldUpgradeChangelogStayOpen,
    upgradeChangelogTranslationCacheKey,
    upgradeExternalChangelogUrl,
} from './upgrade-changelog.ts'

it('changelog translations evict least-recently-used entries and reject oversized bodies', () => {
    for (let index = 0; index < 200; index++) {
        setUpgradeChangelogTranslation(`budget-${index}`, `body-${index}`)
    }
    expect(getUpgradeChangelogTranslation('budget-0')).toBe('body-0')
    setUpgradeChangelogTranslation('budget-new', 'new')
    expect(getUpgradeChangelogTranslation('budget-1')).toBe(undefined)
    expect(getUpgradeChangelogTranslation('budget-0')).toBe('body-0')
    setUpgradeChangelogTranslation('oversized', 'x'.repeat(3 * 1024 * 1024))
    expect(getUpgradeChangelogTranslation('oversized')).toBe(undefined)
})

it('external changelog links only allow HTTP protocols', () => {
    expect(upgradeExternalChangelogUrl('https://example.com/path')).toBe('https://example.com/path')
    expect(upgradeExternalChangelogUrl('https://example.com/path).')).toBe(
        'https://example.com/path',
    )
    expect(upgradeExternalChangelogUrl('http://example.com/path,')).toBe('http://example.com/path')
    expect(upgradeExternalChangelogUrl('javascript:alert(1)')).toBe(null)
})

it('changelog popover stays open while trigger or popup owns hover or focus', () => {
    const empty = {
        triggerHovered: false,
        triggerFocused: false,
        popupHovered: false,
        popupFocused: false,
    }
    expect(shouldUpgradeChangelogStayOpen({ ...empty, triggerHovered: true })).toBe(true)
    expect(shouldUpgradeChangelogStayOpen({ ...empty, popupHovered: true })).toBe(true)
    expect(
        shouldUpgradeChangelogStayOpen({ ...empty, triggerHovered: false, popupHovered: true }),
    ).toBe(true)
    expect(shouldUpgradeChangelogStayOpen({ ...empty, popupFocused: true })).toBe(true)
    expect(shouldUpgradeChangelogStayOpen(empty)).toBe(false)
})

it('translated changelog cache separates target languages and stays lazy', () => {
    const chinese = upgradeChangelogTranslationCacheKey('modrinth', 'project', 'release', 'zh-CN')
    const english = upgradeChangelogTranslationCacheKey('modrinth', 'project', 'release', 'en-US')
    expect(getUpgradeChangelogTranslation(chinese)).toBe(undefined)
    setUpgradeChangelogTranslation(chinese, 'translated')
    expect(getUpgradeChangelogTranslation(chinese)).toBe('translated')
    expect(getUpgradeChangelogTranslation(english)).toBe(undefined)
})
