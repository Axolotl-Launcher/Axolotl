import assert from 'node:assert/strict'
import test from 'node:test'

import {
    getUpgradeChangelogTranslation,
    setUpgradeChangelogTranslation,
    shouldUpgradeChangelogStayOpen,
    upgradeChangelogTranslationCacheKey,
    upgradeExternalChangelogUrl,
} from './upgrade-changelog.ts'

test('changelog translations evict least-recently-used entries and reject oversized bodies', () => {
    for (let index = 0; index < 200; index++) {
        setUpgradeChangelogTranslation(`budget-${index}`, `body-${index}`)
    }
    assert.equal(getUpgradeChangelogTranslation('budget-0'), 'body-0')
    setUpgradeChangelogTranslation('budget-new', 'new')
    assert.equal(getUpgradeChangelogTranslation('budget-1'), undefined)
    assert.equal(getUpgradeChangelogTranslation('budget-0'), 'body-0')
    setUpgradeChangelogTranslation('oversized', 'x'.repeat(3 * 1024 * 1024))
    assert.equal(getUpgradeChangelogTranslation('oversized'), undefined)
})

test('external changelog links only allow HTTP protocols', () => {
    assert.equal(
        upgradeExternalChangelogUrl('https://example.com/path'),
        'https://example.com/path',
    )
    assert.equal(
        upgradeExternalChangelogUrl('https://example.com/path).'),
        'https://example.com/path',
    )
    assert.equal(upgradeExternalChangelogUrl('http://example.com/path,'), 'http://example.com/path')
    assert.equal(upgradeExternalChangelogUrl('javascript:alert(1)'), null)
})

test('changelog popover stays open while trigger or popup owns hover or focus', () => {
    const empty = {
        triggerHovered: false,
        triggerFocused: false,
        popupHovered: false,
        popupFocused: false,
    }
    assert.equal(shouldUpgradeChangelogStayOpen({ ...empty, triggerHovered: true }), true)
    assert.equal(shouldUpgradeChangelogStayOpen({ ...empty, popupHovered: true }), true)
    assert.equal(
        shouldUpgradeChangelogStayOpen({ ...empty, triggerHovered: false, popupHovered: true }),
        true,
    )
    assert.equal(shouldUpgradeChangelogStayOpen({ ...empty, popupFocused: true }), true)
    assert.equal(shouldUpgradeChangelogStayOpen(empty), false)
})

test('translated changelog cache separates target languages and stays lazy', () => {
    const chinese = upgradeChangelogTranslationCacheKey('modrinth', 'project', 'release', 'zh-CN')
    const english = upgradeChangelogTranslationCacheKey('modrinth', 'project', 'release', 'en-US')
    assert.equal(getUpgradeChangelogTranslation(chinese), undefined)
    setUpgradeChangelogTranslation(chinese, 'translated')
    assert.equal(getUpgradeChangelogTranslation(chinese), 'translated')
    assert.equal(getUpgradeChangelogTranslation(english), undefined)
})
